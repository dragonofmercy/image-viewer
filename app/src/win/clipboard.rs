//! CF_DIBV5 / CF_DIB on the Win32 clipboard (`Utilities/ClipboardHelper.cs` in 1.x, plus the paste side).

use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::time::Duration;

use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND};
use windows::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, EnumClipboardFormats, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard, SetClipboardData};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock};
use windows::Win32::System::Ole::{CF_BITMAP, CF_DIB, CF_DIBV5, CF_HDROP};

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

/// Replaces the clipboard content with `dib` as CF_DIB (the 1.x format, always written) and `dib_v5` as CF_DIBV5 for readers that
/// take the alpha channel from it. False when the clipboard could not be opened or CF_DIB could not be written; a failed CF_DIBV5
/// write is only logged. `file` adds the file itself as CF_HDROP, for targets that paste files; a failure there is only logged too.
pub fn set_dibs(owner: HWND, dib_v5: &[u8], dib: &[u8], file: Option<&Path>) -> bool {
    let Some(_open) = Open::new(owner) else { return false };
    // The clipboard is open on this thread.
    if unsafe { EmptyClipboard() }.is_err() {
        return false;
    }
    if !put(CF_DIB.0, dib) {
        return false;
    }
    if !put(CF_DIBV5.0, dib_v5) {
        eprintln!("cannot put CF_DIBV5 on the clipboard, CF_DIB only");
    }
    if let Some(file) = file
        && !put(CF_HDROP.0, &build_hdrop(file))
    {
        eprintln!("cannot put CF_HDROP on the clipboard, image only");
    }
    true
}

/// A `DROPFILES` block for one file (wide characters), built by hand: header of 20 bytes, then the path and the list terminator.
pub fn build_hdrop(path: &Path) -> Vec<u8> {
    let mut block = vec![0u8; 20];
    block[0] = 20; // pFiles: offset of the file list; pt and fNC stay zero
    block[16] = 1; // fWide
    for unit in path.as_os_str().encode_wide().chain([0, 0]) {
        block.extend_from_slice(&unit.to_le_bytes());
    }
    block
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

/// Which bitmap format to read, from the formats in `EnumClipboardFormats` order. The formats an application placed are listed
/// before the ones Windows synthesizes, so CF_DIBV5 listed before CF_DIB and CF_BITMAP was placed by the application itself and
/// carries its alpha mask. Otherwise CF_DIB: what a synthesized CF_DIBV5 holds is not documented (its alpha mask may be zero, which
/// would make a 32 bpp CF_DIB with real alpha opaque).
fn pick_dib_format(formats_in_order: &[u32]) -> Option<u32> {
    let (v5, dib, bitmap) = (u32::from(CF_DIBV5.0), u32::from(CF_DIB.0), u32::from(CF_BITMAP.0));
    formats_in_order.iter().find(|&&f| f == v5 || f == dib || f == bitmap).map(|&f| if f == v5 { v5 } else { dib })
}

/// The formats on the open clipboard, in enumeration order.
fn clipboard_formats() -> Vec<u32> {
    let mut formats = Vec::new();
    let mut format = 0;
    loop {
        // The clipboard is open on this thread; 0 ends the list (or reports an error, which ends it too).
        format = unsafe { EnumClipboardFormats(format) };
        if format == 0 {
            return formats;
        }
        formats.push(format);
    }
}

/// The bitmap of the clipboard (copied out, see `pick_dib_format`), or None when there is none or the clipboard is busy.
pub fn get_dib(owner: HWND) -> Option<Vec<u8>> {
    // CF_DIB is available whenever any bitmap format is (synthesized): a clipboard without a bitmap is not opened at all.
    unsafe { IsClipboardFormatAvailable(u32::from(CF_DIB.0)) }.ok()?;
    let _open = Open::new(owner)?;
    let format = pick_dib_format(&clipboard_formats())?;
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
        assert!(set_dibs(HWND::default(), &v5, &dib, None));
        // CF_DIB is placed first, so it is the one read back.
        assert_eq!(get_dib(HWND::default()).as_deref(), Some(dib.as_slice()));
    }

    #[test]
    fn hdrop_block_holds_one_wide_path() {
        let path = Path::new(r"C:\a.gif");
        let block = build_hdrop(path);
        assert_eq!(block.len(), 20 + (8 + 2) * 2);
        assert_eq!(block[..4], [20, 0, 0, 0]);
        assert_eq!(block[16..20], [1, 0, 0, 0]);
        assert_eq!(block[block.len() - 4..], [0, 0, 0, 0]);
        let units: Vec<u16> = block[20..block.len() - 4].chunks(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        assert_eq!(String::from_utf16(&units).unwrap(), r"C:\a.gif");
    }

    #[test]
    fn dibv5_is_read_only_when_placed_before_dib_and_bitmap() {
        assert_eq!(pick_dib_format(&[17, 8]), Some(17));
        assert_eq!(pick_dib_format(&[8, 17]), Some(8));
        assert_eq!(pick_dib_format(&[2, 8, 17]), Some(8));
        assert_eq!(pick_dib_format(&[17]), Some(17));
        assert_eq!(pick_dib_format(&[8]), Some(8));
        assert_eq!(pick_dib_format(&[1, 13]), None);
        assert_eq!(pick_dib_format(&[]), None);
        // Other formats placed first do not matter.
        assert_eq!(pick_dib_format(&[49161, 13, 17, 8, 2]), Some(17));
    }
}
