//! CF_DIB clipboard payloads: build (Ctrl+C) and parse (Ctrl+V, untrusted bytes from other apps).

use image::RgbaImage;

/// Builds a 32 bpp BI_RGB BITMAPINFOHEADER DIB, rows flipped to bottom-up. `top_down_bgra` must hold `width * height * 4` bytes.
pub fn build_dib(top_down_bgra: &[u8], width: u32, height: u32) -> Vec<u8> {
    build(top_down_bgra, width, height, false)
}

/// Same pixels as `build_dib` behind a 124-byte BITMAPV5HEADER (BI_BITFIELDS with an explicit alpha mask, sRGB), for CF_DIBV5:
/// readers take the alpha channel from it, where a CF_DIB alpha byte is ambiguous.
pub fn build_dib_v5(top_down_bgra: &[u8], width: u32, height: u32) -> Vec<u8> {
    build(top_down_bgra, width, height, true)
}

fn build(top_down_bgra: &[u8], width: u32, height: u32, v5: bool) -> Vec<u8> {
    let header_size: u32 = if v5 { 124 } else { 40 };
    let stride = width as usize * 4;
    let image_size = stride * height as usize;
    let mut dib = Vec::with_capacity(header_size as usize + image_size);
    dib.extend_from_slice(&header_size.to_le_bytes()); // biSize
    dib.extend_from_slice(&width.to_le_bytes()); // biWidth
    dib.extend_from_slice(&height.to_le_bytes()); // biHeight (positive => bottom-up)
    dib.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
    dib.extend_from_slice(&32u16.to_le_bytes()); // biBitCount
    dib.extend_from_slice(&(if v5 { BI_BITFIELDS } else { BI_RGB }).to_le_bytes()); // biCompression
    dib.extend_from_slice(&(image_size as u32).to_le_bytes()); // biSizeImage
    dib.extend_from_slice(&[0; 16]); // biXPelsPerMeter, biYPelsPerMeter, biClrUsed, biClrImportant
    if v5 {
        for mask in [0x00FF_0000u32, 0x0000_FF00, 0x0000_00FF, 0xFF00_0000] {
            dib.extend_from_slice(&mask.to_le_bytes()); // bV5RedMask, bV5GreenMask, bV5BlueMask, bV5AlphaMask
        }
        dib.extend_from_slice(&LCS_SRGB.to_le_bytes()); // bV5CSType
        dib.extend_from_slice(&[0; 48]); // bV5Endpoints, bV5GammaRed/Green/Blue
        dib.extend_from_slice(&LCS_GM_IMAGES.to_le_bytes()); // bV5Intent
        dib.extend_from_slice(&[0; 12]); // bV5ProfileData, bV5ProfileSize, bV5Reserved
    }
    if stride > 0 {
        for row in top_down_bgra[..image_size].chunks_exact(stride).rev() {
            dib.extend_from_slice(row);
        }
    }
    dib
}

const BI_RGB: u32 = 0;
const BI_BITFIELDS: u32 = 3;
/// 'sRGB'
const LCS_SRGB: u32 = 0x7352_4742;
const LCS_GM_IMAGES: u32 = 4;
/// Upper bound on the pixel buffer we accept (the buffer must also really contain the data, so this only rejects absurd claims early).
const MAX_PIXEL_BYTES: u64 = 1 << 32;

fn u32_at(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?))
}

/// Returns (shift, max value) of a channel mask, or None when the mask is empty.
fn channel(mask: u32) -> Option<(u32, u64)> {
    if mask == 0 {
        return None;
    }
    let shift = mask.trailing_zeros();
    Some((shift, u64::from(mask >> shift)))
}

fn extract(pixel: u32, ch: Option<(u32, u64)>) -> u8 {
    match ch {
        Some((shift, max)) => (u64::from(pixel >> shift) & max).saturating_mul(255).checked_div(max).unwrap_or(0) as u8,
        None => 0,
    }
}

