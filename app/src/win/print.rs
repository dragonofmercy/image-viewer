//! Printing through the classic system print dialog and GDI: one image, fitted on the page inside a half inch margin.

use image::{Rgba, RgbaImage, imageops};
use windows::Win32::Foundation::{GlobalFree, HWND};
use windows::Win32::Graphics::Gdi::{BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, DeleteDC, GetDeviceCaps, HALFTONE, HORZRES, LOGPIXELSX, SRCCOPY, SetStretchBltMode, StretchDIBits, VERTRES};
use windows::Win32::Storage::Xps::{AbortDoc, DOCINFOW, EndDoc, EndPage, StartDocW, StartPage};
use windows::Win32::UI::Controls::Dialogs::{PD_NOCURRENTPAGE, PD_NOPAGENUMS, PD_NOSELECTION, PD_RESULT_PRINT, PD_RETURNDC, PD_USEDEVMODECOPIESANDCOLLATE, PRINTDLGEXW, PrintDlgExW, START_PAGE_GENERAL};
use windows::core::{HSTRING, PCWSTR};

/// (x, y, w, h) in device pixels of the image enlarged or reduced to the page minus the margin, centered, aspect kept.
pub fn fit_page(image: (u32, u32), page: (i32, i32), margin: i32) -> (i32, i32, i32, i32) {
    let (bw, bh) = ((page.0 - 2 * margin).max(1) as f64, (page.1 - 2 * margin).max(1) as f64);
    let scale = (bw / image.0 as f64).min(bh / image.1 as f64);
    let (w, h) = (((image.0 as f64 * scale).round() as i32).max(1), ((image.1 as f64 * scale).round() as i32).max(1));
    (margin + (bw as i32 - w) / 2, margin + (bh as i32 - h) / 2, w, h)
}

/// Top-down BGRA composited over white (transparent pixels print as paper), reduced to `size` when that is smaller than the image.
pub fn flatten_on_white(mut img: RgbaImage, size: (u32, u32)) -> Vec<u8> {
    // Composite first: thumbnail averages straight RGBA, so transparent black would darken the edges.
    for Rgba([r, g, b, a]) in img.pixels_mut() {
        let a = u32::from(*a);
        let over = |c: u8| ((u32::from(c) * a + 255 * (255 - a) + 127) / 255) as u8;
        (*r, *g, *b) = (over(*r), over(*g), over(*b));
    }
    if size.0 < img.width() || size.1 < img.height() {
        img = imageops::thumbnail(&img, size.0.clamp(1, img.width()), size.1.clamp(1, img.height()));
    }
    let mut bgra = img.into_raw();
    for px in bgra.as_chunks_mut::<4>().0 {
        px.swap(0, 2);
        px[3] = 255;
    }
    bgra
}

/// Shows the print dialog and prints `pixels` on the chosen printer. Ok(false) when the user cancelled.
pub fn print(hwnd: HWND, pixels: RgbaImage, job_name: &str) -> Result<bool, String> {
    let mut pd = PRINTDLGEXW {
        lStructSize: size_of::<PRINTDLGEXW>() as u32,
        hwndOwner: hwnd,
        Flags: PD_RETURNDC | PD_NOPAGENUMS | PD_NOSELECTION | PD_NOCURRENTPAGE | PD_USEDEVMODECOPIESANDCOLLATE,
        nStartPage: START_PAGE_GENERAL,
        ..Default::default()
    };
    // SAFETY: `pd` is a fully initialised PRINTDLGEXW; the DC and the global handles it returns are released below on every path.
    unsafe {
        let shown = PrintDlgExW(&mut pd);
        let result = match shown {
            Err(e) => Err(format!("print dialog failed: {e}")),
            Ok(()) if pd.dwResultAction == PD_RESULT_PRINT => draw(pd.hDC, pixels, job_name).map(|()| true),
            Ok(()) => Ok(false),
        };
        if !pd.hDC.is_invalid() {
            let _ = DeleteDC(pd.hDC);
        }
        for h in [pd.hDevMode, pd.hDevNames] {
            if !h.is_invalid() {
                let _ = GlobalFree(Some(h));
            }
        }
        result
    }
}

/// One page on the printer DC. SAFETY: `dc` is a valid printer DC.
unsafe fn draw(dc: windows::Win32::Graphics::Gdi::HDC, pixels: RgbaImage, job_name: &str) -> Result<(), String> {
    unsafe {
        let page = (GetDeviceCaps(Some(dc), HORZRES), GetDeviceCaps(Some(dc), VERTRES));
        let margin = GetDeviceCaps(Some(dc), LOGPIXELSX) / 2;
        let (x, y, w, h) = fit_page(pixels.dimensions(), page, margin);
        // No point sending more pixels than the printer can place.
        let (sw, sh) = (pixels.width().min(w as u32), pixels.height().min(h as u32));
        let bgra = flatten_on_white(pixels, (sw, sh));
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: sw as i32,
                biHeight: -(sh as i32),
                biPlanes: 1,
                biBitCount: 32,
                ..Default::default()
            },
            ..Default::default()
        };
        let title = HSTRING::from(job_name);
        let doc = DOCINFOW {
            cbSize: size_of::<DOCINFOW>() as i32,
            lpszDocName: PCWSTR(title.as_ptr()),
            ..Default::default()
        };
        if StartDocW(dc, &doc) <= 0 {
            return Err("StartDoc failed".into());
        }
        let ok = StartPage(dc) > 0
            && {
                SetStretchBltMode(dc, HALFTONE);
                StretchDIBits(dc, x, y, w, h, 0, 0, sw as i32, sh as i32, Some(bgra.as_ptr().cast()), &bmi, DIB_RGB_COLORS, SRCCOPY) != 0
            }
            && EndPage(dc) > 0;
        if ok && EndDoc(dc) > 0 {
            Ok(())
        } else {
            AbortDoc(dc);
            Err("the printer rejected the page".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_fit() {
        // 4800 x 6400 px page, 300 px margin: a landscape image is limited by the width and centered.
        assert_eq!(fit_page((4000, 2000), (4800, 6400), 300), (300, 2150, 4200, 2100));
        // A portrait image is limited by the height.
        assert_eq!(fit_page((1000, 4000), (4800, 6400), 300), (1675, 300, 1450, 5800));
        // A small image is enlarged to the page, as Stretch=Uniform does in 1.x.
        assert_eq!(fit_page((10, 10), (4800, 6400), 300), (300, 1100, 4200, 4200));
        // A margin larger than the page leaves a 1 px box instead of a negative size.
        let (_, _, w, h) = fit_page((10, 10), (100, 100), 300);
        assert!(w >= 1 && h >= 1);
    }

    #[test]
    fn transparent_pixels_print_white() {
        let img = RgbaImage::from_fn(2, 1, |x, _| if x == 0 { Rgba([255, 0, 0, 0]) } else { Rgba([0, 0, 255, 255]) });
        assert_eq!(flatten_on_white(img, (2, 1)), vec![255, 255, 255, 255, 255, 0, 0, 255]);
    }

    #[test]
    fn reduction_does_not_darken_transparent_edges() {
        let img = RgbaImage::from_fn(2, 1, |x, _| if x == 0 { Rgba([255, 0, 0, 255]) } else { Rgba([0, 0, 0, 0]) });
        let out = flatten_on_white(img, (1, 1));
        assert_eq!(out.len(), 4);
        assert!(out[0].abs_diff(127) <= 2 && out[1].abs_diff(127) <= 2 && out[2] == 255 && out[3] == 255, "{out:?}");
    }
}
