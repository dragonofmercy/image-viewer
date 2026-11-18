// Theme resolution (setting + OS) and the Windows accent palette. The brushes themselves live in ui/theme.slint.

use slint::Color;
use viewer_core::settings::Theme as ThemeSetting;
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_BINARY, RRF_RT_REG_DWORD, RegGetValueW};
use windows::core::{PCWSTR, w};

/// true = dark. `Default` follows the OS.
pub fn resolve(setting: ThemeSetting, os_is_dark: bool) -> bool {
    match setting {
        ThemeSetting::Default => os_is_dark,
        ThemeSetting::Light => false,
        ThemeSetting::Dark => true,
    }
}

/// Reads a registry value of the given type under HKCU into a fixed buffer; returns the bytes read.
fn read_hkcu(subkey: PCWSTR, value: PCWSTR, flags: windows::Win32::System::Registry::REG_ROUTINE_FLAGS, buf: &mut [u8]) -> Option<usize> {
    let mut size = buf.len() as u32;
    let status = unsafe { RegGetValueW(HKEY_CURRENT_USER, subkey, value, flags, None, Some(buf.as_mut_ptr().cast()), Some(&mut size)) };
    status.is_ok().then_some(size as usize)
}

/// `AppsUseLightTheme` = 0 means dark; a missing value means light.
pub fn os_is_dark() -> bool {
    let mut buf = [0u8; 4];
    read_hkcu(w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"), w!("AppsUseLightTheme"), RRF_RT_REG_DWORD, &mut buf) == Some(4) && u32::from_le_bytes(buf) == 0
}

/// Light3, Light2, Light1, Base, Dark1, Dark2, Dark3 from 8 RGBA entries; `None` when the buffer is shorter than 7 entries.
pub fn parse_accent_palette(bytes: &[u8]) -> Option<[Color; 7]> {
    let entries = bytes.get(..28)?;
    let mut colors = [Color::from_rgb_u8(0, 0, 0); 7];
    for (color, rgba) in colors.iter_mut().zip(entries.as_chunks::<4>().0) {
        *color = Color::from_rgb_u8(rgba[0], rgba[1], rgba[2]);
    }
    Some(colors)
}

pub fn accent_palette() -> Option<[Color; 7]> {
    let mut buf = [0u8; 32];
    let len = read_hkcu(w!(r"Software\Microsoft\Windows\CurrentVersion\Explorer\Accent"), w!("AccentPalette"), RRF_RT_REG_BINARY, &mut buf)?;
    parse_accent_palette(&buf[..len])
}

/// Accent used by controls: palette Light2 on dark, Dark1 on light; fixed fallbacks when the palette is unavailable.
pub fn accent(dark: bool) -> Color {
    match (accent_palette(), dark) {
        (Some(p), true) => p[1],
        (Some(p), false) => p[4],
        (None, true) => Color::from_rgb_u8(0x60, 0xcd, 0xff),
        (None, false) => Color::from_rgb_u8(0x00, 0x5f, 0xb8),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_follows_setting_then_os() {
        assert!(resolve(ThemeSetting::Default, true));
        assert!(!resolve(ThemeSetting::Default, false));
        assert!(!resolve(ThemeSetting::Light, true));
        assert!(resolve(ThemeSetting::Dark, false));
    }

    #[test]
    fn palette_parses_seven_colors() {
        let bytes: Vec<u8> = (0..8u8).flat_map(|i| [i, i + 10, i + 20, 255]).collect();
        let p = parse_accent_palette(&bytes).unwrap();
        assert_eq!(p[0], Color::from_rgb_u8(0, 10, 20));
        assert_eq!(p[3], Color::from_rgb_u8(3, 13, 23));
        assert_eq!(p[6], Color::from_rgb_u8(6, 16, 26));
        assert!(parse_accent_palette(&bytes[..27]).is_none());
        assert!(parse_accent_palette(&[]).is_none());
    }
}
