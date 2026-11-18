use viewer_core::association::*;
use viewer_core::files::SUPPORTED_FILE_TYPES;
use viewer_core::settings::Settings;
use winreg::RegKey;
use winreg::enums::*;

// A path with a space, so the quoting of the shell command is actually exercised.
const EXE: &str = r"C:\Program Files\Image Viewer\ImageViewer.exe";

fn build_plan() -> FileAssociationPlan {
    FileAssociationPlan::new(EXE, &SUPPORTED_FILE_TYPES)
}

#[test]
fn prog_id_for_prefixes_the_lower_cased_extension() {
    for (extension, expected) in [(".jpg", "ImageViewer.jpg"), (".webp", "ImageViewer.webp"), (".PNG", "ImageViewer.png")] {
        assert_eq!(prog_id_for(extension), expected);
    }
}

#[test]
fn expected_command_quotes_both_the_exe_and_the_argument() {
    assert_eq!(build_plan().expected_command(), format!("\"{EXE}\" \"%1\""));
}

#[test]
fn entries_to_write_registers_an_open_command_for_every_supported_extension() {
    let plan = build_plan();
    let entries = plan.entries_to_write();
    for extension in SUPPORTED_FILE_TYPES {
        let expected_key = format!(r"{}\shell\open\command", prog_id_key_path(extension));
        assert!(entries.iter().any(|e| e.key_path == expected_key && e.value_name.is_none() && e.value == plan.expected_command()));
    }
}

#[test]
fn entries_to_write_labels_each_prog_id_and_points_its_icon_at_the_exe() {
    let entries = build_plan().entries_to_write();
    assert!(entries.iter().any(|e| e.key_path == prog_id_key_path(".jpg") && e.value_name.is_none() && e.value == "JPEG image"));
    assert!(entries.iter().any(|e| e.key_path == prog_id_key_path(".svg") && e.value_name.is_none() && e.value == "SVG image"));
    for extension in SUPPORTED_FILE_TYPES {
        let icon_key = format!(r"{}\DefaultIcon", prog_id_key_path(extension));
        assert!(entries.iter().any(|e| e.key_path == icon_key && e.value_name.is_none() && e.value == format!("{EXE},0")));
    }
}

#[test]
fn entries_to_write_adds_the_prog_id_to_open_with_progids_for_every_supported_extension() {
    let entries = build_plan().entries_to_write();
    for extension in SUPPORTED_FILE_TYPES {
        let expected_key = open_with_progids_key_path(extension);
        assert!(entries.iter().any(|e| e.key_path == expected_key && e.value_name.as_deref() == Some(prog_id_for(extension).as_str()) && e.value.is_empty()));
    }
}

// The core guard of the whole design: we only ever propose ourselves. Writing the
// (Default) value of an extension key would hijack another application's association.
#[test]
fn entries_to_write_never_writes_the_default_value_of_an_extension_key() {
    let entries = build_plan().entries_to_write();
    for extension in SUPPORTED_FILE_TYPES {
        let extension_key = extension_key_path(extension);
        assert!(!entries.iter().any(|e| e.key_path == extension_key && e.value_name.as_deref().is_none_or(str::is_empty)));
    }
}

#[test]
fn label_for_has_a_real_label_for_every_supported_extension() {
    for extension in SUPPORTED_FILE_TYPES {
        assert_ne!(label_for(extension), "Image file");
    }
}

#[test]
fn application_key_path_uses_the_executable_file_name() {
    assert_eq!(build_plan().application_key_path(), r"Software\Classes\Applications\ImageViewer.exe");
}

#[test]
fn entries_to_write_declares_the_application_block_and_every_supported_type() {
    let plan = build_plan();
    let entries = plan.entries_to_write();
    let app = plan.application_key_path();

    assert!(entries.iter().any(|e| e.key_path == app && e.value_name.as_deref() == Some("FriendlyAppName") && e.value == "Image Viewer"));
    assert!(entries.iter().any(|e| e.key_path == format!(r"{app}\shell\open\command") && e.value_name.is_none() && e.value == plan.expected_command()));
    for extension in SUPPORTED_FILE_TYPES {
        assert!(entries.iter().any(|e| e.key_path == format!(r"{app}\SupportedTypes") && e.value_name.as_deref() == Some(extension) && e.value.is_empty()));
    }
}

