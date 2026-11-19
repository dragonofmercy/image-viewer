#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod controller;
mod crop;
mod document;
mod file_info;
mod frame;
mod geometry;
mod image_host;
mod install;
mod launch;
mod options;
mod theme;
mod ui_state;
mod updates;
mod viewport;
mod win {
    pub mod clipboard;
    pub mod dialogs;
    pub mod locale;
    pub mod print;
    pub mod shell;
    pub mod toast;
}

use slint::ComponentHandle;
use slint::platform::WindowEvent;
use std::cell::RefCell;
use std::rc::Rc;
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
pub(crate) fn system_ui_languages() -> Vec<String> {
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

/// Saves the theme the user chose and applies it.
pub(crate) fn choose_theme(ui: &MainWindow, setting: ThemeSetting) {
    if let Err(e) = Settings::app().set_theme(setting) {
        eprintln!("cannot save the theme: {e}");
    }
    apply_theme(ui, setting);
}

fn main() -> Result<(), slint::PlatformError> {
    install::run_hooks();
    let default_backend = || slint::BackendSelector::new().backend_name("winit".into()).renderer_name("skia-software".into()).select();
    // An unforced selector lets Slint read SLINT_BACKEND (for example winit-skia-software).
    if std::env::var_os("SLINT_BACKEND").is_some_and(|v| !v.is_empty()) {
        if let Err(e) = slint::BackendSelector::new().select() {
            eprintln!("SLINT_BACKEND selection failed, using winit-skia-software: {e}");
            default_backend()?;
        }
    } else {
        default_backend()?;
    }

    let ui = MainWindow::new()?;
    ui.set_title_text(window_title(None).into());
    let settings = Settings::app();
    apply_theme(&ui, settings.theme());
    // Swapped by the Settings dialog when the language changes (options.rs).
    let strings = Rc::new(RefCell::new(Strings::load(resolve_language(&settings.language(), &system_ui_languages()))));
    let tr = strings.clone();
    ui.global::<Tr>().on_lookup(move |key, _| tr.borrow().get(&key).into());
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
    // Same path as the caption close button, so the geometry is flushed.
    let weak = ui.as_weak();
    ui.global::<Commands>().on_quit(move || {
        if let Some(ui) = weak.upgrade() {
            ui.window().dispatch_event(WindowEvent::CloseRequested);
        }
    });
    let weak = ui.as_weak();
    ui.global::<Commands>().on_theme_light(move || {
        if let Some(ui) = weak.upgrade() {
            choose_theme(&ui, ThemeSetting::Light);
        }
    });
    let weak = ui.as_weak();
    ui.global::<Commands>().on_theme_dark(move || {
        if let Some(ui) = weak.upgrade() {
            choose_theme(&ui, ThemeSetting::Dark);
        }
    });
    // Shown here so the native window exists: the frame hides it until its first frame is painted (no white flash).
    ui.show()?;
    // Off the UI thread, after the window is up (as 1.x does after the first frame).
    std::thread::spawn(install::ensure_associations);
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
    let controller = controller::Controller::new(&ui, image_host::ImageHost::new(&ui), strings.clone());
    let c = controller.clone();
    let (weak, s) = (ui.as_weak(), strings.clone());
    let updates = updates::Updates::new(move |state| {
        if let Some(ui) = weak.upgrade() {
            options::show_update_state(&ui, &s.borrow(), &state);
        }
    });
    options::install(&ui, &strings, &updates, move || c.language_changed());
    let c = controller.clone();
    ui.window().on_close_requested(move || {
        // Closing now would lose the edit being written: the controller closes the window once the save succeeded.
        if c.defer_close() {
            return slint::CloseRequestResponse::KeepWindowShown;
        }
        c.stop_animation();
        frame::flush_geometry();
        win::toast::clear();
        slint::CloseRequestResponse::HideWindow
    });
    // Paths may be relative and non-ASCII: kept as OsString, made absolute by the document.
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let launch = launch::parse_launch(&args);
    let start_update = matches!(launch, launch::Launch::Update);
    let from_toast = matches!(launch, launch::Launch::Update | launch::Launch::About);
    if let launch::Launch::File(path) = launch {
        controller.open_startup(path);
    }
    // Once the window is up: the background check (toast when an update exists), and the About download when launched from the toast.
    let (weak, s, u) = (ui.as_weak(), strings.clone(), updates.clone());
    slint::Timer::single_shot(std::time::Duration::ZERO, move || {
        let s2 = s.clone();
        // A toast launch skips the quiet check: it would overwrite the state of the About dialog opened below (and show a second toast).
        if !from_toast {
            u.check_if_due(move || {
                let tr = |key: &str| s2.borrow().get(key);
                win::toast::show_update(&tr("ABOUT_UPDATE_INFO_UPDATE_AVAILABLE"), &tr("ABOUT_BTN_DOWNLOAD_UPDATE"));
            });
        }
        if from_toast && let Some(ui) = weak.upgrade() {
            options::show_about(&ui, &s.borrow(), &u, start_update);
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
