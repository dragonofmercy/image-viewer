mod support;

use support::*;
use viewer_core::picture::{self, LoadError};

#[test]
fn load_unsupported_extension_is_error() {
    let dir = fixture_dir();
    let p = write_bytes(&dir, "a.txt", b"hello");
    assert!(matches!(picture::load_path(&p), Err(LoadError::Unsupported(_))));
}

#[test]
fn load_native_format_reports_dimensions() {
    let dir = fixture_dir();
    for name in ["a.png", "a.jpg", "a.bmp", "a.gif", "a.tiff", "a.webp", "a.tga"] {
        let p = save_sample(&dir, name, 4, 2);
        let pic = picture::load_path(&p).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!((pic.width(), pic.height()), (4, 2), "{name}");
        assert_eq!(pic.dimensions_string(), "4 x 2", "{name}");
        assert!(!pic.is_animated(), "{name}");
    }
}

#[test]
fn load_applies_exif_orientation_swapping_width_and_height() {
    let dir = fixture_dir();
    let p = jpeg_with_orientation(&dir, "o.jpg", 4, 2, 6);
    let pic = picture::load_path(&p).unwrap();
    assert_eq!((pic.width(), pic.height()), (2, 4));
}

#[test]
fn bgra_pixels_returns_expected_size_and_dimensions() {
    let dir = fixture_dir();
    let pic = picture::load_path(&save_sample(&dir, "a.png", 4, 2)).unwrap();
    let (bytes, w, h) = pic.bgra_pixels();
    assert_eq!((w, h), (4, 2));
    assert_eq!(bytes.len(), 4 * 2 * 4);
    // pixel (1, 0) = RGBA [11, 2, 30, 255] -> BGRA [30, 2, 11, 255]
    assert_eq!(&bytes[4..8], &[30, 2, 11, 255]);
}

#[test]
fn modified_false_after_load() {
    let dir = fixture_dir();
    assert!(!picture::load_path(&save_sample(&dir, "a.png", 4, 2)).unwrap().modified());
}

#[test]
fn modified_true_after_memory_load() {
    let pic = picture::load_from_rgba(sample_image(3, 3));
    assert!(pic.modified());
    assert_eq!((pic.width(), pic.height()), (3, 3));
}

#[test]
fn depth_string_reports_source_bits() {
    let dir = fixture_dir();
    assert_eq!(picture::load_path(&save_sample(&dir, "a.jpg", 4, 2)).unwrap().depth_string(), "24 bit");
    assert_eq!(picture::load_path(&save_sample(&dir, "a.png", 4, 2)).unwrap().depth_string(), "32 bit");
}

#[test]
fn load_truncated_png_is_error() {
    let dir = fixture_dir();
    let full = std::fs::read(save_sample(&dir, "a.png", 8, 8)).unwrap();
    let p = write_bytes(&dir, "t.png", &full[..full.len() / 2]);
    assert!(picture::load_path(&p).is_err());
}

#[test]
fn load_empty_file_is_error() {
    let dir = fixture_dir();
    assert!(picture::load_path(&write_bytes(&dir, "e.png", b"")).is_err());
    assert!(picture::load_path(&write_bytes(&dir, "g.jpg", b"not an image at all")).is_err());
}

#[test]
fn load_uppercase_extension() {
    let dir = fixture_dir();
    let p = save_sample(&dir, "SAMPLE.PNG", 4, 2);
    assert_eq!(picture::load_path(&p).unwrap().width(), 4);
}
