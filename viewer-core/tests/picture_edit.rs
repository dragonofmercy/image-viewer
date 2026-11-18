mod support;

use support::*;
use viewer_core::files::SAVE_FILE_TYPES;
use viewer_core::picture::{self, Flip, Picture, Rotate, SaveError};

fn load(dir: &tempfile::TempDir, name: &str, w: u32, h: u32) -> Picture {
    picture::load_path(&save_sample(dir, name, w, h)).unwrap()
}

#[test]
fn rotate_flip_rotate90_swaps_dimensions() {
    let dir = fixture_dir();
    let mut pic = load(&dir, "a.png", 40, 20);
    pic.rotate_flip(Rotate::Rotate90, Flip::None);
    assert_eq!((pic.width(), pic.height()), (20, 40));
    // Clockwise: the top-left pixel ends up top-right.
    assert_eq!(pic.current().get_pixel(19, 0).0, sample_image(40, 20).get_pixel(0, 0).0);
}

#[test]
fn rotate_flip_rotates_first_then_flips() {
    let dir = fixture_dir();
    let mut pic = load(&dir, "a.png", 40, 20);
    pic.rotate_flip(Rotate::Rotate90, Flip::Horizontal);
    // Rotated: original (0,0) at (19,0); mirrored: at (0,0).
    assert_eq!(pic.current().get_pixel(0, 0).0, sample_image(40, 20).get_pixel(0, 0).0);
}

#[test]
fn modified_false_after_load_true_after_transform_false_after_save() {
    let dir = fixture_dir();
    let path = save_sample(&dir, "a.png", 20, 20);
    let mut pic = picture::load_path(&path).unwrap();
    assert!(!pic.modified());
    pic.rotate_flip(Rotate::Rotate90, Flip::None);
    assert!(pic.modified());
    pic.save(&path, ".png", None).unwrap();
    assert!(!pic.modified());
    assert_eq!(image::image_dimensions(&path).unwrap(), (20, 20));
}

#[test]
fn crop_clamps_to_bounds() {
    let dir = fixture_dir();
    let mut pic = load(&dir, "a.png", 40, 20);
    pic.crop(30, 10, 100, 100);
    assert_eq!((pic.width(), pic.height()), (10, 10));
    assert_eq!(pic.current().get_pixel(0, 0).0, sample_image(40, 20).get_pixel(30, 10).0);
}

#[test]
fn crop_outside_or_empty_is_a_no_op() {
    let dir = fixture_dir();
    let mut pic = load(&dir, "a.png", 40, 20);
    pic.crop(40, 0, 10, 10);
    pic.crop(0, 20, 10, 10);
    pic.crop(5, 5, 0, 10);
    pic.crop(5, 5, 10, 0);
    assert_eq!((pic.width(), pic.height()), (40, 20));
    assert!(!pic.modified());
}

#[test]
fn select_icon_size_keeps_transforms_on_the_frame_they_were_applied_to() {
    let dir = fixture_dir();
    let mut pic = picture::load_path(&save_ico(&dir, "t.ico", &[(256, 256), (16, 16)])).unwrap();
    pic.crop(0, 0, 100, 40);
    assert_eq!(pic.width(), 100);
    pic.select_icon_size(1);
    assert_eq!(pic.width(), 16);
    pic.select_icon_size(0);
    assert_eq!((pic.width(), pic.height()), (100, 40));
}

#[test]
fn save_lower_quality_produces_smaller_file() {
    let dir = fixture_dir();
    // Noisy picture so quality actually matters.
    let noisy = image::RgbaImage::from_fn(128, 128, |x, y| image::Rgba([((x * 7) ^ (y * 13)) as u8, (x * y) as u8, (x * 31 + y * 17) as u8, 255]));
    let path = dir.path().join("n.png");
    noisy.save(&path).unwrap();
    let mut pic = picture::load_path(&path).unwrap();
    for ext in [".jpg", ".webp"] {
        let (lo, hi) = (dir.path().join(format!("lo{ext}")), dir.path().join(format!("hi{ext}")));
        pic.save(&lo, ext, Some(10)).unwrap();
        pic.save(&hi, ext, Some(100)).unwrap();
        let (lo_len, hi_len) = (std::fs::metadata(&lo).unwrap().len(), std::fs::metadata(&hi).unwrap().len());
        assert!(lo_len < hi_len, "{ext}: {lo_len} !< {hi_len}");
    }
}

