mod support;

use support::*;
use viewer_core::picture::{self, LoadError};

#[test]
fn load_ico_succeeds() {
    let dir = fixture_dir();
    let p = save_ico(&dir, "a.ico", &[(16, 16), (32, 32)]);
    let pic = picture::load_path(&p).unwrap();
    assert_eq!(pic.depth_string(), "32 bit");
    assert!(!pic.is_animated());
}

#[test]
fn load_ico_picks_largest_frame_not_the_first_one() {
    let dir = fixture_dir();
    let p = save_ico(&dir, "a.ico", &[(16, 16), (48, 48), (32, 32)]);
    let pic = picture::load_path(&p).unwrap();
    assert_eq!((pic.width(), pic.height()), (48, 48));
    assert_eq!(pic.icon_size_index(), 0);
}

#[test]
fn load_ico_exposes_every_size_largest_first() {
    let dir = fixture_dir();
    let p = save_ico(&dir, "a.ico", &[(16, 16), (48, 48), (32, 32)]);
    let pic = picture::load_path(&p).unwrap();
    assert!(pic.has_icon_sizes());
    assert_eq!(pic.icon_size_count(), 3);
    let sizes: Vec<_> = (0..3).map(|i| pic.icon_size(i)).collect();
    assert_eq!(sizes, [(48, 48), (32, 32), (16, 16)]);
    assert_eq!(pic.icon_frame(2).dimensions(), (16, 16));
}

#[test]
fn select_icon_size_swaps_the_displayed_frame() {
    let dir = fixture_dir();
    let p = save_ico(&dir, "a.ico", &[(16, 16), (48, 48), (32, 32)]);
    let mut pic = picture::load_path(&p).unwrap();
    pic.select_icon_size(2);
    assert_eq!(pic.icon_size_index(), 2);
    assert_eq!((pic.width(), pic.height()), (16, 16));
    assert_eq!(pic.bgra_pixels().1, 16);
    pic.select_icon_size(99);
    assert_eq!(pic.icon_size_index(), 2);
}

#[test]
fn has_icon_sizes_false_without_size_variants() {
    let dir = fixture_dir();
    let ico = picture::load_path(&save_ico(&dir, "one.ico", &[(32, 32)])).unwrap();
    assert!(!ico.has_icon_sizes());
    assert_eq!(ico.icon_size_count(), 1);
    let png = picture::load_path(&save_sample(&dir, "a.png", 4, 4)).unwrap();
    assert!(!png.has_icon_sizes());
}

#[test]
fn load_ico_with_256_entry_and_bad_input_never_panics() {
    let dir = fixture_dir();
    let pic = picture::load_path(&save_ico(&dir, "big.ico", &[(256, 256), (16, 16)])).unwrap();
    assert_eq!((pic.width(), pic.height()), (256, 256));

    let good = std::fs::read(save_ico(&dir, "g.ico", &[(16, 16)])).unwrap();
    let cases: [(&str, Vec<u8>); 4] = [("empty", vec![]), ("zero_entries", vec![0, 0, 1, 0, 0, 0]), ("truncated_dir", good[..10].to_vec()), ("offset_past_end", good[..good.len() - 10].to_vec())];
    for (name, bytes) in cases {
        let p = write_bytes(&dir, &format!("{name}.ico"), &bytes);
        assert!(matches!(picture::load_path(&p), Err(LoadError::Decode(_))), "{name}");
    }
}

#[test]
fn ico_with_more_than_256_entries_is_error() {
    let dir = fixture_dir();
    let p = save_ico(&dir, "many.ico", &[(1, 1); 300]);
    assert!(matches!(picture::load_path(&p), Err(LoadError::Decode(_))));
    assert!(picture::load_path(&save_ico(&dir, "max.ico", &[(1, 1); 256])).is_ok());
}

