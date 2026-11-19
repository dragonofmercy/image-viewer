//! CF_DIBV5 / CF_DIB on the Win32 clipboard (`Utilities/ClipboardHelper.cs` in 1.x, plus the paste side).

use std::time::Duration;

use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND};
use windows::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard, SetClipboardData};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock};
use windows::Win32::System::Ole::{CF_DIB, CF_DIBV5};

/// Open clipboard; closed on drop, so every path after a successful open closes it.
struct Open;

impl Open {
    /// Another process may hold the clipboard for a moment: retries for about 100 ms, then gives up.
    fn new(owner: HWND) -> Option<Self> {
        for _ in 0..10 {
            if unsafe { OpenClipboard(Some(owner)) }.is_ok() {
                return Some(Self);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        None
    }
}

impl Drop for Open {
    fn drop(&mut self) {
        // Only built after a successful OpenClipboard by this thread.
        let _ = unsafe { CloseClipboard() };
    }
}

/// Replaces the clipboard content with `dib_v5` as CF_DIBV5 (first: the most descriptive) and `dib` as CF_DIB. False when the
/// clipboard could not be opened or written.
pub fn set_dibs(owner: HWND, dib_v5: &[u8], dib: &[u8]) -> bool {
    let Some(_open) = Open::new(owner) else { return false };
    // The clipboard is open on this thread.
    if unsafe { EmptyClipboard() }.is_err() {
        return false;
    }
    put(CF_DIBV5.0, dib_v5) && put(CF_DIB.0, dib)
}

/// Adds one format to the open clipboard.
fn put(format: u16, dib: &[u8]) -> bool {
    // A plain allocation: no pointer is handed out yet.
    let Ok(mem) = (unsafe { GlobalAlloc(GMEM_MOVEABLE, dib.len()) }) else { return false };
    // `mem` is a live movable block we own.
    let target = unsafe { GlobalLock(mem) };
    if target.is_null() {
        free(mem);
        return false;
    }
    // `target` points to at least `dib.len()` writable bytes until GlobalUnlock, and cannot overlap `dib`.
    unsafe { std::ptr::copy_nonoverlapping(dib.as_ptr(), target.cast::<u8>(), dib.len()) };
    // Balances the lock above; the result reports the lock count, not a failure.
    let _ = unsafe { GlobalUnlock(mem) };
    // On success the system owns `mem`; on failure it is still ours.
    if unsafe { SetClipboardData(u32::from(format), Some(HANDLE(mem.0))) }.is_err() {
        free(mem);
        return false;
    }
    true
}

fn free(mem: HGLOBAL) {
    // `mem` is ours and unlocked; nothing refers to it afterwards.
    let _ = unsafe { GlobalFree(Some(mem)) };
}

/// The bitmap of the clipboard (copied out), or None when there is none or the clipboard is busy. CF_DIBV5 comes first: the CF_DIB
/// that Windows synthesizes from it drops the alpha mask. A CF_DIBV5 synthesized from CF_DIB or CF_BITMAP has an all-zero alpha,
/// which `parse_dib` reads as opaque.
pub fn get_dib(owner: HWND) -> Option<Vec<u8>> {
    // Checked first so a clipboard without a bitmap is not opened at all.
    let format = [CF_DIBV5, CF_DIB].into_iter().map(|f| u32::from(f.0)).find(|&f| unsafe { IsClipboardFormatAvailable(f) }.is_ok())?;
    let _open = Open::new(owner)?;
    // The clipboard is open on this thread; the handle stays the clipboard's and is never freed here.
    let mem = HGLOBAL(unsafe { GetClipboardData(format) }.ok()?.0);
    // `mem` is a global block owned by the clipboard, valid while it is open.
    let size = unsafe { GlobalSize(mem) };
    // Same block; a null pointer means it could not be locked.
    let source = unsafe { GlobalLock(mem) };
    if source.is_null() {
        return None;
    }
    // `source` points to `size` readable bytes until GlobalUnlock.
    let bytes = unsafe { std::slice::from_raw_parts(source.cast::<u8>(), size) }.to_vec();
    // Balances the lock above; the result reports the lock count, not a failure.
    let _ = unsafe { GlobalUnlock(mem) };
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Ignored by default: it overwrites the user's clipboard. Run with `cargo test -p imageviewer -- --ignored`.
    #[test]
    #[ignore]
    fn dib_round_trips_through_the_clipboard() {
        let bgra: Vec<u8> = (0..16).collect();
        let (v5, dib) = (viewer_core::dib::build_dib_v5(&bgra, 2, 2), viewer_core::dib::build_dib(&bgra, 2, 2));
        assert!(set_dibs(HWND::default(), &v5, &dib));
        assert_eq!(get_dib(HWND::default()).as_deref(), Some(v5.as_slice()));
    }
}