#[test]
fn save_unknown_type_is_unsupported() {
    let dir = fixture_dir();
    let mut pic = load(&dir, "a.png", 8, 8);
    let target = dir.path().join("a.xyz");
    assert!(matches!(pic.save(&target, ".xyz", None), Err(SaveError::UnsupportedFormat(_))));
    assert!(matches!(pic.save(&target, ".jpeg", None), Err(SaveError::UnsupportedFormat(_))));
    assert!(!target.exists());
}

#[test]
fn save_round_trips_every_save_type() {
    let dir = fixture_dir();
    let mut pic = load(&dir, "a.png", 40, 20);
    for ext in SAVE_FILE_TYPES {
        if ext == ".jxl" && !picture::can_save_jxl() {
            eprintln!("skipping .jxl round trip: no JPEG XL encoder");
            assert!(matches!(pic.save(&dir.path().join("out.jxl"), ext, None), Err(SaveError::JxlUnavailable)));
            continue;
        }
        let path = dir.path().join(format!("out{ext}"));
        pic.save(&path, ext, None).unwrap();
        let back = picture::load_path(&path).unwrap_or_else(|e| panic!("{ext}: {e}"));
        assert_eq!((back.width(), back.height()), (40, 20), "{ext}");
    }
}

#[test]
fn failed_save_keeps_original_file() {
    let dir = fixture_dir();
    let mut pic = load(&dir, "a.png", 8, 8);
    pic.rotate_flip(Rotate::Rotate90, Flip::None);
    // Target is a directory: the final replace must fail.
    let target = dir.path().join("target.png");
    std::fs::create_dir(&target).unwrap();
    std::fs::write(target.join("keep.txt"), b"x").unwrap();
    assert!(pic.save(&target, ".png", None).is_err());
    assert!(pic.modified());
    assert!(target.join("keep.txt").exists());
    let names: Vec<_> = std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(names.len(), 2, "temp file left behind: {names:?}");
}

#[test]
fn failed_replace_leaves_original_bytes_intact() {
    let dir = fixture_dir();
    let path = save_sample(&dir, "a.png", 8, 8);
    let before = std::fs::read(&path).unwrap();
    let mut pic = picture::load_path(&path).unwrap();
    pic.crop(0, 0, 4, 4);
    let mut perms = std::fs::metadata(&path).unwrap().permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&path, perms.clone()).unwrap();
    let result = pic.save(&path, ".png", None);
    #[allow(clippy::permissions_set_readonly_false)] // Windows-only crate: the Unix world-writable concern does not apply
    perms.set_readonly(false);
    std::fs::set_permissions(&path, perms).unwrap();
    assert!(result.is_err());
    assert!(pic.modified());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1, "temp file left behind");
}

#[test]
fn save_webp_over_size_limit_is_an_error_not_a_panic() {
    let dir = fixture_dir();
    let mut pic = picture::load_from_rgba(image::RgbaImage::new(16384, 1));
    let path = dir.path().join("big.webp");
    assert!(matches!(pic.save(&path, ".webp", None), Err(SaveError::Encode(_))));
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn save_overwrites_existing_file() {
    let dir = fixture_dir();
    let path = save_sample(&dir, "a.png", 8, 8);
    let mut pic = picture::load_path(&path).unwrap();
    pic.crop(0, 0, 4, 4);
    pic.save(&path, ".png", None).unwrap();
    assert_eq!(image::image_dimensions(&path).unwrap(), (4, 4));
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn save_to_unicode_path() {
    let dir = fixture_dir();
    let mut pic = load(&dir, "a.png", 8, 8);
    let path = dir.path().join("été 日本.webp");
    pic.save(&path, ".webp", Some(80)).unwrap();
    assert_eq!(picture::load_path(&path).unwrap().width(), 8);
}
