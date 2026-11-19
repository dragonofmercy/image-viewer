//! Velopack install integration: startup hooks, the update protocol, the toast identity and the 1.x leftovers.
use std::io;
use std::path::{Path, PathBuf};

use viewer_core::association::{AssociationService, FileAssociationPlan};
use viewer_core::files::SUPPORTED_FILE_TYPES;
use viewer_core::settings::Settings;
use winreg::RegKey;
use winreg::enums::*;

use crate::launch::PROTOCOL;

pub const AUMID: &str = "velopack.Dragon.ImageViewer";

/// `root` is "" in production; tests pass a sandbox prefix prepended to every key path.
pub struct Integration {
    root: String,
}

fn ignore_missing(r: io::Result<()>) -> io::Result<()> {
    match r {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

/// `{` + 36 hex digits and dashes + `}`, so the value can never walk out of the CLSID key.
fn is_guid(s: &str) -> bool {
    s.len() == 38 && s.starts_with('{') && s.ends_with('}') && s[1..37].chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

impl Integration {
    pub fn new(root: &str) -> Self {
        Self { root: root.to_string() }
    }

    fn classes(&self, sub: &str) -> String {
        if self.root.is_empty() { format!(r"Software\Classes\{sub}") } else { format!(r"{}\Software\Classes\{sub}", self.root) }
    }

    fn hkcu() -> RegKey {
        RegKey::predef(HKEY_CURRENT_USER)
    }

    fn aumid_key(&self) -> String {
        self.classes(&format!(r"AppUserModelId\{AUMID}"))
    }

    pub fn register(&self, exe: &Path) -> io::Result<()> {
        let exe = exe.to_string_lossy().into_owned();
        let (key, _) = Self::hkcu().create_subkey(self.classes(PROTOCOL))?;
        key.set_value("", &"URL:Image Viewer")?;
        key.set_value("URL Protocol", &"")?;
        let (cmd, _) = Self::hkcu().create_subkey(self.classes(&format!(r"{PROTOCOL}\shell\open\command")))?;
        cmd.set_value("", &format!("\"{exe}\" \"%1\""))?;
        let (id, _) = Self::hkcu().create_subkey(self.aumid_key())?;
        id.set_value("DisplayName", &"Image Viewer")?;
        id.set_value("IconUri", &exe)?;
        Ok(())
    }

    pub fn unregister(&self) -> io::Result<()> {
        let protocol = ignore_missing(Self::hkcu().delete_subkey_all(self.classes(PROTOCOL)));
        let identity = ignore_missing(Self::hkcu().delete_subkey_all(self.aumid_key()));
        protocol.and(identity)
    }

    /// 1.x registered a Windows App SDK toast activator under the same AUMID: drop its CLSID and the pointer to it.
    pub fn remove_legacy_activator(&self) -> io::Result<()> {
        let Ok(id) = Self::hkcu().open_subkey_with_flags(self.aumid_key(), KEY_READ | KEY_SET_VALUE) else { return Ok(()) };
        let Ok(guid) = id.get_value::<String, _>("CustomActivator") else { return Ok(()) };
        if !is_guid(&guid) {
            return Ok(());
        }
        let tree = ignore_missing(Self::hkcu().delete_subkey_all(self.classes(&format!(r"CLSID\{guid}"))));
        let value = ignore_missing(id.delete_value("CustomActivator"));
        tree.and(value)
    }
}

fn log(step: &str, r: io::Result<()>) {
    if let Err(e) = r {
        eprintln!("{step} failed: {e}");
    }
}

/// Port of `Helpers\LegacyCleanup.cs`: 1.x data folder, the registry settings key is kept.
fn legacy_cleanup_in(dir: &Path) {
    log("legacy cleanup", ignore_missing(std::fs::remove_dir_all(dir)));
}

fn legacy_cleanup() {
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        legacy_cleanup_in(&PathBuf::from(local).join("Dragon Industries"));
    }
}

fn integrate() {
    match std::env::current_exe() {
        Ok(exe) => log("register integration", Integration::new("").register(&exe)),
        Err(e) => eprintln!("cannot locate the executable: {e}"),
    }
    log("remove legacy activator", Integration::new("").remove_legacy_activator());
}

fn uninstall() {
    if let Some(exe) = std::env::current_exe().ok().and_then(|p| p.to_str().map(str::to_string)) {
        let plan = FileAssociationPlan::new(&exe, &SUPPORTED_FILE_TYPES);
        log("unregister file associations", AssociationService::new("", Settings::app()).unregister(&plan));
    }
    log("unregister integration", Integration::new("").unregister());
}

/// First statement of `main`. Velopack does nothing when the exe is not an installed copy.
pub fn run_hooks() {
    velopack::VelopackApp::build()
        .on_first_run(|_| legacy_cleanup())
        .on_after_install_fast_callback(|_| integrate())
        .on_after_update_fast_callback(|_| integrate())
        .on_before_uninstall_fast_callback(|_| uninstall())
        .run();
}

#[cfg(test)]
mod tests {
    use super::*;

    const GUID: &str = "{11111111-2222-3333-4444-555555555555}";

    fn sandbox(name: &str) -> String {
        let root = format!(r"SOFTWARE\Dragon Industries\Image Viewer Tests\install-{name}");
        let _ = Integration::hkcu().delete_subkey_all(&root);
        root
    }

    fn read(root: &str, sub: &str, name: &str) -> Option<String> {
        Integration::hkcu().open_subkey(format!(r"{root}\Software\Classes\{sub}")).ok()?.get_value::<String, _>(name).ok()
    }

    fn exists(root: &str, sub: &str) -> bool {
        Integration::hkcu().open_subkey(format!(r"{root}\Software\Classes\{sub}")).is_ok()
    }

    #[test]
    fn register_writes_protocol_and_identity() {
        let root = sandbox("register");
        Integration::new(&root).register(Path::new(r"C:\Apps\ImageViewer.exe")).unwrap();
        assert_eq!(read(&root, PROTOCOL, "").as_deref(), Some("URL:Image Viewer"));
        assert_eq!(read(&root, PROTOCOL, "URL Protocol").as_deref(), Some(""));
        assert_eq!(read(&root, r"dragon-imageviewer\shell\open\command", "").as_deref(), Some(r#""C:\Apps\ImageViewer.exe" "%1""#));
        let id = format!(r"AppUserModelId\{AUMID}");
        assert_eq!(read(&root, &id, "DisplayName").as_deref(), Some("Image Viewer"));
        assert_eq!(read(&root, &id, "IconUri").as_deref(), Some(r"C:\Apps\ImageViewer.exe"));
        let _ = Integration::hkcu().delete_subkey_all(&root);
    }

    #[test]
    fn legacy_activator_is_removed() {
        let root = sandbox("legacy");
        let it = Integration::new(&root);
        it.remove_legacy_activator().unwrap(); // missing keys are not an error
        it.register(Path::new(r"C:\Apps\ImageViewer.exe")).unwrap();
        let hk = Integration::hkcu();
        let (id, _) = hk.create_subkey(it.aumid_key()).unwrap();
        id.set_value("CustomActivator", &GUID).unwrap();
        hk.create_subkey(it.classes(&format!(r"CLSID\{GUID}\LocalServer32"))).unwrap();
        it.remove_legacy_activator().unwrap();
        assert!(!exists(&root, &format!(r"CLSID\{GUID}")));
        assert_eq!(read(&root, &format!(r"AppUserModelId\{AUMID}"), "CustomActivator"), None);
        assert_eq!(read(&root, &format!(r"AppUserModelId\{AUMID}"), "DisplayName").as_deref(), Some("Image Viewer"));
        let _ = hk.delete_subkey_all(&root);
    }

    #[test]
    fn malformed_activator_deletes_nothing() {
        let root = sandbox("malformed");
        let it = Integration::new(&root);
        let hk = Integration::hkcu();
        let (id, _) = hk.create_subkey(it.aumid_key()).unwrap();
        id.set_value("CustomActivator", &r"..\..\Software").unwrap();
        hk.create_subkey(it.classes(&format!(r"CLSID\{GUID}"))).unwrap();
        it.remove_legacy_activator().unwrap();
        assert_eq!(read(&root, &format!(r"AppUserModelId\{AUMID}"), "CustomActivator").as_deref(), Some(r"..\..\Software"));
        assert!(exists(&root, &format!(r"CLSID\{GUID}")));
        assert!(!is_guid(&format!("{{{}}}", "g".repeat(36))));
        let _ = hk.delete_subkey_all(&root);
    }

    #[test]
    fn unregister_removes_everything_it_wrote() {
        let root = sandbox("unregister");
        let it = Integration::new(&root);
        it.register(Path::new(r"C:\Apps\ImageViewer.exe")).unwrap();
        it.unregister().unwrap();
        assert!(!exists(&root, PROTOCOL));
        assert!(!exists(&root, &format!(r"AppUserModelId\{AUMID}")));
        it.unregister().unwrap();
        let _ = Integration::hkcu().delete_subkey_all(&root);
    }

    #[test]
    fn legacy_cleanup_removes_only_the_given_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("Dragon Industries");
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        legacy_cleanup_in(&dir);
        assert!(!dir.exists());
        legacy_cleanup_in(&dir); // already gone
    }
}
