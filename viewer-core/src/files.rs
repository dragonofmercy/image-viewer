use std::cmp::Ordering;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

pub const SUPPORTED_FILE_TYPES: [&str; 15] = [".jpg", ".jpeg", ".bmp", ".png", ".gif", ".tif", ".tiff", ".tga", ".ico", ".webp", ".svg", ".jxl", ".avif", ".heic", ".heif"];
pub const SAVE_FILE_TYPES: [&str; 8] = [".jpg", ".png", ".webp", ".bmp", ".gif", ".tiff", ".tga", ".jxl"];

/// Lower-case extension with its leading dot, or None when the path has none.
pub fn extension_of(path: &Path) -> Option<String> {
    path.extension().map(|e| format!(".{}", e.to_string_lossy().to_ascii_lowercase()))
}

pub fn is_supported(path: &Path) -> bool {
    extension_of(path).is_some_and(|e| SUPPORTED_FILE_TYPES.contains(&e.as_str()))
}

/// Canonical save type: lower-case, jpeg -> jpg, tif -> tiff.
pub fn normalize_extension(ext: &str) -> String {
    match ext.to_ascii_lowercase().as_str() {
        ".jpeg" => ".jpg".to_string(),
        ".tif" => ".tiff".to_string(),
        other => other.to_string(),
    }
}

/// Explorer-style natural order (shlwapi StrCmpLogicalW).
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let wide = |s: &str| std::ffi::OsStr::new(s).encode_wide().chain(Some(0)).collect::<Vec<u16>>();
    let (a, b) = (wide(a), wide(b));
    // SAFETY: both buffers are NUL-terminated and live for the duration of the call.
    let r = unsafe { windows::Win32::UI::Shell::StrCmpLogicalW(windows::core::PCWSTR(a.as_ptr()), windows::core::PCWSTR(b.as_ptr())) };
    r.cmp(&0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_extension_maps_aliases() {
        for (input, expected) in [
            (".jpg", ".jpg"),
            (".JPG", ".jpg"),
            (".jpeg", ".jpg"),
            (".JPEG", ".jpg"),
            (".tif", ".tiff"),
            (".TIF", ".tiff"),
            (".tiff", ".tiff"),
            (".png", ".png"),
            (".PNG", ".png"),
            (".webp", ".webp"),
        ] {
            assert_eq!(normalize_extension(input), expected, "input {input}");
        }
    }

    #[test]
    fn is_supported_checks_extension_case_insensitively() {
        assert!(is_supported(Path::new("a.JPG")));
        assert!(!is_supported(Path::new("a.txt")));
        assert!(!is_supported(Path::new("noext")));
    }

    #[test]
    fn extension_of_is_lowercase_with_dot() {
        assert_eq!(extension_of(Path::new("x/A.PnG")), Some(".png".to_string()));
        assert_eq!(extension_of(Path::new("x/noext")), None);
    }

    #[test]
    fn natural_cmp_orders_img2_before_img10() {
        assert_eq!(natural_cmp("img2", "img10"), Ordering::Less);
    }

    #[test]
    fn natural_sort_produces_explorer_order() {
        let mut files = vec!["img10.jpg", "img2.jpg", "img1.jpg"];
        files.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(files, ["img1.jpg", "img2.jpg", "img10.jpg"]);
    }
}
