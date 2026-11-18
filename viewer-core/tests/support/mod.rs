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
    let mut exif = b"II*\0\x08\0\0\0".to_vec(); // little-endian TIFF header, IFD0 at offset 8
    exif.extend_from_slice(&1u16.to_le_bytes()); // one entry
    exif.extend_from_slice(&0x0112u16.to_le_bytes());
    exif.extend_from_slice(&3u16.to_le_bytes()); // SHORT
    exif.extend_from_slice(&1u32.to_le_bytes());
    exif.extend_from_slice(&orientation.to_le_bytes());
    exif.extend_from_slice(&[0, 0]);
    exif.extend_from_slice(&0u32.to_le_bytes()); // no next IFD
    jpeg_with_metadata(dir, name, w, h, Some(&exif), None)
}

/// Little-endian EXIF (TIFF structure, no "Exif\0\0" prefix): IFD0 holds Make = `make` and Orientation = `orientation`.
pub fn exif_block(make: &str, orientation: u16) -> Vec<u8> {
    let make = [make.as_bytes(), b"\0"].concat();
    let mut exif = b"II*\0\x08\0\0\0".to_vec();
    exif.extend_from_slice(&2u16.to_le_bytes());
    // Make: ASCII stored after the IFD (8 header + 2 count + 2 * 12 entries + 4 next-IFD = 38).
    exif.extend_from_slice(&0x010Fu16.to_le_bytes());
    exif.extend_from_slice(&2u16.to_le_bytes());
    exif.extend_from_slice(&(make.len() as u32).to_le_bytes());
    exif.extend_from_slice(&38u32.to_le_bytes());
    exif.extend_from_slice(&0x0112u16.to_le_bytes());
    exif.extend_from_slice(&3u16.to_le_bytes());
    exif.extend_from_slice(&1u32.to_le_bytes());
    exif.extend_from_slice(&orientation.to_le_bytes());
    exif.extend_from_slice(&[0, 0]);
    exif.extend_from_slice(&0u32.to_le_bytes());
    exif.extend_from_slice(&make);
    exif
}

/// Fake ICC profile: a 128-byte header with its big-endian size and the "acsp" signature, then a recognizable tail.
pub fn fake_icc() -> Vec<u8> {
    let mut icc = vec![0u8; 160];
    icc[..4].copy_from_slice(&160u32.to_be_bytes());
    icc[36..40].copy_from_slice(b"acsp");
    icc[128..].iter_mut().enumerate().for_each(|(i, b)| *b = i as u8 + 7);
    icc
}

/// Writes a w x h sample JPEG with an APP1 Exif segment (`exif` is the TIFF structure) and a single APP2 ICC segment spliced after SOI.
pub fn jpeg_with_metadata(dir: &tempfile::TempDir, name: &str, w: u32, h: u32, exif: Option<&[u8]>, icc: Option<&[u8]>) -> PathBuf {
    let path = save_sample(dir, name, w, h);
    let jpeg = std::fs::read(&path).unwrap();
    let mut out = vec![0xFF, 0xD8];
    let mut segment = |marker: u8, payload: Vec<u8>| {
        out.extend_from_slice(&[0xFF, marker]);
        out.extend_from_slice(&((payload.len() + 2) as u16).to_be_bytes());
        out.extend_from_slice(&payload);
    };
    if let Some(exif) = exif {
        segment(0xE1, [b"Exif\0\0", exif].concat());
    }
    if let Some(icc) = icc {
        segment(0xE2, [b"ICC_PROFILE\0\x01\x01", icc].concat());
    }
    out.extend_from_slice(&jpeg[2..]);
    std::fs::write(&path, out).unwrap();
    path
}

/// Writes an RGBA PNG carrying the given EXIF (eXIf) and ICC (iCCP) chunks.
pub fn png_with_metadata(dir: &tempfile::TempDir, name: &str, img: &RgbaImage, exif: &[u8], icc: &[u8]) -> PathBuf {
    use image::ImageEncoder;
    let path = dir.path().join(name);
    let mut enc = image::codecs::png::PngEncoder::new(std::fs::File::create(&path).unwrap());
    enc.set_exif_metadata(exif.to_vec()).unwrap();
    enc.set_icc_profile(icc.to_vec()).unwrap();
    enc.write_image(img.as_raw(), img.width(), img.height(), image::ExtendedColorType::Rgba8).unwrap();
    path
}

/// True when `needle` occurs anywhere in `haystack`.
pub fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
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
    save_animated_gif_with_delays(dir, name, &vec![delay_ms; frames as usize])
}

/// Same as `save_animated_gif`, one frame per delay.
pub fn save_animated_gif_with_delays(dir: &tempfile::TempDir, name: &str, delays_ms: &[u32]) -> PathBuf {
    let path = dir.path().join(name);
    let mut enc = image::codecs::gif::GifEncoder::new(std::fs::File::create(&path).unwrap());
    enc.set_repeat(image::codecs::gif::Repeat::Infinite).unwrap();
    for (i, &delay_ms) in delays_ms.iter().enumerate() {
        let img = RgbaImage::from_pixel(8, 6, anim_color(i as u32));
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
