use std::io::Cursor;
use std::path::Path;

use image::DynamicImage;
use image::codecs::ico::IcoDecoder;

use super::{Frame, LoadError};

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
    let mut frames = Vec::with_capacity(count);
    for i in 0..count {
        let entry = data.get(6 + 16 * i..6 + 16 * (i + 1)).ok_or_else(|| bad("truncated ICO directory"))?;
        let size = u32::from_le_bytes(entry[8..12].try_into().unwrap()) as usize;
        let offset = u32::from_le_bytes(entry[12..16].try_into().unwrap()) as usize;
        let payload = offset.checked_add(size).and_then(|end| data.get(offset..end)).ok_or_else(|| bad("ICO entry out of range"))?;
        // Re-wrap the entry as a one-entry ICO so the image crate handles both PNG and DIB payloads.
        let mut single = vec![0, 0, 1, 0, 1, 0];
        single.extend_from_slice(&entry[..12]);
        single.extend_from_slice(&22u32.to_le_bytes());
        single.extend_from_slice(payload);
        let img = DynamicImage::from_decoder(IcoDecoder::new(Cursor::new(single))?)?;
        frames.push(Frame { pixels: img.into_rgba8(), delay_ms: 0 });
    }
    frames.sort_by_key(|f| std::cmp::Reverse(u64::from(f.pixels.width()) * u64::from(f.pixels.height())));
    Ok(frames)
}
