//! EXIF / ICC carried from load to save. The bytes come from untrusted files: every parse here is bounds-checked and gives up (dropping the metadata, never the image) on anything unexpected.

use image::ImageDecoder;

/// Reads the EXIF and ICC blobs of a decoder that has not decoded its pixels yet. A metadata read error loses the metadata only.
pub(super) fn read(decoder: &mut impl ImageDecoder) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
    let exif = decoder.exif_metadata().ok().flatten().and_then(reset_orientation);
    let icc = decoder.icc_profile().ok().flatten().filter(|icc| !icc.is_empty());
    (exif, icc)
}

/// Sets every IFD0 Orientation entry (tag 0x0112, one SHORT) to 1, since the loader rotates the pixels itself. `None` when the TIFF structure is not parseable.
fn reset_orientation(mut exif: Vec<u8>) -> Option<Vec<u8>> {
    let big = match exif.get(..4)? {
        b"II*\0" => false,
        b"MM\0*" => true,
        _ => return None,
    };
    let u16_at = |b: &[u8], i: usize| -> Option<u16> {
        let v = b.get(i..i.checked_add(2)?)?.try_into().ok()?;
        Some(if big { u16::from_be_bytes(v) } else { u16::from_le_bytes(v) })
    };
    let u32_at = |b: &[u8], i: usize| -> Option<u32> {
        let v = b.get(i..i.checked_add(4)?)?.try_into().ok()?;
        Some(if big { u32::from_be_bytes(v) } else { u32::from_le_bytes(v) })
    };
    let ifd = usize::try_from(u32_at(&exif, 4)?).ok()?;
    let count = usize::from(u16_at(&exif, ifd)?);
    if ifd.checked_add(2 + 12 * count)? > exif.len() {
        return None;
    }
    let one = if big { 1u16.to_be_bytes() } else { 1u16.to_le_bytes() };
    for e in (0..count).map(|i| ifd + 2 + 12 * i) {
        if u16_at(&exif, e)? == 0x0112 && u16_at(&exif, e + 2)? == 3 && u32_at(&exif, e + 4)? == 1 {
            exif[e + 8..e + 10].copy_from_slice(&one);
        }
    }
    Some(exif)
}

const VP8X_ICC: u8 = 0x20;
const VP8X_ALPHA: u8 = 0x10;
const VP8X_EXIF: u8 = 0x08;

/// Adds ICCP / EXIF chunks to a still WebP file as libwebp's simple encoder writes it (`VP8 `, `VP8L`, or `VP8X` + `ALPH` + `VP8 `), wrapping a simple file into an extended one.
/// `width` x `height` is the canvas size. An unexpected layout, or a result too big for RIFF, returns the file unchanged.
pub(super) fn add_webp_chunks(webp: Vec<u8>, width: u32, height: u32, icc: Option<&[u8]>, exif: Option<&[u8]>) -> Vec<u8> {
    if icc.is_none() && exif.is_none() {
        return webp;
    }
    let Some(chunks) = webp_chunks(&webp) else { return webp };
    let Some((first, first_payload)) = chunks.first().map(|&(fourcc, chunk)| (fourcc, &chunk[8..])) else { return webp };
    let mut vp8x = match first {
        b"VP8X" if first_payload.len() >= 10 => first_payload[..10].to_vec(),
        b"VP8 " | b"VP8L" if (1..=1 << 24).contains(&width) && (1..=1 << 24).contains(&height) => {
            // VP8L header: signature byte, then 32 LE bits whose bit 28 is alpha_is_used.
            let alpha = first == b"VP8L" && first_payload.get(4).is_some_and(|b| b & 0x10 != 0);
            let mut p = vec![if alpha { VP8X_ALPHA } else { 0 }, 0, 0, 0];
            p.extend_from_slice(&(width - 1).to_le_bytes()[..3]);
            p.extend_from_slice(&(height - 1).to_le_bytes()[..3]);
            p
        }
        _ => return webp,
    };
    vp8x[0] |= if icc.is_some() { VP8X_ICC } else { 0 } | if exif.is_some() { VP8X_EXIF } else { 0 };
    let mut body = b"WEBP".to_vec();
    push_chunk(&mut body, b"VP8X", &vp8x);
    if let Some(icc) = icc {
        push_chunk(&mut body, b"ICCP", icc);
    }
    let image_chunks = if first == b"VP8X" { &chunks[1..] } else { &chunks[..] };
    for (_, chunk) in image_chunks {
        body.extend_from_slice(chunk);
        // Restores a pad byte missing at the end of the input, so the EXIF chunk after it stays aligned.
        if chunk.len() % 2 == 1 {
            body.push(0);
        }
    }
    if let Some(exif) = exif {
        push_chunk(&mut body, b"EXIF", exif);
    }
    let Ok(riff_size) = u32::try_from(body.len()) else { return webp };
    let mut out = b"RIFF".to_vec();
    out.extend_from_slice(&riff_size.to_le_bytes());
    out.extend_from_slice(&body);
    out
}

/// The webp crate ends the animation at timestamp 0, which libwebp rejects, so the last frame gets the average duration of the others.
/// Sets the last ANMF duration so the frames add up to `total_ms` (the earlier durations are exact, merged identical frames included).
pub(super) fn set_webp_animation_length(webp: &mut [u8], total_ms: u32) {
    let Some(chunks) = webp_chunks(webp) else { return };
    // Chunk start offsets; an ANMF payload holds its 24-bit duration at bytes 12..15, i.e. 20..23 from the chunk start.
    let anmf: Vec<usize> = chunks.iter().filter(|c| c.0 == b"ANMF" && c.1.len() >= 24).map(|c| c.1.as_ptr() as usize - webp.as_ptr() as usize).collect();
    let Some((&last, earlier)) = anmf.split_last() else { return };
    let before: u32 = earlier.iter().map(|&p| u32::from_le_bytes([webp[p + 20], webp[p + 21], webp[p + 22], 0])).sum();
    let ms = total_ms.saturating_sub(before).min(0xFF_FFFF);
    webp[last + 20..last + 23].copy_from_slice(&ms.to_le_bytes()[..3]);
}

