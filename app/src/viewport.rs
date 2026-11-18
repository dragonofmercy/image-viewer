//! Zoom and pan math of the image view (`ScrollViewer` semantics of 1.x). All sizes are logical px, every function is total.
//! `View.offset_*` is the scroll offset: the image is centered on an axis where it is smaller than the view, and the offset is then 0.

use viewer_core::format::round_to_ten;

pub const MIN_ZOOM: f32 = 0.1;
pub const MAX_ZOOM: f32 = 10.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View {
    pub zoom: f32,
    pub offset_x: f32,
    pub offset_y: f32,
}

type Size = (f32, f32);

fn is_empty(image: (u32, u32), view: Size) -> bool {
    image.0 == 0 || image.1 == 0 || view.0.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) || view.1.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater)
}

/// 1.0 unless the image exceeds the view, then the smaller ratio. 1.0 as well for an empty image or view.
pub fn fit_zoom(image: (u32, u32), view: Size) -> f32 {
    if is_empty(image, view) {
        return 1.0;
    }
    let (iw, ih) = (image.0 as f32, image.1 as f32);
    if iw > view.0 || ih > view.1 { (view.1 / ih).min(view.0 / iw) } else { 1.0 }
}

/// The fit zoom may be below `MIN_ZOOM` for a huge image: it must stay reachable.
fn min_zoom(image: (u32, u32), view: Size) -> f32 {
    MIN_ZOOM.min(fit_zoom(image, view))
}

fn content(zoom: f32, image: (u32, u32)) -> Size {
    (image.0 as f32 * zoom, image.1 as f32 * zoom)
}

fn clamp_offset(offset: f32, content: f32, view: f32) -> f32 {
    let max = (content - view).max(0.0);
    if offset.is_finite() { offset.clamp(0.0, max) } else { 0.0 }
}

/// Leading gap of the content on one axis (centering).
fn lead(content: f32, view: f32) -> f32 {
    ((view - content) / 2.0).max(0.0)
}

pub fn clamp(v: View, image: (u32, u32), view: Size) -> View {
    let zoom = if v.zoom.is_finite() { v.zoom.clamp(min_zoom(image, view), MAX_ZOOM) } else { 1.0 };
    let (cw, ch) = content(zoom, image);
    View {
        zoom,
        offset_x: clamp_offset(v.offset_x, cw, view.0),
        offset_y: clamp_offset(v.offset_y, ch, view.1),
    }
}

/// Zooms to `new_zoom` keeping the image pixel under `anchor` (view coordinates) fixed. Returns the clamped `v` when the zoom does not change.
fn zoom_at(v: View, new_zoom: f32, anchor: Size, image: (u32, u32), view: Size) -> View {
    let v = clamp(v, image, view);
    let nz = if new_zoom.is_finite() { new_zoom.clamp(min_zoom(image, view), MAX_ZOOM) } else { v.zoom };
    if nz == v.zoom || is_empty(image, view) {
        return v;
    }
    let axis = |offset: f32, size: u32, a: f32, view: f32| {
        let (old, new) = (size as f32 * v.zoom, size as f32 * nz);
        let pixel = (offset + a - lead(old, view)) / v.zoom;
        pixel * nz - (a - lead(new, view))
    };
    clamp(
        View {
            zoom: nz,
            offset_x: axis(v.offset_x, image.0, anchor.0, view.0),
            offset_y: axis(v.offset_y, image.1, anchor.1, view.1),
        },
        image,
        view,
    )
}

/// Wheel zoom: factor `1.1^(delta/120)`, anchored on the cursor.
pub fn wheel(v: View, delta: i32, cursor: Size, image: (u32, u32), view: Size) -> View {
    let factor = 1.1f64.powf(f64::from(delta) / 120.0) as f32;
    zoom_at(v, v.zoom * factor, cursor, image, view)
}

/// Button zoom (`Context.Zoom` in 1.x), anchored on the view center.
pub fn step(v: View, factor: f32, image: (u32, u32), view: Size) -> View {
    let zoom = round_to_ten(f64::from(v.zoom + factor) * 100.0) / 100.0;
    zoom_at(v, zoom, (view.0 / 2.0, view.1 / 2.0), image, view)
}

/// Drag by the pointer movement (dx, dy): the image follows the pointer.
pub fn pan(v: View, dx: f32, dy: f32, image: (u32, u32), view: Size) -> View {
    clamp(
        View {
            offset_x: v.offset_x - dx,
            offset_y: v.offset_y - dy,
            ..v
        },
        image,
        view,
    )
}

/// Image rectangle (x, y, w, h) in view pixels.
pub fn image_rect(v: View, image: (u32, u32), view: Size) -> (f32, f32, f32, f32) {
    let (w, h) = content(v.zoom, image);
    let pos = |content: f32, view: f32, offset: f32| if content < view { (view - content) / 2.0 } else { -offset };
    (pos(w, view.0, v.offset_x), pos(h, view.1, v.offset_y), w, h)
}

/// True when the zoom is the fit zoom (the 100% button then shows instead of the adjust button).
pub fn is_fit(v: &View, image: (u32, u32), view: Size) -> bool {
    (v.zoom - fit_zoom(image, view)).abs() < 0.0005
}

