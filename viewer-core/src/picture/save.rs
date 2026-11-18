use std::fmt;
use std::io::Cursor;
use std::path::Path;

use image::codecs::gif::{GifEncoder, Repeat};
use image::codecs::jpeg::JpegEncoder;
use image::imageops;
use image::{Delay, DynamicImage, ImageFormat, RgbaImage};

use super::{Frame, Picture, can_save_jxl, encode_jxl_wic};
use crate::files::SAVE_FILE_TYPES;

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
    JxlUnavailable,
    Encode(String),
    Io(std::io::Error),
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SaveError::UnsupportedFormat(e) => write!(f, "unsupported save format: {e}"),
            SaveError::JxlUnavailable => write!(f, "JPEG XL encoder not installed"),
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
        let ext = ext.to_ascii_lowercase();
        if !SAVE_FILE_TYPES.contains(&ext.as_str()) {
            return Err(SaveError::UnsupportedFormat(ext));
        }
        if ext == ".jxl" && !can_save_jxl() {
            return Err(SaveError::JxlUnavailable);
        }
        let dir = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
        // Closing the handle keeps delete-on-drop but lets WIC open the file itself.
        let temp = tempfile::Builder::new().prefix(".save-").tempfile_in(dir)?.into_temp_path();
        if ext == ".jxl" {
            encode_jxl_wic(self.current(), &temp).map_err(SaveError::Encode)?;
        } else {
            std::fs::write(&temp, self.encode(&ext, quality.unwrap_or(100).clamp(1, 100))?)?;
        }
        // Flush to disk before the replace: a power cut must not leave a journaled rename over unwritten data.
        std::fs::OpenOptions::new().write(true).open(&temp)?.sync_all()?;
        temp.persist(path).map_err(|e| SaveError::Io(e.error))?;
        self.modified = false;
        Ok(())
    }

    fn encode(&self, ext: &str, quality: u8) -> Result<Vec<u8>, SaveError> {
        let frames = self.frames();
        let pixels: &RgbaImage = &frames[0].pixels;
        let (w, h) = pixels.dimensions();
        let mut out = Cursor::new(Vec::new());
        match ext {
            // JPEG has no alpha: ImageSharp drops it without compositing, so do the same.
            ".jpg" => DynamicImage::ImageRgba8(pixels.clone()).to_rgb8().write_with_encoder(JpegEncoder::new_with_quality(&mut out, quality))?,
            ".png" => pixels.write_to(&mut out, ImageFormat::Png)?,
            ".bmp" => pixels.write_to(&mut out, ImageFormat::Bmp)?,
            ".tga" => pixels.write_to(&mut out, ImageFormat::Tga)?,
            ".tiff" => pixels.write_to(&mut out, ImageFormat::Tiff)?,
            ".gif" => {
                let mut enc = GifEncoder::new(&mut out);
                enc.set_repeat(Repeat::Infinite)?;
                enc.encode_frames(frames.iter().map(|f| image::Frame::from_parts(f.pixels.clone(), 0, 0, Delay::from_numer_denom_ms(f.delay_ms, 1))))?;
                drop(enc);
            }
            ".webp" => return Ok(webp::Encoder::from_rgba(pixels.as_raw(), w, h).encode_simple(false, f32::from(quality)).map_err(|e| SaveError::Encode(format!("{e:?}")))?.to_vec()),
            _ => return Err(SaveError::UnsupportedFormat(ext.to_string())),
        }
        Ok(out.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn animated_gif_keeps_every_frame_and_delay() {
        let mut pic = Picture::from_rgba(RgbaImage::new(8, 8), 32);
        pic.frames[0].delay_ms = 40;
        pic.frames.push(Frame {
            pixels: RgbaImage::from_pixel(8, 8, image::Rgba([255, 0, 0, 255])),
            delay_ms: 70,
        });
        let bytes = pic.encode(".gif", 100).unwrap();
        let dec = image::codecs::gif::GifDecoder::new(Cursor::new(bytes)).unwrap();
        let frames = image::AnimationDecoder::into_frames(dec).collect_frames().unwrap();
        let delays: Vec<_> = frames.iter().map(|f| f.delay().numer_denom_ms()).map(|(n, d)| n / d).collect();
        assert_eq!(delays, [40, 70]);
    }
}
