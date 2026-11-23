use std::io::Cursor;
use std::path::Path;

use image::codecs::ico::IcoDecoder;
use image::{DynamicImage, ImageDecoder};

use super::{Frame, LoadError};

/// Real icons carry a handful of sizes; the directory count is a u16 an attacker controls.
const MAX_ENTRIES: usize = 256;

fn bad(msg: &str) -> LoadError {
    LoadError::Decode(msg.to_string())
}

/// Decodes every ICONDIR entry at its own size, largest first (stable by area).
pub(super) fn decode_ico(path: &Path) -> Result<Vec<Frame>, LoadError> {
    let data = std::fs::read(path)?;
    let count = match data.get(..6) {
        Some(h) if h[..4] == [0, 0, 1, 0] => u16::from_le_bytes([h[4], h[5]]) as usize,
        _ => return Err(bad("not an ICO file")),
    };
    if count == 0 {
        return Err(bad("ICO has no entries"));
    }
    if count > MAX_ENTRIES {
        return Err(bad("ICO has too many entries"));
    }
    let mut used = 0;
    let mut first_err = None;
    let mut frames = Vec::with_capacity(count);
    for i in 0..count {
        // A truncated directory ends the scan; entries read so far still count.
        let Some(entry) = data.get(6 + 16 * i..6 + 16 * (i + 1)) else {
            first_err.get_or_insert_with(|| bad("truncated ICO directory"));
            break;
        };
        match decode_entry(&data, entry, &mut used)? {
            Ok(frame) => frames.push(frame),
            Err(e) => {
                first_err.get_or_insert(e);
            }
        }
    }
    if frames.is_empty() {
        return Err(first_err.unwrap_or_else(|| bad("ICO has no readable entry")));
    }
    frames.sort_by_key(|f| std::cmp::Reverse(u64::from(f.pixels.width()) * u64::from(f.pixels.height())));
    Ok(frames)
}

/// Decodes one directory entry. The outer error is fatal (the decode budget ran out, the whole icon fails); the inner one only skips this entry.
fn decode_entry(data: &[u8], entry: &[u8], used: &mut u64) -> Result<Result<Frame, LoadError>, LoadError> {
    let size = u32::from_le_bytes(entry[8..12].try_into().unwrap()) as usize;
    let offset = u32::from_le_bytes(entry[12..16].try_into().unwrap()) as usize;
    let Some(payload) = offset.checked_add(size).and_then(|end| data.get(offset..end)) else {
        return Ok(Err(bad("ICO entry out of range")));
    };
    // Re-wrap the entry as a one-entry ICO so the image crate handles both PNG and DIB payloads.
    let mut single = vec![0, 0, 1, 0, 1, 0];
    single.extend_from_slice(&entry[..12]);
    single.extend_from_slice(&22u32.to_le_bytes());
    single.extend_from_slice(payload);
    let decoder = match IcoDecoder::new(Cursor::new(single)).and_then(|mut d| d.set_limits(super::decode_limits()).map(|()| d)) {
        Ok(d) => d,
        Err(e) => return Ok(Err(e.into())),
    };
    // The entry-vs-payload size check runs only after from_decoder allocated the payload's own (unbounded) size.
    super::reserve(used, decoder.total_bytes())?;
    Ok(DynamicImage::from_decoder(decoder).map(|img| Frame { pixels: img.into_rgba8(), delay_ms: 0 }).map_err(Into::into))
}
