//! Registry settings, byte-compatible with the C# app (same key, value names, types and defaults).
use winreg::enums::*;
use winreg::{RegKey, RegValue};

pub const APP_KEY: &str = r"SOFTWARE\Dragon Industries\Image Viewer";
pub const UPDATE_DATE_FORMAT: &str = "%Y-%m-%d %H:%M:%S";
pub const JPEG_QUALITY_DEFAULT: u8 = 100;
pub const WEBP_QUALITY_DEFAULT: u8 = 80;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Theme {
    Default = 0,
    Light = 1,
    Dark = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowState {
    Normal = 0,
    Minimized = 1,
    Maximized = 2,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RawValue {
    Missing,
    Dword(u32),
    Str(String),
}

/// Port of `Settings.ClampQuality`: any unparseable or missing value gives `default`, otherwise clamp to 1..=100.
pub fn clamp_quality(raw: RawValue, default: u8) -> u8 {
    let parsed = match raw {
        RawValue::Missing => None,
        RawValue::Dword(v) => Some(i64::from(v)),
        RawValue::Str(s) => s.trim().parse::<i32>().ok().map(i64::from),
    };
    parsed.map_or(default, |q| q.clamp(1, 100) as u8)
}

#[derive(Clone, Debug)]
pub struct Settings {
    key: String,
}

impl Settings {
    pub fn app() -> Self {
        Self::at(APP_KEY)
    }

    pub fn at(key: &str) -> Self {
        Self { key: key.to_string() }
    }

    fn read(&self, name: &str) -> RawValue {
        let Ok(key) = RegKey::predef(HKEY_CURRENT_USER).open_subkey(&self.key) else {
            return RawValue::Missing;
        };
        match key.get_raw_value(name) {
            Ok(RegValue { vtype: REG_DWORD, .. }) => key.get_value::<u32, _>(name).map_or(RawValue::Missing, RawValue::Dword),
            Ok(RegValue { vtype: REG_SZ, .. }) => key.get_value::<String, _>(name).map_or(RawValue::Missing, RawValue::Str),
            _ => RawValue::Missing,
        }
    }

    fn write_str(&self, name: &str, v: &str) -> std::io::Result<()> {
        RegKey::predef(HKEY_CURRENT_USER).create_subkey(&self.key)?.0.set_value(name, &v)
    }

    fn write_dword(&self, name: &str, v: u32) -> std::io::Result<()> {
        RegKey::predef(HKEY_CURRENT_USER).create_subkey(&self.key)?.0.set_value(name, &v)
    }

    fn string(&self, name: &str) -> String {
        match self.read(name) {
            RawValue::Str(s) => s,
            _ => String::new(),
        }
    }

    /// Numbers are stored as REG_SZ; anything missing or unparseable is `None`.
    fn number<T: std::str::FromStr>(&self, name: &str) -> Option<T> {
        match self.read(name) {
            RawValue::Str(s) => s.trim().parse().ok(),
            _ => None,
        }
    }

    fn dword(&self, name: &str) -> Option<u32> {
        match self.read(name) {
            RawValue::Dword(v) => Some(v),
            _ => None,
        }
    }

    pub fn theme(&self) -> Theme {
        match self.dword("Theme") {
            Some(1) => Theme::Light,
            Some(2) => Theme::Dark,
            _ => Theme::Default,
        }
    }

    pub fn set_theme(&self, v: Theme) -> std::io::Result<()> {
        self.write_dword("Theme", v as u32)
    }

    pub fn pos_x(&self) -> Option<i32> {
        self.number("PosX")
    }

    pub fn set_pos_x(&self, v: Option<i32>) -> std::io::Result<()> {
        self.write_str("PosX", &v.map(|n| n.to_string()).unwrap_or_default())
    }

    pub fn pos_y(&self) -> Option<i32> {
        self.number("PosY")
    }

    pub fn set_pos_y(&self, v: Option<i32>) -> std::io::Result<()> {
        self.write_str("PosY", &v.map(|n| n.to_string()).unwrap_or_default())
    }

    pub fn width(&self) -> u32 {
        self.number("Width").unwrap_or(1280)
    }

    pub fn set_width(&self, v: u32) -> std::io::Result<()> {
        self.write_str("Width", &v.to_string())
    }

    pub fn height(&self) -> u32 {
        self.number("Height").unwrap_or(768)
    }

    pub fn set_height(&self, v: u32) -> std::io::Result<()> {
        self.write_str("Height", &v.to_string())
    }

    pub fn window_state(&self) -> WindowState {
        match self.dword("State") {
            Some(1) => WindowState::Minimized,
            Some(2) => WindowState::Maximized,
            _ => WindowState::Normal,
        }
    }

    pub fn set_window_state(&self, v: WindowState) -> std::io::Result<()> {
        self.write_dword("State", v as u32)
    }

    pub fn last_update_check(&self) -> String {
        self.string("LastUpdateCheck")
    }

    pub fn set_last_update_check(&self, v: &str) -> std::io::Result<()> {
        self.write_str("LastUpdateCheck", v)
    }

    pub fn touch_last_update_check(&self) -> std::io::Result<()> {
        self.set_last_update_check(&chrono::Local::now().format(UPDATE_DATE_FORMAT).to_string())
    }

    pub fn update_interval(&self) -> String {
        self.string("UpdateInterval")
    }

    pub fn set_update_interval(&self, v: &str) -> std::io::Result<()> {
        self.write_str("UpdateInterval", v)
    }

    pub fn language(&self) -> String {
        self.string("Lang")
    }

    pub fn set_language(&self, v: &str) -> std::io::Result<()> {
        self.write_str("Lang", v)
    }

    pub fn jpeg_quality(&self) -> u8 {
        clamp_quality(self.read("JpegQuality"), JPEG_QUALITY_DEFAULT)
    }

    pub fn set_jpeg_quality(&self, v: u8) -> std::io::Result<()> {
        self.write_dword("JpegQuality", u32::from(v.clamp(1, 100)))
    }

    pub fn webp_quality(&self) -> u8 {
        clamp_quality(self.read("WebpQuality"), WEBP_QUALITY_DEFAULT)
    }

    pub fn set_webp_quality(&self, v: u8) -> std::io::Result<()> {
        self.write_dword("WebpQuality", u32::from(v.clamp(1, 100)))
    }

    pub fn file_assoc_stamp(&self) -> String {
        self.string("FileAssocStamp")
    }

    pub fn set_file_assoc_stamp(&self, v: &str) -> std::io::Result<()> {
        self.write_str("FileAssocStamp", v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_quality_ports_the_csharp_cases() {
        let s = |v: &str| RawValue::Str(v.to_string());
        assert_eq!(clamp_quality(s("50"), 100), 50);
        assert_eq!(clamp_quality(s("100"), 100), 100);
        assert_eq!(clamp_quality(s("1"), 100), 1);
        assert_eq!(clamp_quality(s("0"), 100), 1);
        assert_eq!(clamp_quality(s("150"), 100), 100);
        assert_eq!(clamp_quality(s("-5"), 100), 1);
        assert_eq!(clamp_quality(s("abc"), 100), 100);
        assert_eq!(clamp_quality(RawValue::Missing, 80), 80);
        assert_eq!(clamp_quality(RawValue::Dword(42), 100), 42);
    }
}
