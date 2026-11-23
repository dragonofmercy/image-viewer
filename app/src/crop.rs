//! Selection math of the inline cropper. Everything is in image pixels; every function is total and its result lies inside the image,
//! at least 1x1.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sel {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handle {
    N,
    S,
    E,
    W,
    NE,
    NW,
    SE,
    SW,
    Move,
}

/// `CropperAspectRatios` of 1.x, same order: label (a `Tr` key for the first two) and ratio (width / height), with the 1.x sentinels
/// 0 for free and -1 for the image's own ratio.
pub const RATIOS: [(&str, f32); 6] = [("TRANSFORM_CROP_FREE", 0.0), ("TRANSFORM_CROP_SAME", -1.0), ("1:1", 1.0), ("16:9", 16.0 / 9.0), ("16:10", 16.0 / 10.0), ("4:3", 4.0 / 3.0)];

/// Ratio of entry `index` of `RATIOS`: None for free (and an unknown index).
pub fn ratio(index: usize, image: (u32, u32)) -> Option<f32> {
    let (w, h) = dims(image);
    match RATIOS.get(index)?.1 {
        0.0 => None,
        -1.0 => Some(w / h),
        r => Some(r),
    }
}

/// Image size as floats, at least 1x1 so every range below is valid.
fn dims(image: (u32, u32)) -> (f32, f32) {
    (image.0.max(1) as f32, image.1.max(1) as f32)
}

/// `v` limited to `lo..=hi`; `lo` wins when float rounding leaves `hi < lo` (`f32::clamp` would panic).
fn between(v: f32, lo: f32, hi: f32) -> f32 {
    v.min(hi).max(lo)
}

fn finite(v: f32) -> f32 {
    if v.is_finite() { v } else { 0.0 }
}

/// Clamps `s` inside the image, at least 1x1.
fn fit(s: Sel, image: (u32, u32)) -> Sel {
    let (iw, ih) = dims(image);
    let (w, h) = (between(finite(s.w), 1.0, iw), between(finite(s.h), 1.0, ih));
    Sel {
        x: between(finite(s.x), 0.0, iw - w),
        y: between(finite(s.y), 0.0, ih - h),
        w,
        h,
    }
}

pub fn full(image: (u32, u32)) -> Sel {
    let (w, h) = dims(image);
    Sel { x: 0.0, y: 0.0, w, h }
}

/// Moving side of a handle on each axis: -1 the low edge (left, top), 1 the high edge, 0 none.
fn sides(handle: Handle) -> (i8, i8) {
    match handle {
        Handle::N => (0, -1),
        Handle::S => (0, 1),
        Handle::E => (1, 0),
        Handle::W => (-1, 0),
        Handle::NE => (1, -1),
        Handle::NW => (-1, -1),
        Handle::SE => (1, 1),
        Handle::SW => (-1, 1),
        Handle::Move => (0, 0),
    }
}

/// One axis of a free resize: the moving edge stops at the image border and 1px before the opposite edge. Returns (start, length).
fn free_axis(pos: f32, len: f32, side: i8, d: f32, limit: f32) -> (f32, f32) {
    let end = pos + len;
    match side {
        -1 => {
            let pos = between(pos + d, 0.0, end - 1.0);
            (pos, end - pos)
        }
        1 => (pos, between(end + d, pos + 1.0, limit) - pos),
        _ => (pos, len),
    }
}

/// Start of a length `len` anchored on the fixed edge of `start..start + old`: the low edge stays when the high one moves, and back.
fn anchored(start: f32, old: f32, len: f32, side: i8) -> f32 {
    if side > 0 { start } else { start + old - len }
}

/// Room for the selection from its fixed edge to the image border on the moving side.
fn room(start: f32, len: f32, side: i8, limit: f32) -> f32 {
    if side > 0 { limit - start } else { start + len }
}

