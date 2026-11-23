use std::fmt;
use std::io::{BufWriter, Seek, Write};
use std::path::Path;

use image::buffer::ConvertBuffer;
use image::codecs::gif::{GifEncoder, Repeat};
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngEncoder;
use image::codecs::tiff::TiffEncoder;
use image::imageops;
use image::{Delay, ImageEncoder, ImageFormat, RgbaImage};

use super::{Frame, Picture, encode_jxl, metadata};
use crate::files::{SAVE_FILE_TYPES, normalize_extension};

/// EXIF must fit one APP1 segment (65533 data bytes, minus the "Exif\0\0" prefix); the encoder writes the length unchecked.
const JPEG_MAX_EXIF: usize = 65533 - 6;
/// ICC spans at most 255 APP2 segments of 65533 - 14 bytes; above that the encoder fails the whole save.
const JPEG_MAX_ICC: usize = 255 * (65533 - 14);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rotate {
    None,
    Rotate90,
    Rotate180,
    Rotate270,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flip {
    None,
    Horizontal,
    Vertical,
}

#[derive(Debug)]
pub enum SaveError {
    UnsupportedFormat(String),
    Encode(String),
    Io(std::io::Error),
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SaveError::UnsupportedFormat(e) => write!(f, "unsupported save format: {e}"),
            SaveError::Encode(e) => write!(f, "encode error: {e}"),
            SaveError::Io(e) => write!(f, "io error: {e}"),
        }
    }
}

impl std::error::Error for SaveError {}

impl From<std::io::Error> for SaveError {
    fn from(e: std::io::Error) -> Self {
        SaveError::Io(e)
    }
}

impl From<image::ImageError> for SaveError {
    fn from(e: image::ImageError) -> Self {
        SaveError::Encode(e.to_string())
    }
}

impl Picture {
    /// Frames the transforms act on: every frame, or only the selected icon size.
    fn frames_mut(&mut self) -> &mut [Frame] {
        match self.icon_frames.get_mut(self.icon_index) {
            Some(f) => std::slice::from_mut(f),
            None => &mut self.frames,
        }
    }

    /// Rotates clockwise first, then flips.
    pub fn rotate_flip(&mut self, rotate: Rotate, flip: Flip) {
        for f in self.frames_mut() {
            let img = &mut f.pixels;
            match rotate {
                Rotate::None => {}
                Rotate::Rotate90 => *img = imageops::rotate90(img),
                Rotate::Rotate180 => imageops::rotate180_in_place(img),
                Rotate::Rotate270 => *img = imageops::rotate270(img),
            }
            match flip {
                Flip::None => {}
                Flip::Horizontal => imageops::flip_horizontal_in_place(img),
                Flip::Vertical => imageops::flip_vertical_in_place(img),
            }
        }
        self.modified = true;
    }

    /// Clamped to the image bounds; an empty or fully outside rectangle does nothing.
    pub fn crop(&mut self, x: u32, y: u32, w: u32, h: u32) {
        let (iw, ih) = self.current().dimensions();
        if x >= iw || y >= ih {
            return;
        }
        let (w, h) = (w.min(iw - x), h.min(ih - y));
        if w == 0 || h == 0 {
            return;
        }
        for f in self.frames_mut() {
            f.pixels = imageops::crop_imm(&f.pixels, x, y, w, h).to_image();
        }
        self.modified = true;
    }

    /// Encodes into a temp file next to `path`, then replaces `path`: a failure leaves the original untouched.
    pub fn save(&mut self, path: &Path, ext: &str, quality: Option<u8>) -> Result<(), SaveError> {
        let ext = normalize_extension(ext);
        if !SAVE_FILE_TYPES.contains(&ext.as_str()) {
            return Err(SaveError::UnsupportedFormat(ext));
        }
        let dir = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
        let mut temp = tempfile::Builder::new().prefix(".save-").tempfile_in(dir)?;
        let mut out = BufWriter::new(temp.as_file_mut());
        self.encode(&ext, quality.unwrap_or(100).clamp(1, 100), &mut out)?;
        // Explicit flush: BufWriter's Drop would swallow a write error and persist a truncated file.
        out.into_inner().map_err(|e| SaveError::Io(e.into_error()))?;
        // Flush to disk before the replace: a power cut must not leave a journaled rename over unwritten data.
        temp.as_file().sync_all()?;
        temp.persist(path).map_err(|e| SaveError::Io(e.error))?;
        self.modified = false;
        Ok(())
    }

