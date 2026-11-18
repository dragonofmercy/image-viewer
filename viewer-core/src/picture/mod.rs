use std::fmt;
use std::path::Path;

use image::codecs::tiff::TiffDecoder;
use image::{DynamicImage, ImageDecoder, ImageError, ImageFormat, ImageReader, Limits, RgbaImage};

use crate::files::extension_of;

mod animated;
mod heif;
mod ico;
mod jxl;
mod metadata;
mod save;
mod svg;

#[doc(hidden)]
pub use heif::encode_av1_heif;
pub use jxl::can_save_jxl;
#[doc(hidden)]
pub use jxl::encode_jxl_wic;
pub use save::{Flip, Rotate, SaveError};

/// Cap on the pixel memory one decode may allocate: a failed allocation aborts the process, so hostile headers must fail before it. Lets a 200+ Mpx RGBA image through.
pub const MAX_DECODE_BYTES: u64 = 4 * 1024 * 1024 * 1024;

/// `image` decoder limits: no width/height cap, allocations capped at MAX_DECODE_BYTES.
fn decode_limits() -> Limits {
    let mut limits = Limits::no_limits();
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    limits
}

/// Adds `bytes` to the running decode total `used`, failing once it passes MAX_DECODE_BYTES.
fn reserve(used: &mut u64, bytes: u64) -> Result<(), LoadError> {
    *used = used.saturating_add(bytes);
    if *used > MAX_DECODE_BYTES { Err(LoadError::Decode("image too large".into())) } else { Ok(()) }
}

fn rgba_bytes(width: u32, height: u32) -> u64 {
    (u64::from(width) * u64::from(height)).saturating_mul(4)
}

pub struct Frame {
    pub pixels: RgbaImage,
    pub delay_ms: u32,
}

#[derive(Debug)]
pub enum LoadError {
    Unsupported(String),
    Decode(String),
    Io(std::io::Error),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Unsupported(e) => write!(f, "unsupported format: {e}"),
            LoadError::Decode(e) => write!(f, "decode error: {e}"),
            LoadError::Io(e) => write!(f, "io error: {e}"),
        }
    }
}

impl std::error::Error for LoadError {}

impl From<std::io::Error> for LoadError {
    fn from(e: std::io::Error) -> Self {
        LoadError::Io(e)
    }
}

impl From<ImageError> for LoadError {
    fn from(e: ImageError) -> Self {
        match e {
            ImageError::IoError(e) => LoadError::Io(e),
            e => LoadError::Decode(e.to_string()),
        }
    }
}

pub struct Picture {
    frames: Vec<Frame>,
    icon_frames: Vec<Frame>,
    icon_index: usize,
    bits_per_pixel: u16,
    modified: bool,
    exif: Option<Vec<u8>>,
    icc: Option<Vec<u8>>,
}

/// Loads an image file, dispatching on its lower-cased extension.
pub fn load_path(path: &Path) -> Result<Picture, LoadError> {
    let ext = extension_of(path).unwrap_or_default();
    match ext.as_str() {
        ".jpg" | ".jpeg" | ".bmp" | ".png" | ".gif" | ".tif" | ".tiff" | ".tga" | ".webp" => load_native(path),
        ".svg" => Ok(Picture::from_rgba(svg::rasterize_svg(path)?, 32)),
        ".ico" => Ok(Picture::from_icon_frames(ico::decode_ico(path)?)),
        ".heic" | ".heif" | ".avif" => {
            let (pixels, has_alpha) = heif::decode_heif(&std::fs::read(path)?)?;
            Ok(Picture::from_rgba(pixels, if has_alpha { 32 } else { 24 }))
        }
        ".jxl" => {
            let (pixels, has_alpha) = jxl::decode_jxl(&std::fs::read(path)?)?;
            Ok(Picture::from_rgba(pixels, if has_alpha { 32 } else { 24 }))
        }
        _ => Err(LoadError::Unsupported(ext)),
    }
}

/// Clipboard/paste path: the picture is considered modified.
pub fn load_from_rgba(pixels: RgbaImage) -> Picture {
    let mut p = Picture::from_rgba(pixels, 32);
    p.modified = true;
    p
}

