//! User-locale date and time formatting (`DateTime.ToString(CultureInfo.CurrentCulture)` in 1.x).

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use windows::Win32::Foundation::{FILETIME, SYSTEMTIME};
use windows::Win32::Globalization::{DATE_SHORTDATE, GetDateFormatEx, GetTimeFormatEx, TIME_FORMAT_FLAGS};
use windows::Win32::System::Time::{FileTimeToSystemTime, SystemTimeToTzSpecificLocalTime};
use windows::core::PCWSTR;

/// Seconds between 1601-01-01 (FILETIME epoch) and 1970-01-01.
const UNIX_TO_FILETIME_SECS: u64 = 11_644_473_600;

fn to_local_system_time(t: SystemTime) -> Option<SYSTEMTIME> {
    let since_unix = t.duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO);
    let ticks = (since_unix.as_secs() + UNIX_TO_FILETIME_SECS) * 10_000_000 + u64::from(since_unix.subsec_nanos() / 100);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_timestamp_formats_with_its_year() {
        // 2024-06-15 12:00:00 UTC: the year is 2024 in every time zone.
        let text = format_file_time(UNIX_EPOCH + Duration::from_secs(1_718_452_800));
        assert!(text.contains("2024"), "{text}");
    }
}