    fn encode(&self, ext: &str, quality: u8, out: &mut (impl Write + Seek)) -> Result<(), SaveError> {
        let frames = self.frames();
        let pixels: &RgbaImage = &frames[0].pixels;
        let (w, h) = pixels.dimensions();
        match ext {
            ".jxl" => out.write_all(&encode_jxl(pixels).map_err(SaveError::Encode)?)?,
            // JPEG has no alpha: ImageSharp drops it without compositing, so do the same.
            ".jpg" => ConvertBuffer::<image::RgbImage>::convert(pixels).write_with_encoder(self.with_metadata(JpegEncoder::new_with_quality(&mut *out, quality), JPEG_MAX_EXIF, JPEG_MAX_ICC))?,
            ".png" => pixels.write_with_encoder(self.with_metadata(PngEncoder::new(&mut *out), usize::MAX, usize::MAX))?,
            ".bmp" => pixels.write_to(&mut *out, ImageFormat::Bmp)?,
            ".tga" => pixels.write_to(&mut *out, ImageFormat::Tga)?,
            // The image crate's TIFF encoder takes the ICC profile but has no EXIF support.
            ".tiff" => pixels.write_with_encoder(self.with_metadata(TiffEncoder::new(&mut *out), usize::MAX, usize::MAX))?,
            ".gif" => {
                // The default quantizer speed (1) is the slowest; the gif crate documents 10 as a good speed/quality compromise.
                let mut enc = GifEncoder::new_with_speed(&mut *out, 10);
                enc.set_repeat(Repeat::Infinite)?;
                enc.encode_frames(frames.iter().map(|f| image::Frame::from_parts(f.pixels.clone(), 0, 0, Delay::from_numer_denom_ms(f.delay_ms, 1))))?;
                drop(enc);
            }
            ".webp" => {
                let bare = if frames.len() > 1 {
                    encode_animated_webp(frames, quality)?
                } else {
                    webp::Encoder::from_rgba(pixels.as_raw(), w, h).encode_simple(false, f32::from(quality)).map_err(|e| SaveError::Encode(format!("{e:?}")))?.to_vec()
                };
                out.write_all(&metadata::add_webp_chunks(bare, w, h, self.icc.as_deref(), self.exif.as_deref()))?;
            }
            _ => return Err(SaveError::UnsupportedFormat(ext.to_string())),
        }
        Ok(())
    }

    /// Hands the picture's metadata to an encoder. A blob over the format's limit is left out rather than failing the save or writing a corrupt file.
    fn with_metadata<E: ImageEncoder>(&self, mut encoder: E, max_exif: usize, max_icc: usize) -> E {
        // Unsupported (TIFF EXIF) is the only error these setters return: the save goes on without it.
        if let Some(icc) = self.icc.as_ref().filter(|b| b.len() <= max_icc) {
            let _ = encoder.set_icc_profile(icc.clone());
        }
        if let Some(exif) = self.exif.as_ref().filter(|b| b.len() <= max_exif) {
            let _ = encoder.set_exif_metadata(exif.clone());
        }
        encoder
    }
}

