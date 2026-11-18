#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod frame;
mod geometry;
mod theme;

use slint::ComponentHandle;
use viewer_core::culture::{Strings, resolve_language};
use viewer_core::settings::{Settings, Theme as ThemeSetting, WindowState};
use windows::Win32::Globalization::{GetUserPreferredUILanguages, MUI_LANGUAGE_NAME};
use windows::core::PWSTR;

slint::include_modules!();

const PRODUCT_NAME: &str = "Image Viewer";

/// Same format as `MainWindow.UpdateTitle` in 1.x: "prefix - Image Viewer", or the bare product name.
fn window_title(prefix: Option<&str>) -> String {
    match prefix {
        Some(prefix) if !prefix.is_empty() => format!("{prefix} - {PRODUCT_NAME}"),
        _ => PRODUCT_NAME.to_string(),
    }
}

/// UI language tags in preference order (BCP-47), empty when the query fails.
fn system_ui_languages() -> Vec<String> {
    let mut count = 0u32;
    let mut len = 0u32;
    if unsafe { GetUserPreferredUILanguages(MUI_LANGUAGE_NAME, &mut count, None, &mut len) }.is_err() || len == 0 {
        return Vec::new();
    }
    let mut buf = vec![0u16; len as usize];
    if unsafe { GetUserPreferredUILanguages(MUI_LANGUAGE_NAME, &mut count, Some(PWSTR(buf.as_mut_ptr())), &mut len) }.is_err() {
        return Vec::new();
    }
    // Double-NUL terminated list of NUL separated tags.
    buf.split(|&c| c == 0).take_while(|tag| !tag.is_empty()).map(String::from_utf16_lossy).collect()
}

/// Sets the palette, the accent and the native title bar mode from the setting (Default follows the OS).
fn apply_theme(ui: &MainWindow, setting: ThemeSetting) {
    let dark = theme::resolve(setting, theme::os_is_dark());
    let global = ui.global::<Theme>();
    global.set_dark(dark);
    global.set_accent(theme::accent(dark));
    if let Some(hwnd) = frame::hwnd_of(ui.window()) {
        frame::set_dark_title(hwnd, dark);
    }
}

fn main() -> Result<(), slint::PlatformError> {
    slint::BackendSelector::new().backend_name("winit".into()).renderer_name("skia".into()).select()?;

    let ui = MainWindow::new()?;
    ui.set_title_text(window_title(None).into());
    let settings = Settings::app();
    apply_theme(&ui, settings.theme());
    // The language is fixed for the session: a change applies at next start, as in 1.x.
    let strings = Strings::load(resolve_language(&settings.language(), &system_ui_languages()));
    ui.global::<Tr>().on_t(move |key| strings.get(&key).into());
    let saved = geometry::load(&settings);
    geometry::apply(ui.window(), &saved);
    // As 1.x: a saved Minimized state comes back as Normal.
    let state = if saved.state == WindowState::Maximized { WindowState::Maximized } else { WindowState::Normal };
    if state == WindowState::Normal
        && let Err(e) = settings.set_window_state(state)
    {
        eprintln!("cannot save the window state: {e}");
    }
    frame::track_geometry(geometry::Tracker::new(state), settings);
    ui.window().on_close_requested(|| {
        frame::flush_geometry();
        slint::CloseRequestResponse::HideWindow
    });
    let weak = ui.as_weak();
    frame::install_when_ready(&ui, move |_| {
        if let Some(ui) = weak.upgrade() {
            apply_theme(&ui, Settings::app().theme());
        }
    });
    let weak = ui.as_weak();
    frame::on_os_theme_changed(move || {
        let setting = Settings::app().theme();
        if setting == ThemeSetting::Default
            && let Some(ui) = weak.upgrade()
        {
            apply_theme(&ui, setting);
        }
    });
    ui.run()
}

#[cfg(test)]
mod tests {
    use super::window_title;

    #[test]
    fn title_matches_update_title_format() {
        assert_eq!(window_title(None), "Image Viewer");
        assert_eq!(window_title(Some("")), "Image Viewer");
        assert_eq!(window_title(Some("photo.jpg")), "photo.jpg - Image Viewer");
    }
}
