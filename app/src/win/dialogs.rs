//! Native shell dialogs, run modally on the UI thread.

use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;

use windows::Win32::Foundation::{ERROR_CANCELLED, HWND};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree};
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::Shell::{FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM, FileOpenDialog, IFileOpenDialog, SIGDN_FILESYSPATH};
use windows::core::{HSTRING, PCWSTR};

/// `*.jpg;*.jpeg;...` from extensions with their leading dot.
fn pattern(extensions: &[&str]) -> String {
    extensions.iter().map(|e| format!("*{e}")).collect::<Vec<_>>().join(";")
}

/// Open file dialog with one filter listing every type of `extensions`; None when cancelled or on a COM failure.
pub fn pick_open(owner: HWND, extensions: &[&str]) -> Option<PathBuf> {
    // OLE (winit drag and drop) normally already made this thread an STA: then this only adds a reference that is never released.
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    match unsafe { show_open(owner, extensions) } {
        Ok(path) => Some(path),
        Err(e) => {
            if e.code() != ERROR_CANCELLED.to_hresult() {
                eprintln!("open dialog failed: {e}");
            }
            None
        }
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
        let raw = dialog.GetResult()?.GetDisplayName(SIGDN_FILESYSPATH)?;
        let path = PathBuf::from(OsString::from_wide(raw.as_wide()));
        CoTaskMemFree(Some(raw.0 as *const _));
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn pattern_lists_every_extension() {
        assert_eq!(super::pattern(&[".jpg", ".PNG"]), "*.jpg;*.PNG");
    }
}