#[test]
fn command_matches_current_exe_ignores_case_and_rejects_another_exe() {
    let plan = build_plan();
    assert!(plan.command_matches_current_exe(&plan.expected_command().to_uppercase()));
    assert!(!plan.command_matches_current_exe(r#""C:\Other\ImageViewer.exe" "%1""#));
    assert!(!plan.command_matches_current_exe(""));
}

#[test]
fn stamp_changes_with_the_executable_path() {
    let installed = build_plan();
    let moved = FileAssociationPlan::new(r"D:\Portable\ImageViewer.exe", &SUPPORTED_FILE_TYPES);
    assert_ne!(installed.stamp(), moved.stamp());
    assert!(installed.stamp().contains(EXE));
}

#[test]
fn stamp_changes_with_the_extension_set() {
    let fewer = FileAssociationPlan::new(EXE, &[".jpg", ".png"]);
    let more = FileAssociationPlan::new(EXE, &[".jpg", ".png", ".webp"]);
    assert_ne!(fewer.stamp(), more.stamp());
}

#[test]
fn stamp_and_icon_use_the_csharp_layout() {
    let plan = FileAssociationPlan::new(EXE, &[".jpg", ".png"]);
    assert_eq!(plan.stamp(), format!("{LAYOUT_VERSION}|{EXE}|.jpg;.png"));
    assert_eq!(plan.icon_value(), format!("{EXE},0"));
}

/// Sandbox root under HKCU, deleted on drop (even on panic). Every key path is prefixed with it, so the real Software\Classes is never touched.
struct Sandbox {
    root: String,
}

impl Sandbox {
    fn new() -> Self {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        Self {
            root: format!(r"Software\Dragon Industries\Image Viewer Tests\assoc-{}-{}", std::process::id(), nanos),
        }
    }

    fn service(&self) -> AssociationService {
        AssociationService::new(&self.root, Settings::at(&format!(r"{}\Settings", self.root)))
    }

    fn settings(&self) -> Settings {
        Settings::at(&format!(r"{}\Settings", self.root))
    }

    fn key(&self, path: &str) -> Option<RegKey> {
        RegKey::predef(HKEY_CURRENT_USER).open_subkey(format!(r"{}\{}", self.root, path)).ok()
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all(&self.root);
    }
}

#[test]
fn ensure_registered_writes_every_entry_then_is_a_no_op() {
    let sb = Sandbox::new();
    let service = sb.service();
    let plan = build_plan();

    assert!(service.ensure_registered(&plan).unwrap());
    for entry in plan.entries_to_write() {
        let key = sb.key(&entry.key_path).unwrap_or_else(|| panic!("missing key {}", entry.key_path));
        let got: String = key.get_value(entry.value_name.as_deref().unwrap_or("")).unwrap();
        assert_eq!(got, entry.value, "{} / {:?}", entry.key_path, entry.value_name);
    }
    assert_eq!(sb.settings().file_assoc_stamp(), plan.stamp());

    assert!(!service.ensure_registered(&plan).unwrap());
}

#[test]
fn ensure_registered_repairs_when_the_keys_vanish_but_the_stamp_survives() {
    let sb = Sandbox::new();
    let service = sb.service();
    let plan = build_plan();
    service.ensure_registered(&plan).unwrap();

    RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all(format!(r"{}\{}", sb.root, plan.application_key_path())).unwrap();

    assert!(service.ensure_registered(&plan).unwrap());
    assert!(sb.key(&format!(r"{}\shell\open\command", plan.application_key_path())).is_some());
}

#[test]
fn unregister_removes_everything_it_wrote() {
    let sb = Sandbox::new();
    let service = sb.service();
    let plan = build_plan();
    service.ensure_registered(&plan).unwrap();

    // A foreign handler sharing an OpenWithProgids list must survive.
    let (open_with, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(format!(r"{}\{}", sb.root, open_with_progids_key_path(".png"))).unwrap();
    open_with.set_value("Other.png", &"").unwrap();

    service.unregister(&plan).unwrap();

    for extension in SUPPORTED_FILE_TYPES {
        assert!(sb.key(&prog_id_key_path(extension)).is_none(), "{extension} prog id left behind");
        if let Some(key) = sb.key(&open_with_progids_key_path(extension)) {
            assert!(key.get_raw_value(prog_id_for(extension)).is_err(), "{extension} OpenWithProgids entry left behind");
        }
    }
    assert!(sb.key(&plan.application_key_path()).is_none());
    assert!(sb.key(&open_with_progids_key_path(".png")).unwrap().get_raw_value("Other.png").is_ok());
    assert_eq!(sb.settings().file_assoc_stamp(), "");
}

#[test]
fn unregister_keeps_an_application_key_owned_by_another_exe() {
    let sb = Sandbox::new();
    let plan = build_plan();
    let (cmd, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(format!(r"{}\{}\shell\open\command", sb.root, plan.application_key_path())).unwrap();
    cmd.set_value("", &r#""C:\Other\ImageViewer.exe" "%1""#).unwrap();

    sb.service().unregister(&plan).unwrap();

    assert!(sb.key(&plan.application_key_path()).is_some());
}

/// Registers, sets the (Default) of the .png extension key to `default`, unregisters, and returns what is left of it.
fn png_default_after_unregister(default: &str) -> Option<String> {
    let sb = Sandbox::new();
    let plan = build_plan();
    sb.service().ensure_registered(&plan).unwrap();
    let (ext, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(format!(r"{}\{}", sb.root, extension_key_path(".png"))).unwrap();
    ext.set_value("", &default).unwrap();
    sb.service().unregister(&plan).unwrap();
    sb.key(&extension_key_path(".png")).and_then(|k| k.get_value("").ok())
}

#[test]
fn unregister_clears_an_extension_default_that_names_our_prog_id() {
    assert_eq!(png_default_after_unregister("IMAGEVIEWER.PNG"), None);
}

#[test]
fn unregister_keeps_an_extension_default_owned_by_another_app() {
    assert_eq!(png_default_after_unregister("Other.png").as_deref(), Some("Other.png"));
}

#[test]
fn never_writes_extension_default_value() {
    let sb = Sandbox::new();
    sb.service().ensure_registered(&build_plan()).unwrap();

    for extension in SUPPORTED_FILE_TYPES {
        let key = sb.key(&extension_key_path(extension)).expect("extension key exists for OpenWithProgids");
        assert!(key.get_raw_value("").is_err(), "{extension} (Default) was written");
    }
}