/// Lossy animated WebP, looping forever, with the still encoder's quality mapping.
fn encode_animated_webp(frames: &[Frame], quality: u8) -> Result<Vec<u8>, SaveError> {
    let err = |e: String| SaveError::Encode(format!("animated WebP: {e}"));
    let (w, h) = frames[0].pixels.dimensions();
    // libwebp reads every frame as a canvas-sized buffer.
    if frames.iter().any(|f| f.pixels.dimensions() != (w, h)) {
        return Err(err("frames differ in size".into()));
    }
    let mut config = webp::WebPConfig::new().map_err(|_| err("config".into()))?;
    config.lossless = 0;
    config.alpha_compression = 1;
    config.quality = f32::from(quality);
    let mut encoder = webp::AnimEncoder::new(w, h, &config);
    let mut timestamp = 0i32;
    for f in frames {
        encoder.add_frame(webp::AnimFrame::from_rgba(f.pixels.as_raw(), w, h, timestamp));
        timestamp = i32::try_from(f.delay_ms).ok().and_then(|d| timestamp.checked_add(d)).ok_or_else(|| err("animation too long".into()))?;
    }
    let mut out = encoder.try_encode().map_err(|e| err(format!("{e:?}")))?.to_vec();
    metadata::set_webp_animation_length(&mut out, timestamp as u32);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::ImageDecoder;
    use std::io::Cursor;

    #[test]
    fn animated_gif_keeps_every_frame_and_delay() {
        let mut pic = Picture::from_rgba(RgbaImage::new(8, 8), 32);
        pic.frames[0].delay_ms = 40;
        pic.frames.push(Frame {
            pixels: RgbaImage::from_pixel(8, 8, image::Rgba([255, 0, 0, 255])),
            delay_ms: 70,
        });
        let mut bytes = Cursor::new(Vec::new());
        pic.encode(".gif", 100, &mut bytes).unwrap();
        let bytes = bytes.into_inner();
        let dec = image::codecs::gif::GifDecoder::new(Cursor::new(bytes)).unwrap();
        let frames = image::AnimationDecoder::into_frames(dec).collect_frames().unwrap();
        let delays: Vec<_> = frames.iter().map(|f| f.delay().numer_denom_ms()).map(|(n, d)| n / d).collect();
        assert_eq!(delays, [40, 70]);
    }

    #[test]
    fn animated_webp_keeps_frames_delays_and_metadata() {
        let mut pic = Picture::from_rgba(RgbaImage::from_pixel(8, 6, image::Rgba([200, 30, 30, 255])), 32);
        pic.frames[0].delay_ms = 40;
        for (delay_ms, rgba) in [(70, [30, 200, 30, 255]), (500, [30, 30, 200, 128])] {
            pic.frames.push(Frame {
                pixels: RgbaImage::from_pixel(8, 6, image::Rgba(rgba)),
                delay_ms,
            });
        }
        pic.icc = Some(b"icc!".to_vec());
        pic.exif = Some(b"II*\0exif".to_vec());
        let mut bytes = Cursor::new(Vec::new());
        pic.encode(".webp", 90, &mut bytes).unwrap();
        let bytes = bytes.into_inner();
        let fourccs: Vec<&[u8]> = metadata::webp_chunks(&bytes).unwrap().iter().map(|c| c.0).collect();
        assert_eq!(fourccs, [&b"VP8X"[..], b"ICCP", b"ANIM", b"ANMF", b"ANMF", b"ANMF", b"EXIF"]);
        let mut dec = image::codecs::webp::WebPDecoder::new(Cursor::new(bytes)).unwrap();
        assert!(dec.has_animation());
        assert_eq!(dec.icc_profile().unwrap().as_deref(), Some(&b"icc!"[..]));
        assert_eq!(dec.exif_metadata().unwrap().as_deref(), Some(&b"II*\0exif"[..]));
        let frames = image::AnimationDecoder::into_frames(dec).collect_frames().unwrap();
        let delays: Vec<_> = frames.iter().map(|f| f.delay().numer_denom_ms()).map(|(n, d)| n / d).collect();
        assert_eq!(delays, [40, 70, 500]);
        let alpha = frames[2].buffer().get_pixel(4, 3)[3];
        assert!(alpha.abs_diff(128) <= 4, "alpha {alpha}");
    }
}