/// Zoom `zoom` centered in the view (offsets of the centered scroll position).
pub fn centered(zoom: f32, image: (u32, u32), view: Size) -> View {
    let (cw, ch) = content(zoom, image);
    clamp(
        View {
            zoom,
            offset_x: (cw - view.0) / 2.0,
            offset_y: (ch - view.1) / 2.0,
        },
        image,
        view,
    )
}

/// Index of the smallest pyramid level (scale 1, 1/2, 1/4...) whose scale is >= the on-screen scale; 0 when zoomed in.
pub fn level_for(zoom_times_scale: f32, levels: usize) -> usize {
    let last = levels.saturating_sub(1);
    if zoom_times_scale.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
        return last;
    }
    let mut level = 0;
    while level < last && 0.5f32.powi(level as i32 + 1) >= zoom_times_scale {
        level += 1;
    }
    level
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIEW: Size = (800.0, 600.0);

    fn view(zoom: f32, x: f32, y: f32) -> View {
        View { zoom, offset_x: x, offset_y: y }
    }

    #[test]
    fn fit_zoom_cases() {
        assert_eq!(fit_zoom((100, 100), VIEW), 1.0);
        assert_eq!(fit_zoom((1600, 600), VIEW), 0.5);
        assert_eq!(fit_zoom((400, 1200), VIEW), 0.5);
        assert_eq!(fit_zoom((1600, 1200), (0.0, 0.0)), 1.0);
        assert_eq!(fit_zoom((0, 0), VIEW), 1.0);
    }

    #[test]
    fn wheel_keeps_pixel_under_cursor() {
        let image = (4000, 3000);
        let v = view(1.0, 500.0, 400.0);
        let cursor = (300.0, 200.0);
        let (x0, y0, _, _) = image_rect(v, image, VIEW);
        let pixel = ((cursor.0 - x0) / v.zoom, (cursor.1 - y0) / v.zoom);
        for delta in [120, -120, 240, -60] {
            let n = wheel(v, delta, cursor, image, VIEW);
            assert_ne!(n.zoom, v.zoom);
            let (x1, y1, _, _) = image_rect(n, image, VIEW);
            assert!((x1 + pixel.0 * n.zoom - cursor.0).abs() < 0.5, "x delta {delta}");
            assert!((y1 + pixel.1 * n.zoom - cursor.1).abs() < 0.5, "y delta {delta}");
        }
    }

    #[test]
    fn wheel_at_limits_is_identity() {
        let image = (4000, 3000);
        let max = view(MAX_ZOOM, 1000.0, 700.0);
        assert_eq!(wheel(max, 120, (10.0, 10.0), image, VIEW), max);
        let min = view(MIN_ZOOM, 0.0, 0.0);
        assert_eq!(wheel(min, -120, (10.0, 10.0), image, VIEW), min);
    }

    #[test]
    fn wheel_on_small_image_stays_centered_and_finite() {
        let image = (100, 100);
        for delta in [120, -120, 1200] {
            let n = wheel(view(1.0, 0.0, 0.0), delta, (700.0, 50.0), image, VIEW);
            assert_eq!((n.offset_x, n.offset_y), (0.0, 0.0));
            assert!(n.zoom.is_finite());
        }
        assert!(wheel(view(1.0, 0.0, 0.0), 120, (1.0, 1.0), image, (0.0, 0.0)).zoom.is_finite());
    }

    #[test]
    fn pan_clamps_every_edge() {
        let image = (1600, 1200);
        let v = view(1.0, 100.0, 100.0);
        assert_eq!(pan(v, 1000.0, 1000.0, image, VIEW), view(1.0, 0.0, 0.0));
        let n = pan(v, -5000.0, -5000.0, image, VIEW);
        assert_eq!((n.offset_x, n.offset_y), (800.0, 600.0));
        let n = pan(v, 30.0, -20.0, image, VIEW);
        assert_eq!((n.offset_x, n.offset_y), (70.0, 120.0));
    }

    #[test]
    fn step_matches_csharp() {
        let image = (100, 100);
        // RoundToTen((0.33 + 0.1) * 100) / 100
        assert_eq!(step(view(0.33, 0.0, 0.0), 0.1, image, VIEW).zoom, 0.4);
        assert_eq!(step(view(1.0, 0.0, 0.0), 0.1, image, VIEW).zoom, 1.1);
        assert_eq!(step(view(MIN_ZOOM, 0.0, 0.0), -0.1, image, VIEW).zoom, MIN_ZOOM);
    }

    #[test]
    fn level_for_cases() {
        assert_eq!(level_for(0.3, 4), 1);
        assert_eq!(level_for(1.0, 4), 0);
        assert_eq!(level_for(3.0, 4), 0);
        assert_eq!(level_for(0.01, 3), 2);
        assert_eq!(level_for(0.0, 3), 2);
        assert_eq!(level_for(0.5, 3), 1);
        assert_eq!(level_for(0.3, 0), 0);
    }

    #[test]
    fn is_fit_and_centered() {
        let image = (1600, 1200);
        assert!(is_fit(&view(0.5, 0.0, 0.0), image, VIEW));
        assert!(!is_fit(&view(1.0, 0.0, 0.0), image, VIEW));
        assert_eq!(centered(1.0, image, VIEW), view(1.0, 400.0, 300.0));
    }
}
