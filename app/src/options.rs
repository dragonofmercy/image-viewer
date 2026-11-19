//! Settings and About dialogs: the entries of the dropdowns and the wiring between the dialogs and `Settings`.

use crate::win::locale::format_file_time;
use crate::{AboutState, Commands, MainWindow, SettingsState, apply_theme};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::rc::Rc;
use viewer_core::culture::{AVAILABLE_LANGUAGES, Strings, native_name};
use viewer_core::settings::{Settings, Theme as ThemeSetting};

const REPOSITORY_URL: &str = "https://github.com/dragonofmercy/image-viewer";
const UPDATE_INTERVALS: [&str; 4] = ["day", "week", "month", ""];
const THEMES: [ThemeSetting; 3] = [ThemeSetting::Default, ThemeSetting::Light, ThemeSetting::Dark];

/// Uppercases the first character (`UcFirst` in 1.x): .NET native names of some cultures start in lower case.
fn uc_first(s: &str) -> String {
    let mut chars = s.chars();
    chars.next().map_or_else(String::new, |c| c.to_uppercase().chain(chars).collect())
}

/// (label, tag) of the language dropdown: the system default (empty tag), then the native names.
fn language_entries(system_label: &str) -> Vec<(String, String)> {
    let mut entries = vec![(system_label.to_string(), String::new())];
    entries.extend(AVAILABLE_LANGUAGES.iter().map(|tag| (uc_first(native_name(tag)), (*tag).to_string())));
    entries
}

/// Dropdown index of a stored language tag; 0 (system default) for an empty or unknown one.
fn index_of_language(tag: &str) -> usize {
    AVAILABLE_LANGUAGES.iter().position(|t| t.eq_ignore_ascii_case(tag)).map_or(0, |i| i + 1)
}

/// Unknown values read as manual, the last entry.
fn index_of_interval(value: &str) -> usize {
    UPDATE_INTERVALS.iter().position(|v| *v == value).unwrap_or(UPDATE_INTERVALS.len() - 1)
}

fn model(labels: impl IntoIterator<Item = String>) -> ModelRc<SharedString> {
    ModelRc::from(Rc::new(VecModel::from_iter(labels.into_iter().map(SharedString::from))))
}

fn save<E: std::fmt::Display>(what: &str, result: Result<(), E>) {
    if let Err(e) = result {
        eprintln!("cannot save the {what}: {e}");
    }
}

/// Opens the Settings and About dialogs on their commands (dropped while another dialog is up, as `ShowDialogAsync`) and saves each change at once.
pub fn install(ui: &MainWindow, strings: &Rc<Strings>) {
    let state = ui.global::<SettingsState>();
    let weak = ui.as_weak();
    let s = strings.clone();
    ui.global::<Commands>().on_settings(move || {
        let Some(ui) = weak.upgrade() else { return };
        if ui.get_dialog_open() {
            return;
        }
        let stored = Settings::app();
        ui.global::<SettingsState>().set_language_labels(model(language_entries(&s.get("DEFAULT_SYSTEM_LANGUAGE")).into_iter().map(|(label, _)| label)));
        ui.global::<SettingsState>().set_language_index(index_of_language(&stored.language()) as i32);
        ui.global::<SettingsState>().set_theme_index(THEMES.iter().position(|t| *t == stored.theme()).unwrap_or(0) as i32);
        ui.global::<SettingsState>().set_interval_index(index_of_interval(&stored.update_interval()) as i32);
        ui.global::<SettingsState>().set_jpeg_quality(i32::from(stored.jpeg_quality()));
        ui.global::<SettingsState>().set_webp_quality(i32::from(stored.webp_quality()));
        ui.set_settings_open(true);
    });
    state.on_language_chosen(|i| {
        let tag = language_entries("").get(i as usize).map(|(_, tag)| tag.clone()).unwrap_or_default();
        save("language", Settings::app().set_language(&tag));
    });
    let weak = ui.as_weak();
    state.on_theme_chosen(move |i| {
        let Some(theme) = THEMES.get(i as usize).copied() else { return };
        save("theme", Settings::app().set_theme(theme));
        if let Some(ui) = weak.upgrade() {
            apply_theme(&ui, theme);
        }
    });
    state.on_interval_chosen(|i| save("update interval", Settings::app().set_update_interval(UPDATE_INTERVALS.get(i as usize).copied().unwrap_or(""))));
    state.on_jpeg_quality_edited(|v| save("JPEG quality", Settings::app().set_jpeg_quality(v.clamp(1, 100) as u8)));
    state.on_webp_quality_edited(|v| save("WebP quality", Settings::app().set_webp_quality(v.clamp(1, 100) as u8)));

    let weak = ui.as_weak();
    let s = strings.clone();
    ui.global::<Commands>().on_about(move || {
        let Some(ui) = weak.upgrade() else { return };
        if ui.get_dialog_open() {
            return;
        }
        let last = Settings::app().last_update_check_time().map(format_file_time).filter(|t| !t.is_empty()).unwrap_or_else(|| s.get("ABOUT_LABEL_LAST_UPDATE_NEVER"));
        ui.global::<AboutState>().set_version(format!("v{}", env!("CARGO_PKG_VERSION")).into());
        ui.global::<AboutState>().set_last_check(format!("{}{last}", s.get("ABOUT_LABEL_LAST_UPDATE")).into());
        ui.set_about_open(true);
    });
    ui.global::<AboutState>().on_open_url(|release| {
        crate::win::shell::open_url(&if release { format!("{REPOSITORY_URL}/releases") } else { REPOSITORY_URL.to_string() });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_entries_start_with_the_system_default() {
        let entries = language_entries("System");
        assert_eq!(entries.len(), 9);
        assert_eq!(entries[0], ("System".to_string(), String::new()));
        assert_eq!(entries[1].1, "en-US");
        assert!(entries.iter().all(|(label, _)| !label.is_empty()));
    }

    #[test]
    fn language_index_defaults_to_system_and_ignores_case() {
        assert_eq!(index_of_language(""), 0);
        assert_eq!(index_of_language("xx-XX"), 0);
        assert_eq!(index_of_language("en-US"), 1);
        assert_eq!(index_of_language("FR-fr"), 2);
        assert_eq!(index_of_language("ru-RU"), 8);
    }

    #[test]
    fn interval_index_maps_stored_values() {
        assert_eq!(index_of_interval("day"), 0);
        assert_eq!(index_of_interval("month"), 2);
        assert_eq!(index_of_interval(""), 3);
        assert_eq!(index_of_interval("bogus"), 3);
    }
}