/// Splits a RIFF/WEBP file into (fourcc, whole chunk with header and padding). `None` on a bad header or a chunk running past the end.
pub(super) fn webp_chunks(webp: &[u8]) -> Option<Vec<(&[u8], &[u8])>> {
    if webp.len() < 12 || &webp[..4] != b"RIFF" || &webp[8..12] != b"WEBP" {
        return None;
    }
    let mut chunks = Vec::new();
    let mut pos = 12;
    while pos < webp.len() {
        let size = usize::try_from(u32::from_le_bytes(webp.get(pos + 4..pos + 8)?.try_into().ok()?)).ok()?;
        let end = pos.checked_add(8)?.checked_add(size)?;
        if end > webp.len() {
            return None;
        }
        // Tolerates a missing pad byte after the last chunk.
        let end = (end + (size & 1)).min(webp.len());
        chunks.push((&webp[pos..pos + 4], &webp[pos..end]));
        pos = end;
    }
    Some(chunks)
}

/// Appends a chunk, padded to an even size. The caller rejects a total over u32, which also covers a payload whose size would truncate here.
fn push_chunk(out: &mut Vec<u8>, fourcc: &[u8; 4], payload: &[u8]) {
    out.extend_from_slice(fourcc);
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(payload);
    if payload.len() % 2 == 1 {
        out.push(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn orientation_exif(big: bool, orientation: u16) -> Vec<u8> {
        let mut e = if big { b"MM\0*".to_vec() } else { b"II*\0".to_vec() };
        // (value, byte width): IFD0 offset, one entry (Orientation, SHORT, count 1, value, pad), no next IFD.
        for (n, len) in [(8, 4), (1, 2), (0x0112, 2), (3, 2), (1, 4), (u32::from(orientation), 2), (0, 2), (0, 4)] {
            let be = &n.to_be_bytes()[4 - len..];
            e.extend(if big { be.to_vec() } else { be.iter().rev().copied().collect() });
        }
        e
    }

    #[test]
    fn reset_orientation_honors_byte_order() {
        for big in [false, true] {
            let out = reset_orientation(orientation_exif(big, 6)).unwrap();
            assert_eq!(image::metadata::Orientation::from_exif_chunk(&out), Some(image::metadata::Orientation::NoTransforms));
            assert_eq!(out.len(), orientation_exif(big, 6).len());
        }
    }

    #[test]
    fn reset_orientation_rejects_malformed_input() {
        assert_eq!(reset_orientation(Vec::new()), None);
        assert_eq!(reset_orientation(b"Exif\0\0II*\0".to_vec()), None);
        let mut far_ifd = orientation_exif(false, 6);
        far_ifd[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(reset_orientation(far_ifd), None);
        let mut truncated = orientation_exif(false, 6);
        truncated.truncate(20);
        assert_eq!(reset_orientation(truncated), None);
        let mut huge_count = orientation_exif(false, 6);
        huge_count[8..10].copy_from_slice(&u16::MAX.to_le_bytes());
        assert_eq!(reset_orientation(huge_count), None);
    }

    #[test]
    fn reset_orientation_keeps_exif_without_orientation_tag() {
        let mut e = orientation_exif(false, 6);
        e[10..12].copy_from_slice(&0x010Fu16.to_le_bytes());
        assert_eq!(reset_orientation(e.clone()), Some(e));
    }

    #[test]
    fn webp_chunks_survive_garbage_unchanged() {
        let garbage = vec![b"not a webp".to_vec(), b"RIFF\0\0\0\0WEBPVP8 \xff\xff\xff\xff".to_vec(), b"RIFF\0\0\0\0WEBPVP8".to_vec(), b"RIFF\0\0\0\0WEBPABCD\x02\0\0\0xy".to_vec()];
        for g in garbage {
            assert_eq!(add_webp_chunks(g.clone(), 4, 4, Some(b"icc"), Some(b"exif")), g);
        }
    }

    #[test]
    fn simple_webp_is_wrapped_in_vp8x_with_padded_chunks() {
        let img = image::RgbaImage::from_pixel(5, 3, image::Rgba([10, 20, 30, 255]));
        let plain = webp::Encoder::from_rgba(img.as_raw(), 5, 3).encode_simple(false, 90.0).unwrap().to_vec();
        let out = add_webp_chunks(plain, 5, 3, Some(b"icc"), Some(b"exif!"));
        let fourccs: Vec<&[u8]> = webp_chunks(&out).unwrap().iter().map(|c| c.0).collect();
        assert_eq!(fourccs, [&b"VP8X"[..], b"ICCP", b"VP8 ", b"EXIF"]);
        assert_eq!(u32::from_le_bytes(out[4..8].try_into().unwrap()) as usize, out.len() - 8);
        assert_eq!(out[20], VP8X_ICC | VP8X_EXIF);
        assert_eq!(&out[24..30], &[4, 0, 0, 2, 0, 0]);
        let mut dec = image::codecs::webp::WebPDecoder::new(std::io::Cursor::new(out)).unwrap();
        assert_eq!(dec.icc_profile().unwrap().as_deref(), Some(&b"icc"[..]));
        assert_eq!(dec.exif_metadata().unwrap().as_deref(), Some(&b"exif!"[..]));
        assert_eq!(dec.dimensions(), (5, 3));
    }
}
