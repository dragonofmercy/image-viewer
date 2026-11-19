//! Pure decision logic for command availability and the title prefix (`Context.UpdateButtonsAccessiblity` in 1.x).

#[derive(Debug, Clone, Default)]
pub struct Inputs {
    pub loaded: bool,
    pub loading: bool,
    pub saving: bool,
    pub cropping: bool,
    pub modified: bool,
    pub has_path: bool,
    pub folder_count: usize,
    pub fullscreen: bool,
    pub has_icon_sizes: bool,
    pub file_name: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Flags {
    pub image_commands: bool,
    pub save_direct: bool,
    pub file_info: bool,
    pub navigate: bool,
    pub icon_strip: bool,
}

/// The open cropper owns the whole surface: while it is up every image command is off. A running save turns them off too.
pub fn flags(i: &Inputs) -> Flags {
    let enabled = i.loaded && !i.cropping && !i.saving;
    Flags {
        image_commands: enabled,
        save_direct: enabled && i.modified,
        file_info: enabled && i.has_path,
        navigate: !i.cropping && !i.saving && i.folder_count > 1,
        icon_strip: enabled && !i.fullscreen && i.has_icon_sizes,
    }
}

/// Loading text while loading; otherwise the file name (or pasted-content text), with a bullet when modified; `None` when nothing is loaded.
pub fn title_prefix(i: &Inputs, loading_text: &str, pasted_text: &str) -> Option<String> {
    if i.loading {
        return Some(loading_text.to_string());
    }
    if !i.loaded {
        return None;
    }
    let name = i.file_name.as_deref().filter(|_| i.has_path).unwrap_or(pasted_text);
    Some(if i.modified { format!("\u{25CF} {name}") } else { name.to_string() })
}

/// What Escape closes (`Context.EscapeAction`, with the dialog in front).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Dialog,
    Cropper,
    InfoPane,
    Fullscreen,
    Nothing,
}

/// The first open layer, from the front: dialog, cropper, info pane, fullscreen. Never quits.
pub fn escape_target(dialog_open: bool, cropping: bool, info_open: bool, fullscreen: bool) -> Target {
    if dialog_open {
        Target::Dialog
    } else if cropping {
        Target::Cropper
    } else if info_open {
        Target::InfoPane
    } else if fullscreen {
        Target::Fullscreen
    } else {
        Target::Nothing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file() -> Inputs {
        Inputs {
            loaded: true,
            has_path: true,
            folder_count: 3,
            file_name: Some("a.png".into()),
            ..Inputs::default()
        }
    }

    #[test]
    fn nothing_loaded() {
        let i = Inputs::default();
        assert_eq!(
            flags(&i),
            Flags {
                image_commands: false,
                save_direct: false,
                file_info: false,
                navigate: false,
                icon_strip: false
            }
        );
        assert_eq!(title_prefix(&i, "L", "P"), None);
    }

    #[test]
    fn loaded_unmodified() {
        let f = flags(&file());
        assert!(f.image_commands && f.navigate && f.file_info && !f.save_direct && !f.icon_strip);
        assert_eq!(title_prefix(&file(), "L", "P").as_deref(), Some("a.png"));
    }

    #[test]
    fn modified() {
        let i = Inputs { modified: true, ..file() };
        assert!(flags(&i).save_direct);
        assert_eq!(title_prefix(&i, "L", "P").as_deref(), Some("\u{25CF} a.png"));
    }

    #[test]
    fn pasted() {
        let i = Inputs {
            loaded: true,
            modified: true,
            ..Inputs::default()
        };
        assert!(flags(&i).image_commands && !flags(&i).file_info);
        assert_eq!(title_prefix(&i, "L", "P").as_deref(), Some("\u{25CF} P"));
    }

    #[test]
    fn cropping_disables_everything() {
        let i = Inputs {
            cropping: true,
            modified: true,
            has_icon_sizes: true,
            ..file()
        };
        assert_eq!(
            flags(&i),
            Flags {
                image_commands: false,
                save_direct: false,
                file_info: false,
                navigate: false,
                icon_strip: false
            }
        );
    }

    #[test]
    fn saving_disables_everything_and_keeps_the_title() {
        let i = Inputs {
            saving: true,
            modified: true,
            has_icon_sizes: true,
            ..file()
        };
        assert_eq!(
            flags(&i),
            Flags {
                image_commands: false,
                save_direct: false,
                file_info: false,
                navigate: false,
                icon_strip: false
            }
        );
        assert_eq!(title_prefix(&i, "L", "P").as_deref(), Some("\u{25CF} a.png"));
    }

    #[test]
    fn single_file_cannot_navigate() {
        assert!(!flags(&Inputs { folder_count: 1, ..file() }).navigate);
    }

    #[test]
    fn icon_strip_hidden_in_fullscreen() {
        let i = Inputs { has_icon_sizes: true, ..file() };
        assert!(flags(&i).icon_strip);
        assert!(!flags(&Inputs { fullscreen: true, ..i }).icon_strip);
    }

    #[test]
    fn escape_backs_out_by_priority_and_never_quits() {
        use Target::*;
        assert_eq!(escape_target(true, true, true, true), Dialog);
        assert_eq!(escape_target(false, true, true, true), Cropper);
        assert_eq!(escape_target(false, false, true, true), InfoPane);
        assert_eq!(escape_target(false, false, false, true), Fullscreen);
        assert_eq!(escape_target(false, false, false, false), Nothing);
        assert_eq!(escape_target(true, false, false, false), Dialog);
        assert_eq!(escape_target(false, true, false, false), Cropper);
        assert_eq!(escape_target(false, false, true, false), InfoPane);
    }

    #[test]
    fn loading_title() {
        let i = Inputs { loading: true, ..Inputs::default() };
        assert_eq!(title_prefix(&i, "Loading", "P").as_deref(), Some("Loading"));
    }
}
