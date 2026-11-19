//! Settings and About dialogs: the entries of the dropdowns and the wiring between the dialogs and `Settings`.

use crate::install::REPOSITORY_URL;
use crate::win::locale::format_file_time;
use crate::{AboutState, Commands, MainWindow, SettingsState, Tr, choose_theme};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::rc::Rc;
use viewer_core::culture::{AVAILABLE_LANGUAGES, Strings, index_of, native_name, resolve_language};
use viewer_core::format::uc_first;
use viewer_core::settings::{Settings, Theme as ThemeSetting};

const UPDATE_INTERVALS: [&str; 4] = ["day", "week", "month", ""];
const THEMES: [ThemeSetting; 3] = [ThemeSetting::Default, ThemeSetting::Light, ThemeSetting::Dark];

/// (label, tag) of the language dropdown: the system default (empty tag), then the native names.
fn language_entries(system_label: &str) -> Vec<(String, String)> {
    let mut entries = vec![(system_label.to_string(), String::new())];
    entries.extend(AVAILABLE_LANGUAGES.iter().map(|tag| (uc_first(native_name(tag)), (*tag).to_string())));
    entries
}

/// Dropdown index of a stored language tag; 0 (system default) for an empty or unknown one.
fn index_of_language(tag: &str) -> usize {
    index_of(tag).map_or(0, |i| i + 1)
}

/// Unknown values read as manual, the last entry.
fn index_of_interval(value: &str) -> usize {
    UPDATE_INTERVALS.iter().position(|v| *v == value).unwrap_or(UPDATE_INTERVALS.len() - 1)
}

/// Labels of the language dropdown in the current language.
fn language_labels(strings: &Strings) -> ModelRc<SharedString> {
    model(language_entries(&strings.get("DEFAULT_SYSTEM_LANGUAGE")).into_iter().map(|(label, _)| label))
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
/// A language change applies at once: the strings are reloaded, every `Tr.t` binding re-evaluates and `language_changed` runs.
pub fn install(ui: &MainWindow, strings: &Rc<RefCell<Strings>>, language_changed: impl Fn() + 'static) {
    let state = ui.global::<SettingsState>();
    let weak = ui.as_weak();
    let s = strings.clone();
    ui.global::<Commands>().on_settings(move || {
        let Some(ui) = weak.upgrade() else { return };
        if ui.get_dialog_open() {
            return;
        }
        let stored = Settings::app();
        let state = ui.global::<SettingsState>();
        state.set_language_labels(language_labels(&s.borrow()));
        state.set_language_index(index_of_language(&stored.language()) as i32);
        state.set_theme_index(THEMES.iter().position(|t| *t == stored.theme()).unwrap_or(0) as i32);
        state.set_interval_index(index_of_interval(&stored.update_interval()) as i32);
        state.set_jpeg_quality(i32::from(stored.jpeg_quality()));
        state.set_webp_quality(i32::from(stored.webp_quality()));
        ui.set_settings_open(true);
    });
    let weak = ui.as_weak();
    let s = strings.clone();
    let language_changed = Rc::new(language_changed);
    state.on_language_chosen(move |i| {
        // Entry 0 is the system default, stored as an empty tag.
        let tag = usize::try_from(i).ok().and_then(|i| i.checked_sub(1)).and_then(|i| AVAILABLE_LANGUAGES.get(i)).map(|tag| (*tag).to_string()).unwrap_or_default();
        save("language", Settings::app().set_language(&tag));
        // Deferred: this runs inside the dropdown row that is about to be rebuilt with the new labels.
        let (weak, s, language_changed) = (weak.clone(), s.clone(), language_changed.clone());
        slint::Timer::single_shot(std::time::Duration::ZERO, move || {
            let Some(ui) = weak.upgrade() else { return };
            *s.borrow_mut() = Strings::load(resolve_language(&tag, &crate::system_ui_languages()));
            let tr = ui.global::<Tr>();
            tr.set_revision(tr.get_revision().wrapping_add(1));
            ui.global::<SettingsState>().set_language_labels(language_labels(&s.borrow()));
            language_changed();
        });
    });
    let weak = ui.as_weak();
    state.on_theme_chosen(move |i| {
        if let (Some(theme), Some(ui)) = (THEMES.get(i as usize).copied(), weak.upgrade()) {
            choose_theme(&ui, theme);
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
        let last = Settings::app().last_update_check_time().map(format_file_time).filter(|t| !t.is_empty()).unwrap_or_else(|| s.borrow().get("ABOUT_LABEL_LAST_UPDATE_NEVER"));
        ui.global::<AboutState>().set_version(format!("v{}", env!("CARGO_PKG_VERSION")).into());
        ui.global::<AboutState>().set_last_check(format!("{}{last}", s.borrow().get("ABOUT_LABEL_LAST_UPDATE")).into());
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
