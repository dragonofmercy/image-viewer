mod support;

use image::metadata::Orientation;
use support::*;
use viewer_core::picture::{self, Flip, Picture, Rotate};

fn orientation_of(pic: &Picture) -> Option<Orientation> {
    Orientation::from_exif_chunk(pic.exif().expect("exif kept"))
}

fn assert_metadata(pic: &Picture, icc: &[u8]) {
    assert!(contains(pic.exif().expect("exif kept"), b"TestCam"));
    assert_eq!(pic.icc_profile(), Some(icc));
}

fn tagged_jpeg(dir: &tempfile::TempDir) -> std::path::PathBuf {
    jpeg_with_metadata(dir, "src.jpg", 8, 6, Some(&exif_block("TestCam", 1)), Some(&fake_icc()))
}

#[test]
fn jpeg_exif_and_icc_survive_load_rotate_save() {
    let dir = fixture_dir();
    let mut pic = picture::load_path(&tagged_jpeg(&dir)).unwrap();
    assert_metadata(&pic, &fake_icc());
    pic.rotate_flip(Rotate::Rotate90, Flip::None);
    let out = dir.path().join("out.jpg");
    pic.save(&out, ".jpg", None).unwrap();
    let back = picture::load_path(&out).unwrap();
    assert_eq!((back.width(), back.height()), (6, 8));
    assert_metadata(&back, &fake_icc());
}

#[test]
fn orientation_tag_is_reset_after_load() {
    let dir = fixture_dir();
    let mut pic = picture::load_path(&jpeg_with_metadata(&dir, "o.jpg", 4, 2, Some(&exif_block("TestCam", 6)), None)).unwrap();
    assert_eq!((pic.width(), pic.height()), (2, 4));
    assert_eq!(orientation_of(&pic), Some(Orientation::NoTransforms));
    let out = dir.path().join("out.jpg");
    pic.save(&out, ".jpg", None).unwrap();
    let back = picture::load_path(&out).unwrap();
    assert_eq!((back.width(), back.height()), (2, 4));
    assert_eq!(orientation_of(&back), Some(Orientation::NoTransforms));
    assert!(contains(back.exif().unwrap(), b"TestCam"));
}

#[test]
fn metadata_carries_across_formats() {
    let dir = fixture_dir();
    let mut pic = picture::load_path(&tagged_jpeg(&dir)).unwrap();
    for ext in [".png", ".webp"] {
        let out = dir.path().join(format!("out{ext}"));
        pic.save(&out, ext, None).unwrap();
        assert_metadata(&picture::load_path(&out).unwrap(), &fake_icc());
    }
    // The image crate's TIFF encoder writes ICC but has no EXIF support, and its decoder reads no EXIF either.
    let out = dir.path().join("out.tiff");
    pic.save(&out, ".tiff", None).unwrap();
    let back = picture::load_path(&out).unwrap();
    assert_eq!(back.icc_profile(), Some(&fake_icc()[..]));
    assert_eq!(back.exif(), None);
}

#[test]
fn webp_with_alpha_keeps_metadata_and_alpha() {
    let dir = fixture_dir();
    let mut img = sample_image(8, 6);
    img.put_pixel(2, 3, image::Rgba([0, 0, 0, 0]));
    let mut pic = picture::load_path(&png_with_metadata(&dir, "a.png", &img, &exif_block("TestCam", 1), &fake_icc())).unwrap();
    assert_metadata(&pic, &fake_icc());
    let out = dir.path().join("out.webp");
    pic.save(&out, ".webp", None).unwrap();
    let back = picture::load_path(&out).unwrap();
    assert_eq!(back.current().get_pixel(2, 3).0[3], 0);
    assert_eq!(back.current().get_pixel(0, 0).0[3], 255);
    assert_metadata(&back, &fake_icc());
    // Decodes through the image crate directly too, not only through our loader.
    assert_eq!(image::open(&out).unwrap().to_rgba8().get_pixel(2, 3).0[3], 0);
}

#[test]
fn icc_larger_than_one_jpeg_segment_round_trips() {
    let dir = fixture_dir();
    let mut icc = fake_icc();
    icc.resize(150_000, 0x5A);
    icc[..4].copy_from_slice(&150_000u32.to_be_bytes());
    let mut pic = picture::load_path(&png_with_metadata(&dir, "a.png", &sample_image(8, 6), &exif_block("TestCam", 1), &icc)).unwrap();
    let out = dir.path().join("out.jpg");
    pic.save(&out, ".jpg", None).unwrap();
    assert_metadata(&picture::load_path(&out).unwrap(), &icc);
}

#[test]
fn memory_loaded_picture_has_no_metadata() {
    let dir = fixture_dir();
    let mut pic = picture::load_from_rgba(sample_image(4, 4));
    assert_eq!((pic.exif(), pic.icc_profile()), (None, None));
    let out = dir.path().join("out.png");
    pic.save(&out, ".png", None).unwrap();
    let back = picture::load_path(&out).unwrap();
    assert_eq!((back.exif(), back.icc_profile()), (None, None));
}

#[test]
fn png_without_metadata_saves_without_chunks() {
    let dir = fixture_dir();
    let mut pic = picture::load_path(&save_sample(&dir, "a.png", 4, 4)).unwrap();
    for ext in [".png", ".webp", ".jpg"] {
        let out = dir.path().join(format!("out{ext}"));
        pic.save(&out, ext, None).unwrap();
        let bytes = std::fs::read(&out).unwrap();
        for name in [&b"iCCP"[..], b"eXIf", b"ICCP", b"EXIF", b"VP8X", b"Exif\0\0", b"ICC_PROFILE"] {
            assert!(!contains(&bytes, name), "{ext} gained {}", String::from_utf8_lossy(name));
        }
    }
}

#[test]
fn corrupt_exif_does_not_break_load_or_save() {
    let dir = fixture_dir();
    let garbage: Vec<u8> = (0..40u8).map(|i| i.wrapping_mul(37)).collect();
    let mut pic = picture::load_path(&jpeg_with_metadata(&dir, "c.jpg", 4, 2, Some(&garbage), None)).unwrap();
    assert_eq!((pic.width(), pic.height()), (4, 2));
    assert_eq!(pic.exif(), None);
    let out = dir.path().join("out.jpg");
    pic.save(&out, ".jpg", None).unwrap();
    assert!(picture::load_path(&out).is_ok());
}
