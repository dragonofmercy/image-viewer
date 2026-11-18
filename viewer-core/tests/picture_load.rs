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

fn svg_text(w: u32, h: u32) -> String {
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}"><rect width="{w}" height="{h}" fill="red"/></svg>"#)
}

#[test]
fn load_svg_rasters_within_the_size_cap() {
    let dir = fixture_dir();
    for ((w, h), (ew, eh)) in [((32, 32), (32, 32)), ((800, 600), (800, 600)), ((4096, 2048), (1024, 512)), ((1000, 4000), (256, 1024))] {
        let p = write_bytes(&dir, "a.svg", svg_text(w, h).as_bytes());
        let pic = picture::load_path(&p).unwrap_or_else(|e| panic!("{w}x{h}: {e}"));
        assert_eq!((pic.width(), pic.height()), (ew, eh), "{w}x{h}");
        assert_eq!(pic.depth_string(), "32 bit");
        assert!(!pic.modified());
        assert_eq!(pic.current().get_pixel(ew / 2, eh / 2).0, [255, 0, 0, 255]);
    }
}

#[test]
fn load_svg_keeps_straight_alpha() {
    let dir = fixture_dir();
    let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><rect width="4" height="4" fill="#ff0000" fill-opacity="0.5"/></svg>"##;
    let pic = picture::load_path(&write_bytes(&dir, "a.svg", svg)).unwrap();
    let p = pic.current().get_pixel(1, 1).0;
    assert!(p[0] >= 250 && p[1] == 0 && p[2] == 0 && (126..=129).contains(&p[3]), "{p:?}");
}

#[test]
fn load_invalid_svg_is_error() {
    let dir = fixture_dir();
    assert!(matches!(picture::load_path(&write_bytes(&dir, "a.svg", b"not xml at all")), Err(LoadError::Decode(_))));
    let zero = write_bytes(&dir, "z.svg", svg_text(0, 0).as_bytes());
    assert!(matches!(picture::load_path(&zero), Err(LoadError::Decode(_))));
}

#[test]
fn load_animated_gif_keeps_every_frame_and_delay() {
    let dir = fixture_dir();
    let pic = picture::load_path(&save_animated_gif(&dir, "a.gif", 3, 50)).unwrap();
    assert!(pic.is_animated());
    assert_eq!(pic.frames().len(), 3);
    for (i, f) in pic.frames().iter().enumerate() {
        assert_eq!(f.delay_ms, 50);
        assert_eq!(*f.pixels.get_pixel(0, 0), anim_color(i as u32));
    }
}

#[test]
fn zero_delay_becomes_100ms() {
    let dir = fixture_dir();
    for delay in [0, 10] {
        let pic = picture::load_path(&save_animated_gif(&dir, "z.gif", 2, delay)).unwrap();
        assert!(pic.frames().iter().all(|f| f.delay_ms == 100), "delay {delay}");
    }
    let pic = picture::load_path(&save_animated_gif(&dir, "ok.gif", 2, 20)).unwrap();
    assert!(pic.frames().iter().all(|f| f.delay_ms == 20));
}

#[test]
fn single_frame_gif_is_not_animated() {
    let dir = fixture_dir();
    let pic = picture::load_path(&save_animated_gif(&dir, "one.gif", 1, 50)).unwrap();
    assert!(!pic.is_animated());
    assert_eq!(pic.frames().len(), 1);
}

#[test]
fn save_gif_keeps_animation() {
    let dir = fixture_dir();
    let mut pic = picture::load_path(&save_animated_gif(&dir, "a.gif", 3, 50)).unwrap();
    pic.rotate_flip(picture::Rotate::Rotate90, picture::Flip::None);
    let out = dir.path().join("out.gif");
    pic.save(&out, ".gif", None).unwrap();
    let again = picture::load_path(&out).unwrap();
    assert_eq!(again.frames().len(), 3);
    assert_eq!((again.width(), again.height()), (6, 8));
    assert!(again.frames().iter().all(|f| f.delay_ms == 50));
}