/// Drags `handle` by (dx, dy) image pixels from the selection `start` had when the drag began.
pub fn drag(start: Sel, handle: Handle, dx: f32, dy: f32, ratio: Option<f32>, image: (u32, u32)) -> Sel {
    let s = fit(start, image);
    let (iw, ih) = dims(image);
    let (dx, dy) = (finite(dx), finite(dy));
    let (sx, sy) = sides(handle);
    if handle == Handle::Move {
        return fit(Sel { x: s.x + dx, y: s.y + dy, ..s }, image);
    }
    let Some(r) = ratio.filter(|r| r.is_finite() && *r > 0.0) else {
        let (x, w) = free_axis(s.x, s.w, sx, dx, iw);
        let (y, h) = free_axis(s.y, s.h, sy, dy, ih);
        return fit(Sel { x, y, w, h }, image);
    };
    // Smallest width that keeps both sides at least 1px.
    let min_w = r.max(1.0);
    let sel = if sx != 0 && sy != 0 {
        // Corner: the axis dragged further (in ratio terms) drives the size, the opposite corner stays.
        let w = if dx.abs() >= dy.abs() * r { s.w + f32::from(sx) * dx } else { (s.h + f32::from(sy) * dy) * r };
        let w = w.max(min_w).min(room(s.x, s.w, sx, iw).min(room(s.y, s.h, sy, ih) * r));
        let h = w / r;
        Sel {
            x: anchored(s.x, s.w, w, sx),
            y: anchored(s.y, s.h, h, sy),
            w,
            h,
        }
    } else if sx != 0 {
        // Left or right edge: the opposite edge stays, the height follows centered on the selection.
        let w = (s.w + f32::from(sx) * dx).max(min_w).min(room(s.x, s.w, sx, iw).min(ih * r));
        let h = (w / r).min(ih);
        let y = between(s.y + (s.h - h) / 2.0, 0.0, ih - h);
        Sel { x: anchored(s.x, s.w, w, sx), y, w, h }
    } else {
        let h = (s.h + f32::from(sy) * dy).max(min_w / r).min(room(s.y, s.h, sy, ih).min(iw / r));
        let w = (h * r).min(iw);
        let x = between(s.x + (s.w - w) / 2.0, 0.0, iw - w);
        Sel { x, y: anchored(s.y, s.h, h, sy), w, h }
    };
    fit(sel, image)
}

/// The largest selection of `ratio` inside the image, centered on `sel` (shifted back inside the image). Free keeps `sel`.
pub fn apply_ratio(sel: Sel, ratio: Option<f32>, image: (u32, u32)) -> Sel {
    let s = fit(sel, image);
    let Some(r) = ratio.filter(|r| r.is_finite() && *r > 0.0) else { return s };
    let (iw, ih) = dims(image);
    let w = iw.min(ih * r);
    let h = w / r;
    fit(
        Sel {
            x: s.x + (s.w - w) / 2.0,
            y: s.y + (s.h - h) / 2.0,
            w,
            h,
        },
        image,
    )
}

/// The selection as whole pixels (x, y, width, height): rounded, inside the image, at least 1x1.
pub fn to_pixels(sel: Sel, image: (u32, u32)) -> (u32, u32, u32, u32) {
    let s = fit(sel, image);
    let (iw, ih) = dims(image);
    let axis = |pos: f32, len: f32, limit: f32| {
        // Round the length, not the far end, so equal sides stay equal.
        let len = len.round().clamp(1.0, limit);
        let start = pos.round().clamp(0.0, limit - len);
        (start as u32, len as u32)
    };
    let (x, w) = axis(s.x, s.w, iw);
    let (y, h) = axis(s.y, s.h, ih);
    (x, y, w, h)
}

#[cfg(test)]
mod tests {
    use super::*;

    const IMAGE: (u32, u32) = (400, 300);

    fn sel(x: f32, y: f32, w: f32, h: f32) -> Sel {
        Sel { x, y, w, h }
    }

    #[test]
    fn full_covers_the_image() {
        assert_eq!(full(IMAGE), sel(0.0, 0.0, 400.0, 300.0));
    }

    #[test]
    fn edge_stops_at_the_image_border() {
        assert_eq!(drag(full(IMAGE), Handle::E, 50.0, 0.0, None, IMAGE).w, 400.0);
    }

    #[test]
    fn edge_past_its_opposite_stops_one_pixel_before() {
        let s = drag(full(IMAGE), Handle::W, 500.0, 0.0, None, IMAGE);
        assert_eq!((s.x, s.w), (399.0, 1.0));
    }

    #[test]
    fn move_is_clamped_without_resizing() {
        assert_eq!(drag(full(IMAGE), Handle::Move, -10.0, -10.0, None, IMAGE), full(IMAGE));
        assert_eq!(drag(sel(10.0, 10.0, 100.0, 50.0), Handle::Move, 1000.0, -5.0, None, IMAGE), sel(300.0, 5.0, 100.0, 50.0));
    }

