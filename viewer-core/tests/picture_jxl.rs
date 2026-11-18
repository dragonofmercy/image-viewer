mod support;

use support::*;
use viewer_core::picture::{self, LoadError, can_save_jxl, encode_jxl_wic};

/// 240x135 opaque codestream from jxl-oxide's own documentation (bare codestream, no container),
/// so the read path is covered on machines without the Store "JPEG XL Image Extension".
const TINY_JXL: [u8; 42] = [
    0xff, 0x0a, 0x30, 0x54, 0x10, 0x09, 0x08, 0x06, 0x01, 0x00, 0x78, 0x00, 0x4b, 0x38, 0x41, 0x3c, 0xb6, 0x3a, 0x51, 0xfe, 0x00, 0x47, 0x1e, 0xa0, 0x85, 0xb8, 0x27, 0x1a, 0x48, 0x45, 0x84, 0x1b, 0x71, 0x4f, 0xa8, 0x3e, 0x8e, 0x30, 0x03, 0x92, 0x84, 0x01,
];

#[test]
fn load_embedded_jxl_codestream() {
    let dir = fixture_dir();
    let p = write_bytes(&dir, "tiny.JXL", &TINY_JXL);
    let pic = picture::load_path(&p).unwrap();
    assert_eq!((pic.width(), pic.height()), (240, 135));
    assert_eq!(pic.depth_string(), "24 bit");
    assert!(!pic.is_animated());
    assert!(!pic.modified());
    assert!(pic.current().pixels().all(|p| p[3] == 255));
}

#[test]
fn save_jxl_round_trips_through_wic() {
    if !can_save_jxl() {
        eprintln!("skipped: no working WIC JPEG XL encoder (Store \"JPEG XL Image Extension\" not installed)");
        return;
    }
    let dir = fixture_dir();
    let p = dir.path().join("out.jxl");
    encode_jxl_wic(&sample_image(4, 2), &p).unwrap();
    let pic = picture::load_path(&p).unwrap();
    assert_eq!((pic.width(), pic.height()), (4, 2));
}

#[test]
fn load_jxl_decodes_without_store_extension() {
    if !can_save_jxl() {
        eprintln!("skipped: no WIC JPEG XL encoder to build the fixture (load_embedded_jxl_codestream covers the read path)");
        return;
    }
    let dir = fixture_dir();
    let sub = dir.path().join("été 照片");
    std::fs::create_dir(&sub).unwrap();
    let p = sub.join("img.jxl");
    let src = sample_image(32, 16);
    encode_jxl_wic(&src, &p).unwrap();
    let pic = picture::load_path(&p).unwrap();
    assert_eq!((pic.width(), pic.height()), (32, 16));
    assert!(!pic.modified());
    // Channel order kept: red grows with x, green with y (lossy, so a tolerance).
    let [r, g, b, a] = pic.current().get_pixel(20, 0).0;
    assert!(r > 170 && g < 40 && b < 60 && a == 255, "{r} {g} {b} {a}");
    let [r, g, _, _] = pic.current().get_pixel(0, 15).0;
    assert!(r < 40 && g > 120, "{r} {g}");
}

#[test]
fn load_garbage_jxl_is_error() {
    let dir = fixture_dir();
    let p = write_bytes(&dir, "bad.jxl", b"definitely not a jpeg xl codestream");
    assert!(matches!(picture::load_path(&p), Err(LoadError::Decode(_))));
}
