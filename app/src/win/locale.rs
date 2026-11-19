//! User-locale date and time formatting (`DateTime.ToString(CultureInfo.CurrentCulture)` in 1.x).

use std::time::{SystemTime, UNIX_EPOCH};

use windows::Win32::Foundation::{FILETIME, SYSTEMTIME};
use windows::Win32::Globalization::{DATE_SHORTDATE, GetDateFormatEx, GetTimeFormatEx, TIME_FORMAT_FLAGS};
use windows::Win32::System::Time::{FileTimeToSystemTime, SystemTimeToTzSpecificLocalTime};
use windows::core::PCWSTR;

/// Seconds between 1601-01-01 (FILETIME epoch) and 1970-01-01.
const UNIX_TO_FILETIME_SECS: u64 = 11_644_473_600;

fn to_local_system_time(t: SystemTime) -> Option<SYSTEMTIME> {
    let epoch_ticks = i128::from(UNIX_TO_FILETIME_SECS) * 10_000_000;
    let ticks = match t.duration_since(UNIX_EPOCH) {
        Ok(d) => epoch_ticks + (d.as_nanos() / 100) as i128,
        Err(e) => epoch_ticks - (e.duration().as_nanos() / 100) as i128,
    };
    // Before 1601 there is no FILETIME (Windows SystemTime cannot even hold such a time, so this is a safety net).
    let ticks = u64::try_from(ticks).ok()?;
    let file_time = FILETIME {
        dwLowDateTime: ticks as u32,
        dwHighDateTime: (ticks >> 32) as u32,
    };
    let (mut utc, mut local) = (SYSTEMTIME::default(), SYSTEMTIME::default());
    unsafe {
        FileTimeToSystemTime(&file_time, &mut utc).ok()?;
        SystemTimeToTzSpecificLocalTime(None, &utc, &mut local).ok()?;
    }
    Some(local)
}

fn wide_to_string(buf: &[u16], written: i32) -> String {
    // The count includes the terminating NUL; 0 means failure.
    let len = usize::try_from(written).unwrap_or(0).saturating_sub(1);
    String::from_utf16_lossy(&buf[..len.min(buf.len())])
}

/// Short date and long time of the user's locale in local time, joined by a space. Empty when the conversion fails.
pub fn format_file_time(t: SystemTime) -> String {
    let Some(local) = to_local_system_time(t) else { return String::new() };
    // A null locale name is LOCALE_NAME_USER_DEFAULT.
    let locale = PCWSTR::null();
    let mut date = [0u16; 128];
    let mut time = [0u16; 128];
    let (d, t) = unsafe {
        (
            GetDateFormatEx(locale, DATE_SHORTDATE, Some(&local), PCWSTR::null(), Some(&mut date), PCWSTR::null()),
            GetTimeFormatEx(locale, TIME_FORMAT_FLAGS(0), Some(&local), PCWSTR::null(), Some(&mut time)),
        )
    };
    format!("{} {}", wide_to_string(&date, d), wide_to_string(&time, t)).trim().to_string()
}

/// `DateTime.Now.ToString("yyyy-MM-dd-hh-mm-ss")` in local time, 12-hour clock as in 1.x: the Save As name suggested for pasted content.
pub fn save_stamp(t: SystemTime) -> String {
    to_local_system_time(t).map(|local| stamp(&local)).unwrap_or_default()
}

fn stamp(t: &SYSTEMTIME) -> String {
    let hour = match t.wHour % 12 {
        0 => 12,
        h => h,
    };
    format!("{:04}-{:02}-{:02}-{hour:02}-{:02}-{:02}", t.wYear, t.wMonth, t.wDay, t.wMinute, t.wSecond)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn stamp_uses_a_12_hour_clock() {
        let at = |h| SYSTEMTIME {
            wYear: 2024,
            wMonth: 6,
            wDay: 5,
            wHour: h,
            wMinute: 7,
            wSecond: 9,
            ..Default::default()
        };
        assert_eq!(stamp(&at(0)), "2024-06-05-12-07-09");
        assert_eq!(stamp(&at(9)), "2024-06-05-09-07-09");
        assert_eq!(stamp(&at(13)), "2024-06-05-01-07-09");
    }

    #[test]
    fn known_timestamp_formats_to_digits() {
        // The short date may use a two-digit year or a non-Gregorian calendar: only digits are certain.
        let text = format_file_time(UNIX_EPOCH + Duration::from_secs(1_718_452_800));
        assert!(text.chars().any(|c| c.is_ascii_digit()), "{text}");
    }

    #[test]
    fn dates_before_1970_are_not_clamped() {
        let text = format_file_time(UNIX_EPOCH - Duration::from_secs(86_400 * 365));
        assert!(text.contains("1969") && !text.contains("1970"), "{text}");
    }
}