    #[test]
    fn corner_with_ratio_keeps_the_opposite_corner() {
        assert_eq!(drag(sel(0.0, 0.0, 300.0, 300.0), Handle::SE, -100.0, 0.0, Some(1.0), IMAGE), sel(0.0, 0.0, 200.0, 200.0));
        assert_eq!(drag(sel(100.0, 0.0, 300.0, 300.0), Handle::NW, 50.0, 0.0, Some(1.0), IMAGE), sel(150.0, 50.0, 250.0, 250.0));
    }

    #[test]
    fn edge_with_ratio_centers_the_other_axis() {
        let s = drag(sel(0.0, 50.0, 200.0, 200.0), Handle::E, -100.0, 0.0, Some(1.0), IMAGE);
        assert_eq!(s, sel(0.0, 100.0, 100.0, 100.0));
        // Growing past the image height shrinks to fit.
        let s = drag(sel(0.0, 50.0, 200.0, 200.0), Handle::E, 300.0, 0.0, Some(1.0), IMAGE);
        assert_eq!(s, sel(0.0, 0.0, 300.0, 300.0));
    }

    #[test]
    fn apply_ratio_is_the_largest_centered_fit() {
        assert_eq!(apply_ratio(full(IMAGE), Some(1.0), IMAGE), sel(50.0, 0.0, 300.0, 300.0));
        assert_eq!(apply_ratio(full(IMAGE), Some(16.0 / 9.0), IMAGE), sel(0.0, 37.5, 400.0, 225.0));
        assert_eq!(apply_ratio(sel(10.0, 10.0, 20.0, 20.0), None, IMAGE), sel(10.0, 10.0, 20.0, 20.0));
        // Centered on the selection, shifted back inside the image.
        assert_eq!(apply_ratio(sel(0.0, 0.0, 10.0, 10.0), Some(1.0), IMAGE), sel(0.0, 0.0, 300.0, 300.0));
    }

    #[test]
    fn to_pixels_is_rounded_and_never_empty() {
        assert_eq!(to_pixels(sel(0.4, 0.4, 0.2, 0.2), IMAGE), (0, 0, 1, 1));
        assert_eq!(to_pixels(sel(10.4, 20.6, 99.8, 50.0), IMAGE), (10, 21, 100, 50));
        assert_eq!(to_pixels(sel(399.9, 299.9, 5.0, 5.0), IMAGE), (395, 295, 5, 5));
    }

    #[test]
    fn to_pixels_keeps_equal_lengths_equal() {
        let (_, _, w, h) = to_pixels(sel(0.0, 99.7, 100.6, 100.6), IMAGE);
        assert_eq!((w, h), (101, 101));
    }

    #[test]
    fn ratio_entries_follow_cropper_aspect_ratios() {
        let labels: Vec<&str> = RATIOS.iter().map(|r| r.0).collect();
        assert_eq!(labels, ["TRANSFORM_CROP_FREE", "TRANSFORM_CROP_SAME", "1:1", "16:9", "16:10", "4:3"]);
        assert_eq!(ratio(0, IMAGE), None);
        assert_eq!(ratio(1, IMAGE), Some(400.0 / 300.0));
        assert_eq!(ratio(3, IMAGE), Some(16.0 / 9.0));
        assert_eq!(ratio(99, IMAGE), None);
    }

    #[test]
    fn non_finite_deltas_change_nothing() {
        let square = apply_ratio(full(IMAGE), Some(1.0), IMAGE);
        assert_eq!(drag(square, Handle::SE, f32::NAN, f32::INFINITY, Some(1.0), IMAGE), square);
        assert_eq!(drag(full(IMAGE), Handle::Move, f32::NAN, 0.0, None, IMAGE), full(IMAGE));
    }

    const HANDLES: [Handle; 9] = [Handle::N, Handle::S, Handle::E, Handle::W, Handle::NE, Handle::NW, Handle::SE, Handle::SW, Handle::Move];

    fn assert_inside(s: Sel, image: (u32, u32), context: &str) {
        let (iw, ih) = (image.0 as f32, image.1 as f32);
        assert!(s.x >= 0.0 && s.y >= 0.0 && s.w >= 1.0 && s.h >= 1.0, "{s:?} {image:?} {context}");
        assert!(s.x + s.w <= iw && s.y + s.h <= ih, "{s:?} {image:?} {context}");
    }

