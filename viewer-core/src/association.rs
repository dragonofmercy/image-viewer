//! Windows file-association layout and registry service, byte-compatible with the C# app (same keys, values and stamp).
//!
//! The app only ever proposes itself through OpenWithProgids. It never writes the (Default) value of an extension key,
//! so it never steals another application's default handler.
use std::io;
use std::path::Path;

use winreg::RegKey;
use winreg::enums::*;

use crate::settings::Settings;

/// Bump whenever the shape of the keys themselves changes. The stamp stops matching and every install re-registers once.
/// Adding or removing a supported extension does not need a bump: the extension set is part of the stamp.
pub const LAYOUT_VERSION: u32 = 1;

/// One registry value to write under HKEY_CURRENT_USER. `None` targets the key's (Default) value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegistryEntry {
    pub key_path: String,
    pub value_name: Option<String>,
    pub value: String,
}

fn entry(key_path: &str, value_name: Option<&str>, value: &str) -> RegistryEntry {
    RegistryEntry {
        key_path: key_path.to_string(),
        value_name: value_name.map(str::to_string),
        value: value.to_string(),
    }
}

pub struct FileAssociationPlan {
    exe_path: String,
    extensions: Vec<String>,
}

impl FileAssociationPlan {
    pub fn new(exe_path: &str, extensions: &[&str]) -> Self {
        Self {
            exe_path: exe_path.to_string(),
            extensions: extensions.iter().map(|e| e.to_string()).collect(),
        }
    }

    pub fn expected_command(&self) -> String {
        format!("\"{}\" \"%1\"", self.exe_path)
    }

    pub fn icon_value(&self) -> String {
        format!("{},0", self.exe_path)
    }

    pub fn application_key_path(&self) -> String {
        application_key_path_for(file_name_of(&self.exe_path))
    }

    /// Layout version + executable path + extension set.
    pub fn stamp(&self) -> String {
        format!("{LAYOUT_VERSION}|{}|{}", self.exe_path, self.extensions.join(";"))
    }

    pub fn entries_to_write(&self) -> Vec<RegistryEntry> {
        let mut entries = Vec::new();
        for ext in &self.extensions {
            let prog_id_key = prog_id_key_path(ext);
            entries.push(entry(&prog_id_key, None, label_for(ext)));
            entries.push(entry(&format!(r"{prog_id_key}\DefaultIcon"), None, &self.icon_value()));
            entries.push(entry(&format!(r"{prog_id_key}\shell\open\command"), None, &self.expected_command()));
            entries.push(entry(&open_with_progids_key_path(ext), Some(&prog_id_for(ext)), ""));
        }

        // The Applications block is what puts the app in "Open with > Choose another app".
        let app = self.application_key_path();
        entries.push(entry(&app, Some("FriendlyAppName"), "Image Viewer"));
        entries.push(entry(&format!(r"{app}\DefaultIcon"), None, &self.icon_value()));
        entries.push(entry(&format!(r"{app}\shell\open\command"), None, &self.expected_command()));
        for ext in &self.extensions {
            entries.push(entry(&format!(r"{app}\SupportedTypes"), Some(&ext.to_ascii_lowercase()), ""));
        }
        entries
    }

    pub fn command_matches_current_exe(&self, read: &str) -> bool {
        read.eq_ignore_ascii_case(&self.expected_command())
    }
}

fn file_name_of(exe_path: &str) -> &str {
    Path::new(exe_path).file_name().and_then(|n| n.to_str()).unwrap_or("")
}

fn application_key_path_for(exe_file_name: &str) -> String {
    format!(r"Software\Classes\Applications\{exe_file_name}")
}

pub fn prog_id_for(ext: &str) -> String {
    format!("ImageViewer{}", ext.to_ascii_lowercase())
}

pub fn extension_key_path(ext: &str) -> String {
    format!(r"Software\Classes\{}", ext.to_ascii_lowercase())
}

pub fn prog_id_key_path(ext: &str) -> String {
    format!(r"Software\Classes\{}", prog_id_for(ext))
}

pub fn open_with_progids_key_path(ext: &str) -> String {
    format!(r"{}\OpenWithProgids", extension_key_path(ext))
}

pub fn label_for(ext: &str) -> &'static str {
    match ext.to_ascii_lowercase().as_str() {
        ".jpg" | ".jpeg" => "JPEG image",
        ".bmp" => "Bitmap image",
        ".png" => "PNG image",
        ".gif" => "GIF image",
        ".tif" | ".tiff" => "TIFF image",
        ".tga" => "Targa image",
        ".ico" => "Icon",
        ".webp" => "WebP image",
        ".svg" => "SVG image",
        ".jxl" => "JPEG XL image",
        ".avif" => "AVIF image",
        ".heic" => "HEIC image",
        ".heif" => "HEIF image",
        _ => "Image file",
    }
}

