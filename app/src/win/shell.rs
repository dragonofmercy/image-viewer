//! Shell launches (`Process.Start` in 1.x) and the recycle bin.

use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Shell::{FO_DELETE, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_SILENT, FOF_WANTNUKEWARNING, SHFILEOPSTRUCTW, SHFileOperationW, ShellExecuteW};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{HSTRING, PCWSTR, w};

fn open(target: &HSTRING) {
    // ShellExecuteW returns a value above 32 on success.
    let result = unsafe { ShellExecuteW(None, w!("open"), PCWSTR(target.as_ptr()), PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL) };
    if result.0 as isize <= 32 {
        eprintln!("cannot open {target}");
    }
}

/// Opens a folder in Explorer.
pub fn open_in_explorer(folder: &Path) {
    open(&HSTRING::from(folder.as_os_str()));
}

/// Opens a URL in the default browser.
pub fn open_url(url: &str) {
    open(&HSTRING::from(url));
}

/// Sends a file to the recycle bin without confirmation (`FileSystem.DeleteFile` with `OnlyErrorDialogs`, `SendToRecycleBin`); the shell
/// still shows its error dialogs, owned by `owner`. When the file cannot be recycled (no recycle bin on the drive, too big) Windows asks
/// before deleting it for good; answering No aborts. `path` must be absolute. False when the file was not recycled or deleted.
pub fn recycle(owner: HWND, path: &Path) -> bool {
    // pFrom is a list of NUL-terminated names ended by an empty one.
    let from: Vec<u16> = path.as_os_str().encode_wide().chain([0, 0]).collect();
    let mut op = SHFILEOPSTRUCTW {
        hwnd: owner,
        wFunc: FO_DELETE,
        pFrom: PCWSTR(from.as_ptr()),
        fFlags: (FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT | FOF_WANTNUKEWARNING).0 as u16,
        ..Default::default()
    };
    let result = unsafe { SHFileOperationW(&mut op) };
    result == 0 && !op.fAnyOperationsAborted.as_bool()
}

#[cfg(test)]
mod tests {
    // Ignored by default: it leaves the file in the real recycle bin. Run with `cargo test -p imageviewer -- --ignored`.
    #[test]
    #[ignore]
    fn recycles_a_file_of_its_own_temp_directory() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("imageviewer-recycle-test \u{5199}.PNG");
        std::fs::write(&file, b"x").expect("write");
        assert!(super::recycle(super::HWND::default(), &file));
        assert!(!file.exists());
    }
}
