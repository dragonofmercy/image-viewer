//! Native shell dialogs, run modally on the UI thread.

use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::{Path, PathBuf};

use viewer_core::files::{extension_of, normalize_extension};
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
/// upper-case type without its dot.
fn save_filters<'a>(types: &[&'a str], format: &str) -> Vec<(String, &'a str)> {
    types.iter().map(|t| (format.replace("{0}", &t.trim_start_matches('.').to_uppercase()), *t)).collect()
}

/// The saved file from the dialog result. As `PickSaveTargetAsync` reads `StorageFile.FileType`, a typed extension that normalizes to
/// one of the offered `types` decides the type and the path stays as typed. Otherwise the selected filter's type decides and its
/// extension is appended; the bool is then true: the dialog never confirmed that final name.
fn resolve_save_target(picked: &Path, filter_type: &str, types: &[&str]) -> (PathBuf, String, bool) {
    if let Some(ext) = extension_of(picked).map(|e| normalize_extension(&e)).filter(|e| types.contains(&e.as_str())) {
        return (picked.to_path_buf(), ext, false);
    }
    let mut name = picked.as_os_str().to_os_string();
    name.push(filter_type);
    (PathBuf::from(name), filter_type.to_string(), true)
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

/// Save As dialog. Returns the target path, its save type (see `resolve_save_target`) and whether the name was changed after the
/// dialog's overwrite prompt. `format` is the localized FOOTER_TOOLBAR_MENU_FILE_SAVE_FORMAT. None when cancelled or on a COM failure.
pub fn pick_save(owner: HWND, suggested_name: &str, types: &[&str], format: &str) -> Option<(PathBuf, String, bool)> {
    init_com();
    let filters = save_filters(types, format);
    let (path, index) = completed(unsafe { show_save(owner, suggested_name, &filters) }, "save")?;
    let offered: Vec<&str> = filters.iter().map(|(_, t)| *t).collect();
    Some(resolve_save_target(&path, filters.get(index)?.1, &offered))
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
        // No strict file types: they could rewrite a typed `a.png` under the JPG filter. The caller refuses to replace an existing file
        // whose name changed after the dialog.
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
        let filters = save_filters(&SAVE_FILE_TYPES, "{0} file");
        let labels: Vec<&str> = filters.iter().map(|(l, _)| l.as_str()).collect();
        assert_eq!(labels, ["JPG file", "PNG file", "WEBP file", "BMP file", "GIF file", "TIFF file", "TGA file", "JXL file"]);
        assert!(filters.iter().zip(SAVE_FILE_TYPES).all(|((_, t), e)| *t == e));
    }

    #[test]
    fn save_target_follows_the_typed_extension_then_the_filter() {
        let dir = PathBuf::from(r"C:\Été \u{5199}");
        let offered = [".jpg", ".png", ".webp", ".bmp", ".gif", ".tiff", ".tga"];
        let resolve = |name: &str, filter: &str| resolve_save_target(&dir.join(name), filter, &offered);
        assert_eq!(resolve("a.png", ".jpg"), (dir.join("a.png"), ".png".into(), false));
        assert_eq!(resolve("a", ".jpg"), (dir.join("a.jpg"), ".jpg".into(), true));
        assert_eq!(resolve("a.JPEG", ".png"), (dir.join("a.JPEG"), ".jpg".into(), false));
        assert_eq!(resolve("scan.TIF", ".png"), (dir.join("scan.TIF"), ".tiff".into(), false));
        assert_eq!(resolve("a.txt", ".webp"), (dir.join("a.txt.webp"), ".webp".into(), true));
        assert_eq!(resolve("photo.2024", ".png"), (dir.join("photo.2024.png"), ".png".into(), true));
    }
}