    /// A fixed ratio holds up to rounding: one side is off by under 1px of length (on tiny images the sides cannot both be whole and exact).
    fn assert_ratio(s: Sel, r: Option<f32>, context: &str) {
        let Some(r) = r else { return };
        let off = (s.w - s.h * r).abs().min((s.h - s.w / r).abs());
        assert!(off < 1.0 + 1e-3, "ratio {r}: {s:?} off by {off} ({context})");
    }

    /// Reviewer case: with the image's own ratio, h capped at iw / r gave w = h * r one ulp above iw, and `clamp` panicked.
    #[test]
    fn same_ratio_edge_drags_on_1013x600_do_not_panic() {
        let image = (1013, 600);
        let r = ratio(1, image);
        let start = apply_ratio(full(image), r, image);
        let small = apply_ratio(Sel { x: 400.0, y: 250.0, w: 100.0, h: 100.0 }, None, image);
        for handle in HANDLES {
            for (dx, dy) in [(-1000.0, -1000.0), (1000.0, 1000.0), (-1000.0, 1000.0), (1000.0, -1000.0), (0.0, -1000.0), (0.0, 1000.0), (-1000.0, 0.0), (1000.0, 0.0)] {
                for from in [start, drag(small, Handle::SE, 0.0, 0.0, r, image)] {
                    assert_inside(drag(from, handle, dx, dy, r, image), image, &format!("{handle:?} {dx} {dy}"));
                }
            }
        }
    }

    #[test]
    fn every_size_ratio_and_handle_stays_inside() {
        let awkward = [(3, 21), (1013, 600), (1013, 1200), (1015, 600), (4284, 5712), (7680, 5200)];
        let sizes = (1..=64u32).flat_map(|w| (1..=64u32).map(move |h| (w, h))).chain(awkward);
        let deltas = [(-1e5, -1e5), (1e5, 1e5), (-1e5, 1e5), (1e5, -1e5), (0.0, -1e5), (0.0, 1e5), (-1e5, 0.0), (1e5, 0.0)];
        for image in sizes {
            for r in [ratio(1, image), Some(1.0), Some(16.0 / 9.0), Some(9.0 / 16.0), Some(0.1), Some(10.0)] {
                let large = apply_ratio(full(image), r, image);
                let small = drag(large, Handle::SE, -1e5, -1e5, r, image);
                assert_inside(large, image, "apply_ratio");
                assert_ratio(large, r, "apply_ratio");
                assert_inside(small, image, "shrunk");
                assert_ratio(small, r, "shrunk");
                for handle in HANDLES {
                    for (dx, dy) in deltas {
                        for from in [large, small] {
                            let moved = drag(from, handle, dx, dy, r, image);
                            assert_inside(moved, image, &format!("{handle:?} {dx} {dy} {r:?}"));
                            if handle != Handle::Move {
                                assert_ratio(moved, r, &format!("{handle:?} {dx} {dy} {image:?}"));
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn random_drags_stay_valid() {
        let mut seed: u32 = 12345;
        let mut next = move || {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            seed >> 8
        };
        // Index 0 is free, 1 the image's own ratio (as `RATIOS`), the rest fixed.
        let fixed = [1.0, 16.0 / 9.0, 4.0 / 3.0, 0.25];
        for _ in 0..10 {
            let image = (16 + next() % 2000, 16 + next() % 2000);
            let (iw, ih) = (image.0 as f32, image.1 as f32);
            let mut choice = 0;
            let mut s = full(image);
            for _ in 0..200 {
                // The ratio changes now and then; the selection is only re-fitted to it then, as the combo does.
                if next() % 8 == 0 {
                    choice = next() as usize % (fixed.len() + 2);
                    let r = if choice < 2 { ratio(choice, image) } else { Some(fixed[choice - 2]) };
                    s = apply_ratio(s, r, image);
                }
                let r = if choice < 2 { ratio(choice, image) } else { Some(fixed[choice - 2]) };
                let handle = HANDLES[next() as usize % HANDLES.len()];
                let dx = (next() % 4000) as f32 - 2000.0 + 0.37;
                let dy = (next() % 4000) as f32 - 2000.0 - 0.61;
                s = drag(s, handle, dx, dy, r, image);
                assert!(s.x >= 0.0 && s.y >= 0.0 && s.w >= 1.0 && s.h >= 1.0, "{s:?} {image:?}");
                assert!(s.x + s.w <= iw && s.y + s.h <= ih, "{s:?} {image:?}");
                if let Some(r) = r {
                    assert!((s.w - s.h * r).abs() <= 1.0, "{s:?} {r} {handle:?} {image:?}");
                }
            }
        }
    }
}
