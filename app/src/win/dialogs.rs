//! Native shell dialogs, run modally on the UI thread.

use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;

use viewer_core::files::{extension_of, normalize_extension};
use viewer_core::picture::can_save_jxl;
use windows::Win32::Foundation::{ERROR_CANCELLED, HWND};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree};
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::Shell::{FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM, FOS_OVERWRITEPROMPT, FileOpenDialog, FileSaveDialog, IFileOpenDialog, IFileSaveDialog, IShellItem, SIGDN_FILESYSPATH};
use windows::core::{HSTRING, PCWSTR};

/// `*.jpg;*.jpeg;...` from extensions with their leading dot.
fn pattern(extensions: &[&str]) -> String {
    extensions.iter().map(|e| format!("*{e}")).collect::<Vec<_>>().join(";")
}

/// `SaveService.PickSaveTargetAsync` filters: one per type in `types` order, labelled from `format` with "{0}" replaced by the
/// upper-case type without its dot; `.jxl` only when `jxl` (the encoder is installed).
fn save_filters<'a>(types: &[&'a str], format: &str, jxl: bool) -> Vec<(String, &'a str)> {
    types.iter().filter(|t| jxl || **t != ".jxl").map(|t| (format.replace("{0}", &t.trim_start_matches('.').to_uppercase()), *t)).collect()
}

/// Appends `ext` unless the file name already ends with an extension that normalizes to it ("a.JPEG" stays as is for ".jpg").
fn with_type_extension(path: PathBuf, ext: &str) -> PathBuf {
    if extension_of(&path).is_some_and(|e| normalize_extension(&e) == ext) {
        return path;
    }
    let mut name = path.into_os_string();
    name.push(ext);
    PathBuf::from(name)
}

/// Cancellation is silent; any other COM failure is logged.
fn completed<T>(result: windows::core::Result<T>, what: &str) -> Option<T> {
    result
        .inspect_err(|e| {
            if e.code() != ERROR_CANCELLED.to_hresult() {
                eprintln!("{what} dialog failed: {e}")
            }
        })
        .ok()
}

/// OLE (winit drag and drop) normally already made this thread an STA: then this only adds a reference that is never released.
fn init_com() {
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
}

/// Open file dialog with one filter listing every type of `extensions`; None when cancelled or on a COM failure.
pub fn pick_open(owner: HWND, extensions: &[&str]) -> Option<PathBuf> {
    init_com();
    completed(unsafe { show_open(owner, extensions) }, "open")
}

/// Save As dialog. Returns the target path and its save type, taken from the selected filter; the type's extension is appended when
/// the name lacks it. `format` is the localized FOOTER_TOOLBAR_MENU_FILE_SAVE_FORMAT. None when cancelled or on a COM failure.
pub fn pick_save(owner: HWND, suggested_name: &str, types: &[&str], format: &str) -> Option<(PathBuf, String)> {
    init_com();
    let filters = save_filters(types, format, can_save_jxl());
    let (path, index) = completed(unsafe { show_save(owner, suggested_name, &filters) }, "save")?;
    let ext = filters.get(index)?.1;
    Some((with_type_extension(path, ext), ext.to_string()))
}

unsafe fn file_system_path(item: &IShellItem) -> windows::core::Result<PathBuf> {
    unsafe {
        let raw = item.GetDisplayName(SIGDN_FILESYSPATH)?;
        let path = PathBuf::from(OsString::from_wide(raw.as_wide()));
        CoTaskMemFree(Some(raw.0 as *const _));
        Ok(path)
    }
}

