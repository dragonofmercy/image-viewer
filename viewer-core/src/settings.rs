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

    /// The stored `yyyy-MM-dd HH:mm:ss` (invariant) as a local time; None when empty or unparsable (1.x `ToUpdateDate`).
    pub fn last_update_check_time(&self) -> Option<std::time::SystemTime> {
        use chrono::TimeZone;
        parse_update_date(&self.last_update_check()).and_then(|t| chrono::Local.from_local_datetime(&t).earliest()).map(std::time::SystemTime::from)
    }

    pub fn set_last_update_check(&self, v: &str) -> std::io::Result<()> {
        self.write_str("LastUpdateCheck", v)
    }

    pub fn touch_last_update_check(&self) -> std::io::Result<()> {
        self.set_last_update_check(&chrono::Local::now().format(UPDATE_DATE_FORMAT).to_string())
    }

    /// Whether the background update check is due now (local time), from the stored interval and last check.
    pub fn update_check_due(&self) -> bool {
        check_due(&self.update_interval(), parse_update_date(&self.last_update_check()), chrono::Local::now().naive_local())
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

/// Port of `UpdateService.CheckUpdate`: "day" = +1 day, "week" = +7 days, any other non-empty value = +1 calendar month, "" = manual only.
/// Compared on calendar dates; never checked (or unparsable) is due; a clock moved back waits.
pub fn check_due(interval: &str, last: Option<chrono::NaiveDateTime>, now: chrono::NaiveDateTime) -> bool {
    if interval.is_empty() {
        return false;
    }
    let Some(last) = last else { return true };
    let next = match interval {
        "day" => last.checked_add_days(chrono::Days::new(1)),
        "week" => last.checked_add_days(chrono::Days::new(7)),
        _ => last.checked_add_months(chrono::Months::new(1)),
    };
    next.is_some_and(|n| n.date() <= now.date())
}

fn parse_update_date(value: &str) -> Option<chrono::NaiveDateTime> {
    chrono::NaiveDateTime::parse_from_str(value, UPDATE_DATE_FORMAT).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_date_parses_the_stored_format_only() {
        let parsed = parse_update_date("2026-03-04 05:06:07").unwrap();
        assert_eq!(parsed.format(UPDATE_DATE_FORMAT).to_string(), "2026-03-04 05:06:07");
        assert!(parse_update_date("").is_none());
        assert!(parse_update_date("garbage").is_none());
        assert!(parse_update_date("04/03/2026 05:06:07").is_none());
    }

    #[test]
    fn update_check_schedule() {
        let at = |s: &str| chrono::NaiveDateTime::parse_from_str(s, UPDATE_DATE_FORMAT).unwrap();
        let now = at("2026-11-19 10:00:00");
        assert!(!check_due("", None, now)); // manual: never
        assert!(check_due("day", None, now)); // never checked, or unparsable
        assert!(!check_due("day", Some(at("2026-11-19 00:00:01")), now)); // next day not reached
        assert!(check_due("day", Some(at("2026-11-18 23:59:59")), now)); // calendar dates, not 24 h
        assert!(!check_due("week", Some(at("2026-11-13 09:00:00")), now));
        assert!(check_due("week", Some(at("2026-11-12 09:00:00")), now));
        assert!(!check_due("month", Some(at("2026-10-20 09:00:00")), now));
        assert!(check_due("month", Some(at("2026-10-19 09:00:00")), now));
        assert!(check_due("bogus", Some(at("2026-10-19 09:00:00")), now)); // unknown value = month
        assert!(!check_due("day", Some(at("2027-01-01 00:00:00")), now)); // clock moved back: wait
        assert!(check_due("month", Some(at("2026-01-31 09:00:00")), at("2026-02-28 09:00:00"))); // Jan 31 + 1 month clamps to Feb 28
    }

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