#[test]
fn truncated_gif_keeps_complete_frames() {
    let dir = fixture_dir();
    let bytes = std::fs::read(save_animated_gif(&dir, "a.gif", 3, 50)).unwrap();
    let pic = picture::load_path(&write_bytes(&dir, "cut.gif", &bytes[..bytes.len() - 12])).unwrap();
    assert_eq!(pic.frames().len(), 2);
    assert_eq!(*pic.frames()[1].pixels.get_pixel(0, 0), anim_color(1));
}

#[test]
fn gif_truncated_before_first_frame_is_error() {
    let dir = fixture_dir();
    let bytes = std::fs::read(save_animated_gif(&dir, "a.gif", 3, 50)).unwrap();
    // Header and logical screen descriptor only.
    let r = picture::load_path(&write_bytes(&dir, "cut.gif", &bytes[..13]));
    assert!(matches!(r, Err(LoadError::Decode(_))), "{:?}", r.err());
}

#[test]
fn load_animated_webp_keeps_every_frame() {
    let dir = fixture_dir();
    let pic = picture::load_path(&save_animated_webp(&dir, "a.webp", 3, 80)).unwrap();
    assert!(pic.is_animated());
    assert_eq!(pic.frames().len(), 3);
    assert_eq!(pic.frames()[0].delay_ms, 80);
    for (i, f) in pic.frames().iter().enumerate() {
        // The encoder is lossy by a level or two even with lossless set.
        let (got, want) = (f.pixels.get_pixel(0, 0).0, anim_color(i as u32).0);
        assert!(got.iter().zip(want).all(|(g, w)| g.abs_diff(w) <= 2), "{got:?} vs {want:?}");
    }
}

#[test]
fn still_webp_stays_single_frame() {
    let dir = fixture_dir();
    let pic = picture::load_path(&save_sample(&dir, "s.webp", 8, 6)).unwrap();
    assert!(!pic.is_animated());
}

#[test]
fn gif_named_png_still_plays_animated() {
    let dir = fixture_dir();
    let bytes = std::fs::read(save_animated_gif(&dir, "a.gif", 3, 50)).unwrap();
    let pic = picture::load_path(&write_bytes(&dir, "mislabeled.png", &bytes)).unwrap();
    assert_eq!(pic.frames().len(), 3);
}

fn is_too_large(r: Result<picture::Picture, LoadError>) -> bool {
    matches!(&r, Err(LoadError::Decode(m)) if m.contains("too large"))
}

#[test]
fn gif_with_huge_logical_screen_is_an_error_without_allocating_it() {
    let dir = fixture_dir();
    // 65535 x 65535 logical screen (17 GB as RGBA), then one 1x1 frame with a 2-color local table.
    let mut gif = b"GIF89a\xff\xff\xff\xff\x00\x00\x00".to_vec();
    gif.extend_from_slice(b"\x2c\0\0\0\0\x01\0\x01\0\x80\0\0\0\xff\xff\xff\x02\x02\x44\x01\0\x3b");
    let start = std::time::Instant::now();
    assert!(is_too_large(picture::load_path(&write_bytes(&dir, "huge.gif", &gif))));
    assert!(start.elapsed().as_secs() < 5);
}

#[test]
fn still_image_over_the_decode_cap_is_rejected_before_allocating() {
    let dir = fixture_dir();
    // BMP header only: 50000 x 50000 at 32 bpp (10 GB), no pixel data.
    let mut bmp = b"BM".to_vec();
    for v in [54u32, 0, 54, 40, 50000, 50000] {
        bmp.extend_from_slice(&v.to_le_bytes());
    }
    bmp.extend_from_slice(&1u16.to_le_bytes());
    bmp.extend_from_slice(&32u16.to_le_bytes());
    bmp.extend_from_slice(&[0; 24]);
    assert!(is_too_large(picture::load_path(&write_bytes(&dir, "huge.bmp", &bmp))));
}

#[test]
fn png_named_gif_and_webp_open_as_still() {
    let dir = fixture_dir();
    let bytes = std::fs::read(save_sample(&dir, "a.png", 8, 6)).unwrap();
    for name in ["mislabeled.gif", "mislabeled.webp"] {
        let pic = picture::load_path(&write_bytes(&dir, name, &bytes)).unwrap();
        assert!(!pic.is_animated(), "{name}");
        assert_eq!((pic.width(), pic.height()), (8, 6), "{name}");
    }
}
