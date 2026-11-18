#![allow(dead_code)]

use std::path::{Path, PathBuf};

use image::{DynamicImage, RgbaImage};

pub fn fixture_dir() -> tempfile::TempDir {
    tempfile::tempdir().expect("create temp dir")
}

/// Deterministic sample: pixel (x, y) = [x*10+1, y*10+2, 30, 255].
pub fn sample_image(w: u32, h: u32) -> RgbaImage {
    RgbaImage::from_fn(w, h, |x, y| image::Rgba([(x * 10 + 1) as u8, (y * 10 + 2) as u8, 30, 255]))
}

/// Writes a w x h image in the format of the file name's extension (JPEG is written as RGB).
pub fn save_sample(dir: &tempfile::TempDir, name: &str, w: u32, h: u32) -> PathBuf {
    let path = dir.path().join(name);
    let img = DynamicImage::ImageRgba8(sample_image(w, h));
    let img = if name.to_ascii_lowercase().ends_with(".jpg") { DynamicImage::ImageRgb8(img.to_rgb8()) } else { img };
    img.save(&path).expect("save sample");
    path
}

/// Writes a JPEG whose APP1 Exif segment (inserted right after SOI) carries the given orientation.
pub fn jpeg_with_orientation(dir: &tempfile::TempDir, name: &str, w: u32, h: u32, orientation: u16) -> PathBuf {
    let path = save_sample(dir, name, w, h);
    let jpeg = std::fs::read(&path).unwrap();
    let mut exif: Vec<u8> = b"Exif\0\0".to_vec();
    exif.extend_from_slice(b"II*\0\x08\0\0\0"); // little-endian TIFF header, IFD0 at offset 8
    exif.extend_from_slice(&1u16.to_le_bytes()); // one entry
    exif.extend_from_slice(&0x0112u16.to_le_bytes());
    exif.extend_from_slice(&3u16.to_le_bytes()); // SHORT
    exif.extend_from_slice(&1u32.to_le_bytes());
    exif.extend_from_slice(&orientation.to_le_bytes());
    exif.extend_from_slice(&[0, 0]);
    exif.extend_from_slice(&0u32.to_le_bytes()); // no next IFD
    let mut out = vec![0xFF, 0xD8, 0xFF, 0xE1];
    out.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
    out.extend_from_slice(&exif);
    out.extend_from_slice(&jpeg[2..]);
    std::fs::write(&path, out).unwrap();
    path
}

/// Writes an .ico with one PNG-payload entry per (w, h), in the given order.
pub fn save_ico(dir: &tempfile::TempDir, name: &str, sizes: &[(u32, u32)]) -> PathBuf {
    let payloads: Vec<Vec<u8>> = sizes
        .iter()
        .map(|&(w, h)| {
            let mut buf = std::io::Cursor::new(Vec::new());
            DynamicImage::ImageRgba8(sample_image(w, h)).write_to(&mut buf, image::ImageFormat::Png).unwrap();
            buf.into_inner()
        })
        .collect();
    let mut out = vec![0, 0, 1, 0];
    out.extend_from_slice(&(sizes.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * sizes.len() as u32;
    for (&(w, h), p) in sizes.iter().zip(&payloads) {
        out.extend_from_slice(&[w as u8, h as u8, 0, 0]); // 256 wraps to 0, as the format wants
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&32u16.to_le_bytes());
        out.extend_from_slice(&(p.len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += p.len() as u32;
    }
    payloads.iter().for_each(|p| out.extend_from_slice(p));
    write_bytes(dir, name, &out)
}

pub fn write_bytes(dir: &tempfile::TempDir, name: &str, bytes: &[u8]) -> PathBuf {
    let path: &Path = &dir.path().join(name);
    std::fs::write(path, bytes).unwrap();
    path.to_path_buf()
}

/// Solid color of animation frame `i`: distinct per frame so tests can tell frames apart.
pub fn anim_color(i: u32) -> image::Rgba<u8> {
    image::Rgba([40 + (i * 80) as u8, 200 - (i * 70) as u8, 90, 255])
}

/// Writes an animated GIF of `frames` 8x6 solid-color frames (see `anim_color`), each shown for `delay_ms`.
pub fn save_animated_gif(dir: &tempfile::TempDir, name: &str, frames: u32, delay_ms: u32) -> PathBuf {
    let path = dir.path().join(name);
    let mut enc = image::codecs::gif::GifEncoder::new(std::fs::File::create(&path).unwrap());
    enc.set_repeat(image::codecs::gif::Repeat::Infinite).unwrap();
    for i in 0..frames {
        let img = RgbaImage::from_pixel(8, 6, anim_color(i));
        enc.encode_frame(image::Frame::from_parts(img, 0, 0, image::Delay::from_numer_denom_ms(delay_ms, 1))).unwrap();
    }
    path
}

/// Writes an animated WebP of `frames` 8x6 lossless solid-color frames (see `anim_color`), `delay_ms` apart.
pub fn save_animated_webp(dir: &tempfile::TempDir, name: &str, frames: u32, delay_ms: u32) -> PathBuf {
    let mut config = webp::WebPConfig::new().unwrap();
    config.lossless = 1;
    let imgs: Vec<RgbaImage> = (0..frames).map(|i| RgbaImage::from_pixel(8, 6, anim_color(i))).collect();
    let mut enc = webp::AnimEncoder::new(8, 6, &config);
    for (i, img) in imgs.iter().enumerate() {
        enc.add_frame(webp::AnimFrame::from_rgba(img.as_raw(), 8, 6, (i as u32 * delay_ms) as i32));
    }
    write_bytes(dir, name, &enc.encode())
}
