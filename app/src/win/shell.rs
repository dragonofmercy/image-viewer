//! Shell launches (`Process.Start` in 1.x).

use std::path::Path;

use windows::Win32::UI::Shell::ShellExecuteW;
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
#[allow(dead_code)] // first caller: the About dialog task
pub fn open_url(url: &str) {
    open(&HSTRING::from(url));
}
