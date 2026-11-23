use std::sync::atomic::{AtomicU32, Ordering};

use viewer_core::settings::*;
use winreg::enums::*;
use winreg::{RegKey, RegValue};

/// Sandbox key under HKCU, deleted on drop (even on panic). Never touches the real app key.
struct Sandbox {
    path: String,
}

impl Sandbox {
    fn new() -> Self {
        // pid + clock + counter: parallel tests in one process can read the same clock tick.
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = format!(r"SOFTWARE\Dragon Industries\Image Viewer Tests\{}-{}-{}", std::process::id(), nanos, NEXT.fetch_add(1, Ordering::Relaxed));
        assert_ne!(path, APP_KEY);
        Self { path }
    }

    fn settings(&self) -> Settings {
        Settings::at(&self.path)
    }

    fn raw(&self, name: &str) -> RegValue<'_> {
        let key = RegKey::predef(HKEY_CURRENT_USER).open_subkey(&self.path).unwrap();
        key.get_raw_value(name).unwrap()
    }

    fn get_string(&self, name: &str) -> String {
        RegKey::predef(HKEY_CURRENT_USER).open_subkey(&self.path).unwrap().get_value(name).unwrap()
    }

    fn put_sz(&self, name: &str, v: &str) {
        let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(&self.path).unwrap();
        key.set_value(name, &v).unwrap();
    }

    fn put_dword(&self, name: &str, v: u32) {
        let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(&self.path).unwrap();
        key.set_value(name, &v).unwrap();
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let _ = hkcu.delete_subkey_all(&self.path);
        // Remove the shared parent only when empty (non-recursive, fails if a parallel test still has a child).
        let _ = hkcu.delete_subkey(r"SOFTWARE\Dragon Industries\Image Viewer Tests");
    }
}

#[test]
fn defaults_on_an_empty_key() {
    let sb = Sandbox::new();
    let s = sb.settings();
    assert_eq!(s.theme(), Theme::Default);
    assert_eq!(s.pos_x(), None);
    assert_eq!(s.pos_y(), None);
    assert_eq!(s.width(), 1280);
    assert_eq!(s.height(), 768);
    assert_eq!(s.window_state(), WindowState::Normal);
    assert_eq!(s.last_update_check(), "");
    assert_eq!(s.update_interval(), "");
    assert_eq!(s.language(), "");
    assert_eq!(s.jpeg_quality(), 100);
    assert_eq!(s.webp_quality(), 80);
    assert_eq!(s.file_assoc_stamp(), "");
    assert!(!s.checkerboard());
}

#[test]
fn round_trip_every_value() {
    let sb = Sandbox::new();
    let s = sb.settings();
    s.set_theme(Theme::Dark).unwrap();
    s.set_pos_x(Some(-1920)).unwrap();
    s.set_pos_y(Some(40)).unwrap();
    s.set_width(1300).unwrap();
    s.set_height(900).unwrap();
    s.set_window_state(WindowState::Maximized).unwrap();
    s.set_last_update_check("2026-01-02 03:04:05").unwrap();
    s.set_update_interval("7").unwrap();
    s.set_language("fr").unwrap();
    s.set_jpeg_quality(55).unwrap();
    s.set_webp_quality(66).unwrap();
    s.set_checkerboard(true).unwrap();
    s.set_file_assoc_stamp(r"1|C:\app.exe").unwrap();

    assert_eq!(s.theme(), Theme::Dark);
    assert_eq!(s.pos_x(), Some(-1920));
    assert_eq!(s.pos_y(), Some(40));
    assert_eq!(s.width(), 1300);
    assert_eq!(s.height(), 900);
    assert_eq!(s.window_state(), WindowState::Maximized);
    assert_eq!(s.last_update_check(), "2026-01-02 03:04:05");
    assert_eq!(s.update_interval(), "7");
    assert_eq!(s.language(), "fr");
    assert_eq!(s.jpeg_quality(), 55);
    assert_eq!(s.webp_quality(), 66);
    assert!(s.checkerboard());
    assert_eq!(s.file_assoc_stamp(), r"1|C:\app.exe");

    s.set_pos_x(None).unwrap();
    assert_eq!(s.pos_x(), None);
}

#[test]
fn settings_clone_targets_the_same_key() {
    let sb = Sandbox::new();
    let s = sb.settings();
    let copy = s.clone();
    s.set_language("de").unwrap();
    assert_eq!(copy.language(), "de");
    assert!(format!("{copy:?}").contains("Image Viewer Tests"));
}

#[test]
fn quality_setters_clamp() {
    let sb = Sandbox::new();
    let s = sb.settings();
    s.set_jpeg_quality(0).unwrap();
    assert_eq!(s.jpeg_quality(), 1);
    s.set_webp_quality(250).unwrap();
    assert_eq!(s.webp_quality(), 100);
}

#[test]
fn touch_last_update_check_writes_the_format() {
    let sb = Sandbox::new();
    let s = sb.settings();
    s.touch_last_update_check().unwrap();
    let v = s.last_update_check();
    chrono::NaiveDateTime::parse_from_str(&v, UPDATE_DATE_FORMAT).unwrap();
}

#[test]
fn wrong_types_fall_back_to_defaults() {
    let sb = Sandbox::new();
    sb.put_sz("Theme", "2");
    sb.put_sz("Width", "abc");
    sb.put_dword("State", 99);
    sb.put_dword("PosX", 5);
    let s = sb.settings();
    assert_eq!(s.theme(), Theme::Default);
    assert_eq!(s.width(), 1280);
    assert_eq!(s.window_state(), WindowState::Normal);
    assert_eq!(s.pos_x(), None);
}

#[test]
fn writes_use_the_1x_value_types() {
    let sb = Sandbox::new();
    let s = sb.settings();
    s.set_width(1300).unwrap();
    s.set_theme(Theme::Dark).unwrap();
    s.set_jpeg_quality(70).unwrap();
    let w = sb.raw("Width");
    assert_eq!(w.vtype, REG_SZ);
    assert_eq!(sb.get_string("Width"), "1300");
    let t = sb.raw("Theme");
    assert_eq!(t.vtype, REG_DWORD);
    assert_eq!(&t.bytes[..], &2u32.to_le_bytes()[..]);
    assert_eq!(sb.raw("JpegQuality").vtype, REG_DWORD);
}
