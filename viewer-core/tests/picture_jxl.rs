mod support;

use support::*;
use viewer_core::picture::{self, LoadError, encode_jxl};

/// 240x135 opaque codestream from jxl-oxide's own documentation (bare codestream, no container),
/// so the read path is covered by a file libjxl did not write.
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
fn load_jxl_round_trips_pixels_under_a_non_ascii_path() {
    let dir = fixture_dir();
    let sub = dir.path().join("été 照片");
    std::fs::create_dir(&sub).unwrap();
    let p = sub.join("img.jxl");
    let src = sample_image(32, 16);
    std::fs::write(&p, encode_jxl(&src).unwrap()).unwrap();
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
fn save_jxl_keeps_alpha_only_when_used() {
    let dir = fixture_dir();
    let (opaque, translucent) = (dir.path().join("opaque.jxl"), dir.path().join("alpha.jxl"));
    let mut src = sample_image(8, 8);
    std::fs::write(&opaque, encode_jxl(&src).unwrap()).unwrap();
    assert!(picture::load_path(&opaque).unwrap().current().pixels().all(|p| p[3] == 255));
    src.get_pixel_mut(0, 0)[3] = 0;
    std::fs::write(&translucent, encode_jxl(&src).unwrap()).unwrap();
    assert!(picture::load_path(&translucent).unwrap().current().get_pixel(0, 0)[3] < 32);
}

#[test]
fn jxl_header_over_the_decode_cap_is_rejected() {
    let dir = fixture_dir();
    // Codestream signature, then SizeHeader bits (LSB first): div8 0, height selector 2 + 18 bits (40000), ratio 1 (square), all_default
    // metadata: 6.4 GB as RGBA. The zero tail is the start of a frame header whose TOC is never complete.
    let mut jxl = vec![0xff, 0x0a, 0xfc, 0xe1, 0x24, 0x01];
    jxl.resize(64, 0);
    let r = picture::load_path(&write_bytes(&dir, "huge.jxl", &jxl));
    assert!(matches!(&r, Err(LoadError::Decode(m)) if m.contains("too large")), "{:?}", r.err());
}

#[test]
fn load_garbage_jxl_is_error() {
    let dir = fixture_dir();
    let p = write_bytes(&dir, "bad.jxl", b"definitely not a jpeg xl codestream");
    assert!(matches!(picture::load_path(&p), Err(LoadError::Decode(_))));
}