/// `root` is "" in production; tests pass a sandbox prefix that is prepended to every key path (the shell is only notified when it is empty).
pub struct AssociationService {
    root: String,
    settings: Settings,
}

impl AssociationService {
    pub fn new(root: &str, settings: Settings) -> Self {
        Self { root: root.to_string(), settings }
    }

    fn full(&self, key_path: &str) -> String {
        if self.root.is_empty() { key_path.to_string() } else { format!(r"{}\{}", self.root, key_path) }
    }

    fn hkcu() -> RegKey {
        RegKey::predef(HKEY_CURRENT_USER)
    }

    fn delete_tree(&self, key_path: &str) -> io::Result<()> {
        match Self::hkcu().delete_subkey_all(self.full(key_path)) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            other => other,
        }
    }

    fn application_command(&self, app_key_path: &str) -> Option<String> {
        Self::hkcu().open_subkey(self.full(&format!(r"{app_key_path}\shell\open\command"))).ok()?.get_value::<String, _>("").ok()
    }

    /// Write the association keys unless they are already in place for this executable. Returns `false` when the stamp
    /// matched and nothing was written. On every run after the first this costs a couple of registry reads and no write.
    pub fn ensure_registered(&self, plan: &FileAssociationPlan) -> io::Result<bool> {
        // An empty exe name would resolve the application key to the shared Applications key itself.
        if file_name_of(&plan.exe_path).is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "executable path has no file name"));
        }

        // Belt and braces: the stamp survives a registry cleaner that wiped Software\Classes, so confirm the keys are still there.
        if self.settings.file_assoc_stamp() == plan.stamp() && self.application_command(&plan.application_key_path()).is_some_and(|c| plan.command_matches_current_exe(&c)) {
            return Ok(false);
        }

        for e in plan.entries_to_write() {
            let (key, _) = Self::hkcu().create_subkey(self.full(&e.key_path))?;
            key.set_value(e.value_name.as_deref().unwrap_or(""), &e.value)?;
        }

        self.settings.set_file_assoc_stamp(&plan.stamp())?;
        self.notify();
        Ok(true)
    }

    /// Remove everything `ensure_registered` wrote, so an uninstalled app stops appearing in the "Open with" menu.
    pub fn unregister(&self, exe_file_name: &str, extensions: &[&str]) -> io::Result<()> {
        // An empty name would resolve to the shared Applications key, and deleting that tree would wipe every per-user Open With registration.
        if exe_file_name.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "executable file name is empty"));
        }

        // Clear the stamp first so a failure partway through leaves the app looking unregistered and a later ensure_registered repairs it.
        self.settings.set_file_assoc_stamp("")?;

        for ext in extensions {
            let prog_id = prog_id_for(ext);

            // (a) the ProgID tree we own.
            self.delete_tree(&prog_id_key_path(ext))?;

            // (b) only our own entry in the shared OpenWithProgids list.
            if let Ok(open_with) = Self::hkcu().open_subkey_with_flags(self.full(&open_with_progids_key_path(ext)), KEY_SET_VALUE) {
                match open_with.delete_value(&prog_id) {
                    Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
                    _ => {}
                }
            }

            // (c) clear the extension default only if the user made one of our now-dead ProgIDs the default handler.
            if let Ok(ext_key) = Self::hkcu().open_subkey_with_flags(self.full(&extension_key_path(ext)), KEY_READ | KEY_SET_VALUE)
                && ext_key.get_value::<String, _>("").is_ok_and(|d| d.eq_ignore_ascii_case(&prog_id))
            {
                match ext_key.delete_value("") {
                    Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
                    _ => {}
                }
            }
        }

        // Software\Classes\Applications\<name> is shared by file name: only delete it when its open command is ours.
        // Only the file name is known here, so ownership is "quoted path ending in <name>" rather than the exact exe path.
        let app_key_path = application_key_path_for(exe_file_name);
        let suffix = format!("\\{exe_file_name}\" \"%1\"").to_ascii_lowercase();
        if self.application_command(&app_key_path).is_some_and(|c| {
            let c = c.to_ascii_lowercase();
            c.starts_with('"') && c.ends_with(&suffix)
        }) {
            self.delete_tree(&app_key_path)?;
        }

        self.notify();
        Ok(())
    }

    /// Tell Explorer to reload associations, no reboot and no explorer restart needed.
    fn notify(&self) {
        if self.root.is_empty() {
            use windows::Win32::UI::Shell::{SHCNE_ASSOCCHANGED, SHCNF_IDLIST, SHChangeNotify};
            // SAFETY: both item pointers are None, which SHCNE_ASSOCCHANGED requires.
            unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None) };
        }
    }
}
