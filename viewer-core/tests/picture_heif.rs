// All fixtures are AV1 (AVIF / AV1-coded HEIF); HEVC (real iPhone HEIC) was verified manually on 2026-11-18 with two real files, not by an automated test.
mod support;

use std::path::PathBuf;

use image::{Rgba, RgbaImage};
use support::*;
use viewer_core::picture::{self, LoadError, encode_av1_heif};

/// Left half fully transparent, right half opaque.
fn half_transparent(w: u32, h: u32) -> RgbaImage {
    RgbaImage::from_fn(w, h, |x, _| if x < w / 2 { Rgba([0, 0, 0, 0]) } else { Rgba([200, 40, 40, 255]) })
}

fn save_heif(dir: &tempfile::TempDir, name: &str, img: &RgbaImage) -> PathBuf {
    let path = dir.path().join(name);
    encode_av1_heif(img, &path).expect("encode AV1 HEIF fixture");
    path
}

#[test]
fn load_avif_decodes_through_libheif() {
    let dir = fixture_dir();
    let p = save_heif(&dir, "a.avif", &sample_image(32, 16));
    let pic = picture::load_path(&p).unwrap();
    assert_eq!((pic.width(), pic.height()), (32, 16));
    assert_eq!(pic.depth_string(), "24 bit");
    assert!(!pic.is_animated());
    assert!(!pic.modified());
    assert_eq!(pic.current().get_pixel(5, 5)[3], 255);
}

#[test]
fn load_heif_with_alpha_keeps_transparency() {
    for name in ["a.avif", "a.heif"] {
        let dir = fixture_dir();
        let p = save_heif(&dir, name, &half_transparent(32, 16));
        let pic = picture::load_path(&p).unwrap();
        assert_eq!(pic.depth_string(), "32 bit", "{name}");
        assert_eq!(pic.current().get_pixel(0, 0)[3], 0, "{name}");
        let [r, g, _, a] = pic.current().get_pixel(31, 15).0;
        assert_eq!(a, 255, "{name}");
        assert!(r > 150 && g < 90, "channel order kept for {name}: {r} {g}");
    }
}

#[test]
fn load_heif_from_unicode_path() {
    let dir = fixture_dir();
    let src = save_heif(&dir, "src.heic", &sample_image(32, 16));
    let sub = dir.path().join("été 照片");
    std::fs::create_dir(&sub).unwrap();
    let p = sub.join("img.heic");
    std::fs::copy(&src, &p).unwrap();
    let pic = picture::load_path(&p).unwrap();
    assert_eq!((pic.width(), pic.height()), (32, 16));
}

#[test]
fn load_garbage_heic_is_error() {
    let dir = fixture_dir();
    let p = write_bytes(&dir, "bad.heic", b"definitely not a heif container");
    let r = picture::load_path(&p);
    // A missing or unloadable DLL is a Decode error too: make sure libheif itself rejected the bytes.
    assert!(matches!(&r, Err(LoadError::Decode(m)) if m != "libheif.dll not found" && !m.starts_with("libheif.dll:")), "{:?}", r.err());
}
