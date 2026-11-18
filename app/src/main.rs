#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod frame;
mod geometry;
mod theme;

use slint::ComponentHandle;
use viewer_core::settings::{Settings, Theme as ThemeSetting, WindowState};

slint::include_modules!();

const PRODUCT_NAME: &str = "Image Viewer";

/// Same format as `MainWindow.UpdateTitle` in 1.x: "prefix - Image Viewer", or the bare product name.
fn window_title(prefix: Option<&str>) -> String {
    match prefix {
        Some(prefix) if !prefix.is_empty() => format!("{prefix} - {PRODUCT_NAME}"),
        _ => PRODUCT_NAME.to_string(),
    }
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