#[test]
fn load_ico_decodes_dib_payload() {
    let dir = fixture_dir();
    let mut dib = Vec::new();
    for v in [40u32, 2, 4] {
        dib.extend_from_slice(&v.to_le_bytes()); // header size, width, height (doubled)
    }
    dib.extend_from_slice(&1u16.to_le_bytes());
    dib.extend_from_slice(&32u16.to_le_bytes());
    dib.extend_from_slice(&[0; 24]); // compression .. important colors
    for _ in 0..4 {
        dib.extend_from_slice(&[10, 20, 30, 255]); // BGRA
    }
    dib.extend_from_slice(&[0; 8]); // 1-bpp AND mask, 2 rows padded to 4 bytes
    let mut ico = vec![0, 0, 1, 0, 1, 0, 2, 2, 0, 0, 1, 0, 32, 0];
    ico.extend_from_slice(&(dib.len() as u32).to_le_bytes());
    ico.extend_from_slice(&22u32.to_le_bytes());
    ico.extend_from_slice(&dib);
    let pic = picture::load_path(&write_bytes(&dir, "dib.ico", &ico)).unwrap();
    assert_eq!((pic.width(), pic.height()), (2, 2));
    assert_eq!(pic.current().get_pixel(0, 0).0, [30, 20, 10, 255]);
}

#[test]
fn frames_yields_the_displayed_icon_frame_and_is_never_animated() {
    let dir = fixture_dir();
    let mut pic = picture::load_path(&save_ico(&dir, "a.ico", &[(16, 16), (48, 48), (32, 32)])).unwrap();
    for i in [0, 2] {
        pic.select_icon_size(i);
        assert_eq!(pic.frames().len(), 1);
        assert_eq!(pic.frames()[0].pixels.dimensions(), pic.current().dimensions());
        assert!(!pic.is_animated());
    }
}

/// Two-entry ICO: the real 16x16 entry of `save_ico`, then a second entry pointing at `second_payload` (appended) or at `second_offset` when given.
fn ico_with_second_entry(dir: &tempfile::TempDir, second_payload: &[u8], second_offset: Option<u32>) -> std::path::PathBuf {
    let good = std::fs::read(save_ico(dir, "g.ico", &[(16, 16)])).unwrap();
    let (entry, payload) = (&good[6..22], &good[22..]);
    let mut second = entry.to_vec();
    second[8..12].copy_from_slice(&(second_payload.len() as u32).to_le_bytes());
    let payload_end = 38 + payload.len() as u32;
    second[12..16].copy_from_slice(&second_offset.unwrap_or(payload_end).to_le_bytes());
    let mut first = entry.to_vec();
    first[12..16].copy_from_slice(&38u32.to_le_bytes());
    let mut ico = vec![0, 0, 1, 0, 2, 0];
    ico.extend_from_slice(&first);
    ico.extend_from_slice(&second);
    ico.extend_from_slice(payload);
    ico.extend_from_slice(second_payload);
    write_bytes(dir, "two.ico", &ico)
}

#[test]
fn load_ico_skips_a_corrupt_entry() {
    let dir = fixture_dir();
    let pic = picture::load_path(&ico_with_second_entry(&dir, &[0xAB; 40], None)).unwrap();
    assert_eq!(pic.width(), 16);
    assert!(!pic.has_icon_sizes());
}

#[test]
fn load_ico_with_only_corrupt_entries_fails() {
    let dir = fixture_dir();
    let mut ico = vec![0, 0, 1, 0, 1, 0];
    ico.extend_from_slice(&[16, 16, 0, 0, 1, 0, 32, 0, 40, 0, 0, 0, 22, 0, 0, 0]);
    ico.extend_from_slice(&[0xAB; 40]);
    assert!(picture::load_path(&write_bytes(&dir, "bad.ico", &ico)).is_err());
}

#[test]
fn load_ico_skips_an_out_of_range_entry() {
    let dir = fixture_dir();
    let pic = picture::load_path(&ico_with_second_entry(&dir, &[0xAB; 40], Some(1 << 30))).unwrap();
    assert_eq!(pic.width(), 16);
}
