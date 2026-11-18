/// Format a byte count as B/KB/MB/GB/TB with at most one decimal ("0.#" in the C# original).
pub fn humanize_bytes(mut bytes: f64) -> String {
    const SIZES: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut order = 0;
    while bytes >= 1024.0 && order < SIZES.len() - 1 {
        order += 1;
        bytes /= 1024.0;
    }
    // Round half away from zero like .NET "0.#"; Rust's {:.1} rounds the binary value half to even.
    let number = format!("{:.1}", (bytes * 10.0).round() / 10.0);
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
            (1280.0, "1.3 KB"),
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
}
