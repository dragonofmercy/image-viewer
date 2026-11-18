use std::fmt;
use std::path::Path;

use image::{DynamicImage, ImageDecoder, ImageError, ImageReader, Limits, RgbaImage};

use crate::files::extension_of;

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
}

/// Loads an image file, dispatching on its lower-cased extension.
pub fn load_path(path: &Path) -> Result<Picture, LoadError> {
    let ext = extension_of(path).unwrap_or_default();
    match ext.as_str() {
        ".jpg" | ".jpeg" | ".bmp" | ".png" | ".gif" | ".tif" | ".tiff" | ".tga" | ".webp" => load_native(path),
        ".svg" | ".ico" | ".heic" | ".heif" | ".avif" | ".jxl" => Err(LoadError::Unsupported(ext)),
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
    let mut reader = ImageReader::open(path)?.with_guessed_format()?;
    reader.limits(Limits::no_limits());
    let mut decoder = reader.into_decoder()?;
    let orientation = decoder.orientation().unwrap_or(image::metadata::Orientation::NoTransforms);
    let bits = decoder.color_type().bits_per_pixel();
    let mut img = DynamicImage::from_decoder(decoder)?;
    img.apply_orientation(orientation);
    Ok(Picture::from_rgba(img.into_rgba8(), bits))
}

impl Picture {
    fn from_rgba(pixels: RgbaImage, bits_per_pixel: u16) -> Picture {
        Picture {
            frames: vec![Frame { pixels, delay_ms: 0 }],
            icon_frames: Vec::new(),
            icon_index: 0,
            bits_per_pixel,
            modified: false,
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
        &self.frames
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
}