unsafe fn show_open(owner: HWND, extensions: &[&str]) -> windows::core::Result<PathBuf> {
    unsafe {
        let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?;
        // No string table key names the filter in 1.x (FileOpenPicker shows none): the pattern is its own label.
        let spec = HSTRING::from(pattern(extensions));
        dialog.SetFileTypes(&[COMDLG_FILTERSPEC {
            pszName: PCWSTR(spec.as_ptr()),
            pszSpec: PCWSTR(spec.as_ptr()),
        }])?;
        dialog.SetOptions(dialog.GetOptions()? | FOS_FORCEFILESYSTEM | FOS_FILEMUSTEXIST)?;
        dialog.Show(Some(owner))?;
        file_system_path(&dialog.GetResult()?)
    }
}

/// Returns the chosen path and the zero-based index of the selected filter.
unsafe fn show_save(owner: HWND, suggested_name: &str, filters: &[(String, &str)]) -> windows::core::Result<(PathBuf, usize)> {
    unsafe {
        let dialog: IFileSaveDialog = CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER)?;
        let names: Vec<HSTRING> = filters.iter().map(|(label, _)| HSTRING::from(label.as_str())).collect();
        let specs: Vec<HSTRING> = filters.iter().map(|(_, ext)| HSTRING::from(pattern(&[ext]))).collect();
        let spec: Vec<COMDLG_FILTERSPEC> = names
            .iter()
            .zip(&specs)
            .map(|(n, s)| COMDLG_FILTERSPEC {
                pszName: PCWSTR(n.as_ptr()),
                pszSpec: PCWSTR(s.as_ptr()),
            })
            .collect();
        dialog.SetFileTypes(&spec)?;
        dialog.SetFileTypeIndex(1)?;
        // With a default extension the dialog appends the selected filter's extension to a bare name.
        if let Some((_, ext)) = filters.first() {
            dialog.SetDefaultExtension(&HSTRING::from(ext.trim_start_matches('.')))?;
        }
        dialog.SetFileName(&HSTRING::from(suggested_name))?;
        dialog.SetOptions(dialog.GetOptions()? | FOS_FORCEFILESYSTEM | FOS_OVERWRITEPROMPT)?;
        dialog.Show(Some(owner))?;
        let index = dialog.GetFileTypeIndex()? as usize;
        Ok((file_system_path(&dialog.GetResult()?)?, index.saturating_sub(1)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use viewer_core::files::SAVE_FILE_TYPES;

    #[test]
    fn pattern_lists_every_extension() {
        assert_eq!(pattern(&[".jpg", ".PNG"]), "*.jpg;*.PNG");
    }

    #[test]
    fn save_filters_keep_the_order_and_label_each_type() {
        let with = save_filters(&SAVE_FILE_TYPES, "{0} file", true);
        let labels: Vec<&str> = with.iter().map(|(l, _)| l.as_str()).collect();
        assert_eq!(labels, ["JPG file", "PNG file", "WEBP file", "BMP file", "GIF file", "TIFF file", "TGA file", "JXL file"]);
        assert!(with.iter().zip(SAVE_FILE_TYPES).all(|((_, t), e)| *t == e));
        let without = save_filters(&SAVE_FILE_TYPES, "{0}", false);
        assert_eq!(without.len(), 7);
        assert!(without.iter().all(|(_, t)| *t != ".jxl"));
    }

    #[test]
    fn type_extension_is_appended_only_when_missing() {
        let dir = PathBuf::from(r"C:\Été \u{5199}");
        assert_eq!(with_type_extension(dir.join("a"), ".jpg"), dir.join("a.jpg"));
        assert_eq!(with_type_extension(dir.join("a.JPEG"), ".jpg"), dir.join("a.JPEG"));
        assert_eq!(with_type_extension(dir.join("a.PNG"), ".png"), dir.join("a.PNG"));
        assert_eq!(with_type_extension(dir.join("a.tif"), ".tiff"), dir.join("a.tif"));
        assert_eq!(with_type_extension(dir.join("v1.2"), ".png"), dir.join("v1.2.png"));
        assert_eq!(with_type_extension(dir.join("a.png"), ".jpg"), dir.join("a.png.jpg"));
    }
}