/// Parses a CF_DIB / CF_DIBV5 payload (24/32 bpp, BI_RGB or BI_BITFIELDS, bottom-up or top-down).
pub fn parse_dib(bytes: &[u8]) -> Result<RgbaImage, String> {
    let header_size = u32_at(bytes, 0).ok_or("DIB too short")?;
    if !matches!(header_size, 40 | 108 | 124) || bytes.len() < header_size as usize {
        return Err(format!("unsupported DIB header size {header_size}"));
    }
    let width = u32_at(bytes, 4).unwrap() as i32;
    let raw_height = u32_at(bytes, 8).unwrap() as i32;
    let bpp = u16::from_le_bytes([bytes[14], bytes[15]]);
    let compression = u32_at(bytes, 16).unwrap();
    let colors_used = u64::from(u32_at(bytes, 32).unwrap());
    if width <= 0 || raw_height == 0 {
        return Err("invalid DIB dimensions".into());
    }
    let (w, h, top_down) = (width as u64, u64::from(raw_height.unsigned_abs()), raw_height < 0);
    if !matches!((bpp, compression), (24, BI_RGB) | (32, BI_RGB | BI_BITFIELDS)) {
        return Err(format!("unsupported DIB format: {bpp} bpp, compression {compression}"));
    }

    // Channel masks (R, G, B, A): after the 40-byte header, inside V4/V5 headers, or the BI_RGB defaults.
    let mut masks = [0x00FF_0000, 0x0000_FF00, 0x0000_00FF, 0xFF00_0000];
    let mut pixel_offset = u64::from(header_size) + colors_used * 4;
    if compression == BI_BITFIELDS {
        if header_size == 40 {
            for (i, m) in masks.iter_mut().take(3).enumerate() {
                *m = u32_at(bytes, 40 + i * 4).ok_or("DIB too short for bit masks")?;
            }
            masks[3] = 0;
            pixel_offset += 12;
        } else {
            for (i, m) in masks.iter_mut().enumerate() {
                *m = u32_at(bytes, 40 + i * 4).unwrap();
            }
        }
    }

    let bytes_per_pixel = u64::from(bpp / 8);
    let stride = (w * bytes_per_pixel).div_ceil(4) * 4;
    let data_size = stride.checked_mul(h).filter(|n| *n <= MAX_PIXEL_BYTES).ok_or("DIB dimensions too large")?;
    let data_end = pixel_offset.checked_add(data_size).ok_or("DIB dimensions too large")?;
    if data_end > bytes.len() as u64 {
        return Err("DIB pixel data truncated".into());
    }
    // From here the buffer provably holds every pixel, so w * h * 4 <= data_size * 4 fits in memory we already have.
    let (w, h, stride, start) = (w as usize, h as usize, stride as usize, pixel_offset as usize);
    let data = &bytes[start..start + data_size as usize];
    let [r, g, b, a] = masks.map(channel);

    let mut out = vec![0u8; w * h * 4];
    for y in 0..h {
        let src_y = if top_down { y } else { h - 1 - y };
        let row = &data[src_y * stride..(src_y + 1) * stride];
        for x in 0..w {
            let px = &mut out[(y * w + x) * 4..][..4];
            if bpp == 24 {
                px.copy_from_slice(&[row[x * 3 + 2], row[x * 3 + 1], row[x * 3], 255]);
            } else {
                let v = u32::from_le_bytes(row[x * 4..x * 4 + 4].try_into().unwrap());
                px.copy_from_slice(&[extract(v, r), extract(v, g), extract(v, b), extract(v, a)]);
            }
        }
    }
    // An all-zero alpha channel (typical of 32 bpp BI_RGB written by apps that ignore alpha) means opaque.
    if bpp == 32 && out.as_chunks::<4>().0.iter().all(|p| p[3] == 0) {
        out.as_chunks_mut::<4>().0.iter_mut().for_each(|p| p[3] = 255);
    }
    RgbaImage::from_raw(w as u32, h as u32, out).ok_or_else(|| "DIB dimensions too large".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn le32(b: &[u8], o: usize) -> i32 {
        i32::from_le_bytes(b[o..o + 4].try_into().unwrap())
    }

    fn le16(b: &[u8], o: usize) -> u16 {
        u16::from_le_bytes(b[o..o + 2].try_into().unwrap())
    }

    /// Hand-made DIB: `header_size` 40/108/124, `masks` = R,G,B,A written where the format puts them.
    fn make_dib(header_size: u32, width: i32, height: i32, bpp: u16, compression: u32, masks: Option<[u32; 4]>, pixels: &[u8]) -> Vec<u8> {
        let mut h = vec![0u8; header_size as usize];
        h[0..4].copy_from_slice(&header_size.to_le_bytes());
        h[4..8].copy_from_slice(&width.to_le_bytes());
        h[8..12].copy_from_slice(&height.to_le_bytes());
        h[12..14].copy_from_slice(&1u16.to_le_bytes());
        h[14..16].copy_from_slice(&bpp.to_le_bytes());
        h[16..20].copy_from_slice(&compression.to_le_bytes());
        let mut extra = Vec::new();
        if let Some(m) = masks {
            if header_size == 40 {
                for v in &m[..3] {
                    extra.extend_from_slice(&v.to_le_bytes());
                }
            } else {
                for (i, v) in m.iter().enumerate() {
                    h[40 + i * 4..44 + i * 4].copy_from_slice(&v.to_le_bytes());
                }
            }
        }
        [h, extra, pixels.to_vec()].concat()
    }

    #[test]
    fn build_dib_writes_bitmap_info_header() {
        let dib = build_dib(&[0; 16], 2, 2);
        assert_eq!(le32(&dib, 0), 40);
        assert_eq!(le32(&dib, 4), 2);
        assert_eq!(le32(&dib, 8), 2);
        assert_eq!(le16(&dib, 12), 1);
        assert_eq!(le16(&dib, 14), 32);
        assert_eq!(le32(&dib, 16), 0);
        assert_eq!(le32(&dib, 20), 16);
        assert_eq!(dib.len(), 40 + 16);
    }

    #[test]
    fn build_dib_flips_rows_bottom_up() {
        let dib = build_dib(&[10, 11, 12, 13, 20, 21, 22, 23], 1, 2);
        assert_eq!(&dib[40..44], &[20, 21, 22, 23]);
        assert_eq!(&dib[44..48], &[10, 11, 12, 13]);
    }

    #[test]
    fn parse_dib_round_trips_build_dib() {
        // 2x2 top-down BGRA with distinct non-zero alpha
        let bgra = [1, 2, 3, 200, 4, 5, 6, 201, 7, 8, 9, 202, 10, 11, 12, 203];
        let img = parse_dib(&build_dib(&bgra, 2, 2)).unwrap();
        assert_eq!((img.width(), img.height()), (2, 2));
        assert_eq!(img.get_pixel(0, 0).0, [3, 2, 1, 200]);
        assert_eq!(img.get_pixel(1, 0).0, [6, 5, 4, 201]);
        assert_eq!(img.get_pixel(0, 1).0, [9, 8, 7, 202]);
        assert_eq!(img.get_pixel(1, 1).0, [12, 11, 10, 203]);
    }

    #[test]
    fn parse_dib_24bpp_bottom_up() {
        // 1x2, rows padded to 4 bytes; stored bottom row first. BGR.
        let dib = make_dib(40, 1, 2, 24, 0, None, &[1, 2, 3, 0xEE, 4, 5, 6, 0xEE]);
        let img = parse_dib(&dib).unwrap();
        assert_eq!(img.get_pixel(0, 0).0, [6, 5, 4, 255]);
        assert_eq!(img.get_pixel(0, 1).0, [3, 2, 1, 255]);
    }

    #[test]
    fn parse_dib_24bpp_top_down() {
        let dib = make_dib(40, 1, -2, 24, 0, None, &[1, 2, 3, 0, 4, 5, 6, 0]);
        let img = parse_dib(&dib).unwrap();
        assert_eq!(img.get_pixel(0, 0).0, [3, 2, 1, 255]);
        assert_eq!(img.get_pixel(0, 1).0, [6, 5, 4, 255]);
    }

    #[test]
    fn parse_dib_zero_alpha_is_opaque() {
        let dib = make_dib(40, 2, 1, 32, 0, None, &[1, 2, 3, 0, 4, 5, 6, 0]);
        let img = parse_dib(&dib).unwrap();
        assert_eq!(img.get_pixel(0, 0).0, [3, 2, 1, 255]);
        assert_eq!(img.get_pixel(1, 0).0, [6, 5, 4, 255]);
    }

    #[test]
    fn parse_dib_bitfields_after_40_byte_header() {
        // RGBA byte order via masks; no alpha mask means opaque.
        let dib = make_dib(40, 1, 1, 32, 3, Some([0xFF, 0xFF00, 0xFF0000, 0]), &[10, 20, 30, 0]);
        assert_eq!(parse_dib(&dib).unwrap().get_pixel(0, 0).0, [10, 20, 30, 255]);
    }

    #[test]
    fn parse_dib_v5_bitfields_with_alpha_mask() {
        // Pixel bytes [R,G,B,A] as little-endian u32 with masks R=0xFF, G=0xFF00, B=0xFF0000, A=0xFF000000.
        let dib = make_dib(124, 1, 1, 32, 3, Some([0xFF, 0xFF00, 0xFF0000, 0xFF00_0000]), &[10, 20, 30, 40]);
        assert_eq!(parse_dib(&dib).unwrap().get_pixel(0, 0).0, [10, 20, 30, 40]);
    }

    #[test]
    fn build_dib_v5_round_trips_with_alpha() {
        let bgra = [1, 2, 3, 0, 4, 5, 6, 128, 7, 8, 9, 255, 10, 11, 12, 1];
        let dib = build_dib_v5(&bgra, 2, 2);
        assert_eq!(dib.len(), 124 + 16);
        assert_eq!((le32(&dib, 0), le32(&dib, 16), le32(&dib, 52)), (124, 3, 0xFF00_0000u32 as i32));
        assert_eq!(&dib[56..60], b"BGRs");
        let img = parse_dib(&dib).unwrap();
        assert_eq!(img.get_pixel(0, 0).0, [3, 2, 1, 0]);
        assert_eq!(img.get_pixel(1, 0).0, [6, 5, 4, 128]);
        assert_eq!(img.get_pixel(1, 1).0, [12, 11, 10, 1]);
    }

    #[test]
    fn parse_dib_v5_without_alpha_is_opaque() {
        // What Windows synthesizes from CF_DIB / CF_BITMAP: no alpha mask, or an alpha mask over zero bytes.
        let no_mask = make_dib(124, 1, 1, 32, 3, Some([0xFF0000, 0xFF00, 0xFF, 0]), &[1, 2, 3, 99]);
        assert_eq!(parse_dib(&no_mask).unwrap().get_pixel(0, 0).0, [3, 2, 1, 255]);
        let zero_alpha = make_dib(124, 1, 1, 32, 3, Some([0xFF0000, 0xFF00, 0xFF, 0xFF00_0000]), &[1, 2, 3, 0]);
        assert_eq!(parse_dib(&zero_alpha).unwrap().get_pixel(0, 0).0, [3, 2, 1, 255]);
        let bi_rgb = make_dib(124, 1, 1, 32, 0, None, &[1, 2, 3, 0]);
        assert_eq!(parse_dib(&bi_rgb).unwrap().get_pixel(0, 0).0, [3, 2, 1, 255]);
    }

    #[test]
    fn parse_dib_v4_bi_rgb_default_bgra() {
        let dib = make_dib(108, 1, 1, 32, 0, None, &[1, 2, 3, 77]);
        assert_eq!(parse_dib(&dib).unwrap().get_pixel(0, 0).0, [3, 2, 1, 77]);
    }

    #[test]
    fn parse_dib_skips_color_table() {
        let mut dib = make_dib(40, 1, 1, 24, 0, None, &[]);
        dib[32..36].copy_from_slice(&2u32.to_le_bytes()); // biClrUsed = 2
        dib.extend_from_slice(&[9; 8]);
        dib.extend_from_slice(&[1, 2, 3, 0]);
        assert_eq!(parse_dib(&dib).unwrap().get_pixel(0, 0).0, [3, 2, 1, 255]);
    }

    #[test]
    fn parse_dib_rejects_bad_input_without_panicking() {
        assert!(parse_dib(&[]).is_err());
        assert!(parse_dib(&[40, 0, 0, 0]).is_err());
        let px = [0u8; 8];
        assert!(parse_dib(&make_dib(40, 2, 1, 32, 0, None, &px[..7])).is_err(), "truncated");
        assert!(parse_dib(&make_dib(40, 0, 1, 32, 0, None, &px)).is_err(), "zero width");
        assert!(parse_dib(&make_dib(40, 1, 0, 32, 0, None, &px)).is_err(), "zero height");
        assert!(parse_dib(&make_dib(40, -1, 1, 32, 0, None, &px)).is_err(), "negative width");
        assert!(parse_dib(&make_dib(40, 1, 1, 8, 0, None, &px)).is_err(), "8 bpp");
        assert!(parse_dib(&make_dib(40, 1, 1, 32, 1, None, &px)).is_err(), "RLE");
        assert!(parse_dib(&make_dib(40, 1, 1, 24, 3, None, &px)).is_err(), "24 bpp bitfields");
        assert!(parse_dib(&make_dib(41, 1, 1, 32, 0, None, &px)).is_err(), "odd header size");
        assert!(parse_dib(&make_dib(40, i32::MAX, i32::MAX, 32, 0, None, &px)).is_err(), "huge");
        assert!(parse_dib(&make_dib(40, i32::MAX, i32::MIN, 24, 0, None, &px)).is_err(), "huge top-down");
        let mut v = make_dib(40, 1, 1, 24, 0, None, &px);
        v[32..36].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(parse_dib(&v).is_err(), "huge color table");
    }
}
