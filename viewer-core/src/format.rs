/// Format a byte count as B/KB/MB/GB/TB with at most one decimal ("0.#" in the C# original).
pub fn humanize_bytes(mut bytes: f64) -> String {
    const SIZES: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut order = 0;
    while bytes >= 1024.0 && order < SIZES.len() - 1 {
        order += 1;
        bytes /= 1024.0;
    }
    let number = format!("{bytes:.1}");
    format!("{} {}", number.strip_suffix(".0").unwrap_or(&number), SIZES[order])
}

/// Round to the nearest integer (ties to even, like .NET Math.Round), then truncate to a multiple of ten.
pub fn round_to_ten(input: f64) -> f32 {
    ((input.round_ties_even() / 10.0) as i32 * 10) as f32
}

/// Upper-case the first character.
pub fn uc_first(s: &str) -> String {
    let mut chars = s.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

/// Normalize a stored "yyyy-MM-dd HH:mm:ss" update date. Returns an empty string when the value is not a valid date, so the caller shows its localized "never" label.
// shortcut: the C# version formats with the current culture; this crate has no locale support, so the date comes back in its stored form.
pub fn to_update_date(raw: &str) -> String {
    if is_valid_update_date(raw) { raw.to_string() } else { String::new() }
}

fn is_valid_update_date(raw: &str) -> bool {
    let b = raw.as_bytes();
    if b.len() != 19 || !raw.is_ascii() {
        return false;
    }
    let separators = [(4, b'-'), (7, b'-'), (10, b' '), (13, b':'), (16, b':')];
    if separators.iter().any(|&(i, c)| b[i] != c) {
        return false;
    }
    let field = |range: std::ops::Range<usize>| {
        let digits = &raw[range];
        if digits.bytes().all(|c| c.is_ascii_digit()) { digits.parse::<u32>().ok() } else { None }
    };
    let (Some(year), Some(month), Some(day), Some(hour), Some(minute), Some(second)) = (field(0..4), field(5..7), field(8..10), field(11..13), field(14..16), field(17..19)) else {
        return false;
    };
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => 28 + u32::from(leap),
        _ => return false,
    };
    year >= 1 && (1..=days).contains(&day) && hour < 24 && minute < 60 && second < 60
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn humanize_bytes_units() {
        let cases = [
            (0.0, "0 B"),
            (1.0, "1 B"),
            (1023.0, "1023 B"),
            (1024.0, "1 KB"),
            (1048576.0, "1 MB"),
            (1073741824.0, "1 GB"),
            (1099511627776.0, "1 TB"),
            (1125899906842624.0, "1024 TB"),
            (1536.0, "1.5 KB"),
        ];
        for (input, expected) in cases {
            assert_eq!(humanize_bytes(input), expected, "input {input}");
        }
    }

    #[test]
    fn round_to_ten_cases() {
        let cases = [(0.0, 0.0), (5.0, 0.0), (12.0, 10.0), (15.0, 10.0), (19.0, 10.0), (25.0, 20.0), (99.0, 90.0), (100.0, 100.0)];
        for (input, expected) in cases {
            assert_eq!(round_to_ten(input), expected, "input {input}");
        }
    }

    #[test]
    fn uc_first_cases() {
        assert_eq!(uc_first("hello"), "Hello");
        assert_eq!(uc_first(""), "");
    }

    #[test]
    fn to_update_date_valid_and_invalid() {
        assert_eq!(to_update_date("2026-11-18 09:05:03"), "2026-11-18 09:05:03");
        assert_eq!(to_update_date("garbage"), "");
        assert_eq!(to_update_date("2026-13-18 09:05:03"), "");
    }
}