fn load_native(path: &Path) -> Result<Picture, LoadError> {
    if let Some(pic) = animated::load_animated(path)? {
        return Ok(pic);
    }
    let mut reader = ImageReader::open(path)?.with_guessed_format()?;
    reader.limits(decode_limits());
    let is_tiff = reader.format() == Some(ImageFormat::Tiff);
    let mut decoder = reader.into_decoder()?;
    // from_decoder allocates the whole buffer without consulting the limits: check it, plus the RGBA copy, first.
    let (w, h) = decoder.dimensions();
    let mut used = 0;
    reserve(&mut used, decoder.total_bytes())?;
    reserve(&mut used, rgba_bytes(w, h))?;
    let (exif, mut icc) = metadata::read(&mut decoder);
    if icc.is_none() && is_tiff {
        // image's TiffDecoder::set_limits caps tag values to the pixel buffer size, dropping the ICC of small images: retry at the tiff crate's own limits.
        icc = std::fs::File::open(path).ok().and_then(|f| TiffDecoder::new(std::io::BufReader::new(f)).ok()).and_then(|mut d| metadata::read(&mut d).1);
    }
    let orientation = decoder.orientation().unwrap_or(image::metadata::Orientation::NoTransforms);
    let bits = decoder.color_type().bits_per_pixel();
    let mut img = DynamicImage::from_decoder(decoder)?;
    img.apply_orientation(orientation);
    Ok(Picture {
        exif,
        icc,
        ..Picture::from_rgba(img.into_rgba8(), bits)
    })
}

impl Picture {
    fn from_rgba(pixels: RgbaImage, bits_per_pixel: u16) -> Picture {
        Picture {
            frames: vec![Frame { pixels, delay_ms: 0 }],
            icon_frames: Vec::new(),
            icon_index: 0,
            bits_per_pixel,
            modified: false,
            exif: None,
            icc: None,
        }
    }

    fn from_icon_frames(icon_frames: Vec<Frame>) -> Picture {
        Picture {
            frames: Vec::new(),
            icon_frames,
            icon_index: 0,
            bits_per_pixel: 32,
            modified: false,
            exif: None,
            icc: None,
        }
    }

    pub fn has_icon_sizes(&self) -> bool {
        self.icon_frames.len() > 1
    }

    pub fn icon_size_count(&self) -> usize {
        self.icon_frames.len()
    }

    pub fn icon_size(&self, i: usize) -> (u32, u32) {
        self.icon_frames[i].pixels.dimensions()
    }

    pub fn icon_frame(&self, i: usize) -> &RgbaImage {
        &self.icon_frames[i].pixels
    }

    pub fn icon_size_index(&self) -> usize {
        self.icon_index
    }

    /// Out-of-range indices are ignored.
    pub fn select_icon_size(&mut self, i: usize) {
        if i < self.icon_frames.len() {
            self.icon_index = i;
        }
    }

    pub fn width(&self) -> u32 {
        self.current().width()
    }

    pub fn height(&self) -> u32 {
        self.current().height()
    }

    pub fn dimensions_string(&self) -> String {
        format!("{} x {}", self.width(), self.height())
    }

    pub fn depth_string(&self) -> String {
        format!("{} bit", self.bits_per_pixel)
    }

    pub fn frames(&self) -> &[Frame] {
        match self.icon_frames.get(self.icon_index) {
            Some(f) => std::slice::from_ref(f),
            None => &self.frames,
        }
    }

    pub fn is_animated(&self) -> bool {
        self.frames.len() > 1
    }

    /// Displayed frame: frame 0, or the selected icon size.
    pub fn current(&self) -> &RgbaImage {
        match self.icon_frames.get(self.icon_index) {
            Some(f) => &f.pixels,
            None => &self.frames[0].pixels,
        }
    }

    /// Top-down BGRA with straight alpha, as the clipboard path expects.
    pub fn bgra_pixels(&self) -> (Vec<u8>, u32, u32) {
        let img = self.current();
        let mut bytes = img.as_raw().clone();
        bytes.as_chunks_mut::<4>().0.iter_mut().for_each(|p| p.swap(0, 2));
        (bytes, img.width(), img.height())
    }

    pub fn modified(&self) -> bool {
        self.modified
    }

    /// Raw EXIF payload: the TIFF structure from its byte-order mark, without the "Exif\0\0" prefix. Its Orientation is 1: the loader already applied it.
    pub fn exif(&self) -> Option<&[u8]> {
        self.exif.as_deref()
    }

    pub fn icc_profile(&self) -> Option<&[u8]> {
        self.icc.as_deref()
    }
}
