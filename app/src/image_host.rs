//! View controller of the image area: owns the `View` (zoom and offset), the display pyramid, and pushes the result to the Slint properties.
//!
//! Interface for the loading and editing tasks: `ImageHost::new(&ui)`, `show(&RgbaImage)` (displays at the fit zoom), `clear()`, `fit()`, `fit_after_resize()`.
//! Zoom and pan callbacks (wheel, drag, resize, `Commands.adjust/zoom-full/zoom-in/zoom-out`) are registered by `new`; `set_locked` turns
//! them off for the cropper.

use crate::viewport::{self, View};
use crate::{AppState, Commands, MainWindow};
use image::{ImageBuffer, Rgba, RgbaImage, imageops};
use slint::{ComponentHandle, Image, Rgba8Pixel, SharedPixelBuffer, SharedString};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Renderer image side limit: a level above it is never uploaded.
const MAX_TEXTURE: u32 = 16384;
/// Levels stop once the longest edge is at or under this.
const LEVEL_STOP: u32 = 1024;

type Buffer = SharedPixelBuffer<Rgba8Pixel>;

thread_local! {
    // Worker results come back through the event loop, which can only carry `Send` closures: they reach the host through this.
    static CURRENT: RefCell<Weak<ImageHost>> = const { RefCell::new(Weak::new()) };
}

/// Sizes of the pyramid levels: level 0 is the image, each next one halves both sides until the longest edge is <= `LEVEL_STOP`.
fn level_sizes(w: u32, h: u32) -> Vec<(u32, u32)> {
    let mut sizes = vec![(w, h)];
    while let Some(&(lw, lh)) = sizes.last()
        && lw.max(lh) > LEVEL_STOP
    {
        sizes.push(((lw / 2).max(1), (lh / 2).max(1)));
    }
    sizes
}

/// First level that fits in a texture (the last one when none does, which cannot happen for realistic sizes).
fn first_displayable(sizes: &[(u32, u32)]) -> usize {
    sizes.iter().position(|&(w, h)| w.max(h) <= MAX_TEXTURE).unwrap_or(sizes.len().saturating_sub(1))
}

/// The wanted level if built, else the nearest finer built one, else the nearest coarser built one.
fn pick_level(present: &[bool], want: usize) -> Option<usize> {
    if present.is_empty() {
        return None;
    }
    let want = want.min(present.len() - 1);
    (0..=want).rev().chain(want + 1..present.len()).find(|&i| present[i])
}

fn has_area(view: (f32, f32)) -> bool {
    view.0 > 0.0 && view.1 > 0.0
}

/// A fit asked while the view had no area (before the first layout, minimized) waits for a view with an area.
/// Returns (fit now, still pending).
fn resolve_pending(pending: bool, view: (f32, f32)) -> (bool, bool) {
    let ready = has_area(view);
    (pending && ready, pending && !ready)
}

fn premultiply(w: u32, h: u32, bytes: &[u8]) -> Option<RgbaImage> {
    let data = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| {
            let a = u32::from(p[3]);
            [p[0], p[1], p[2]].map(|c| ((u32::from(c) * a + 127) / 255) as u8).into_iter().chain([p[3]])
        })
        .collect();
    RgbaImage::from_raw(w, h, data)
}

fn unpremultiply(bytes: &[u8]) -> Vec<u8> {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| {
            let a = u32::from(p[3]);
            let rgb = [p[0], p[1], p[2]].map(|c| (u32::from(c) * 255 + a / 2).checked_div(a).map_or(0, |v| v.min(255) as u8));
            rgb.into_iter().chain([p[3]])
        })
        .collect()
}

/// Builds levels 1.. of `sizes` from the level 0 bytes (no copy unless the image has transparency, where the chain runs premultiplied
/// to avoid dark fringes). `emit(index, buffer)` receives each level at or after `first` and returns false to stop; `stale` is checked
/// before each level and ends the build early.
fn build_levels(w: u32, h: u32, bytes: &[u8], sizes: &[(u32, u32)], first: usize, stale: &dyn Fn() -> bool, emit: &mut dyn FnMut(usize, Buffer) -> bool) {
    let Some(borrowed) = ImageBuffer::<Rgba<u8>, &[u8]>::from_raw(w, h, bytes) else { return };
    let alpha = bytes.as_chunks::<4>().0.iter().any(|p| p[3] != 255);
    let mut current: Option<RgbaImage> = if alpha {
        match premultiply(w, h, bytes) {
            Some(image) => Some(image),
            None => return,
        }
    } else {
        None
    };
    for (index, &(lw, lh)) in sizes.iter().enumerate().skip(1) {
        if stale() {
            return;
        }
        let next = match &current {
            Some(image) => imageops::thumbnail(image, lw, lh),
            None => imageops::thumbnail(&borrowed, lw, lh),
        };
        if index >= first {
            let buffer = if alpha {
                Buffer::clone_from_slice(&unpremultiply(next.as_raw()), next.width(), next.height())
            } else {
                Buffer::clone_from_slice(next.as_raw(), next.width(), next.height())
            };
            if stale() || !emit(index, buffer) {
                return;
            }
        }
        current = Some(next);
    }
}

#[derive(Default)]
struct State {
    image: (u32, u32),
    levels: Vec<Option<Image>>,
    shown: Option<usize>,
    view: Option<View>,
    fit_pending: bool,
}

pub struct ImageHost {
    ui: slint::Weak<MainWindow>,
    state: RefCell<State>,
    /// Incremented by each `show`/`clear`: workers and levels of an older one stop or are dropped.
    token: Arc<AtomicU64>,
    /// Wheel delta below one unit (touchpads), carried to the next event.
    wheel_rest: Cell<f32>,
    /// The cropper is open: zoom and pan do not move, a resize refits.
    locked: Cell<bool>,
}

impl ImageHost {
    pub fn new(ui: &MainWindow) -> Rc<Self> {
        let host = Rc::new(Self {
            ui: ui.as_weak(),
            state: RefCell::default(),
            token: Arc::new(AtomicU64::new(0)),
            wheel_rest: Cell::new(0.0),
            locked: Cell::new(false),
        });
        CURRENT.with(|c| *c.borrow_mut() = Rc::downgrade(&host));
        let h = host.clone();
        ui.on_view_wheel(move |delta, x, y| {
            if h.locked.get() {
                return;
            }
            let total = h.wheel_rest.get() + delta;
            let whole = total.trunc();
            h.wheel_rest.set(total - whole);
            if whole != 0.0 {
                h.update(|v, image, view| viewport::wheel(v, whole as i32, (x, y), image, view));
            }
        });
        let h = host.clone();
        ui.on_view_drag(move |dx, dy| h.unlocked_update(|v, image, view| viewport::pan(v, dx, dy, image, view)));
        let h = host.clone();
        ui.on_view_resized(move || h.resized());
        let commands = ui.global::<Commands>();
        let h = host.clone();
        commands.on_adjust(move || {
            if !h.locked.get() {
                h.fit();
            }
        });
        let h = host.clone();
        commands.on_zoom_full(move || h.unlocked_update(|_, image, view| viewport::centered(1.0, image, view)));
        let h = host.clone();
        commands.on_zoom_in(move || h.unlocked_update(|v, image, view| viewport::step(v, 0.1, image, view)));
        let h = host.clone();
        commands.on_zoom_out(move || h.unlocked_update(|v, image, view| viewport::step(v, -0.1, image, view)));
        host
    }

    /// Locks zoom and pan while the cropper is open (the controller sets it from the document state).
    pub fn set_locked(&self, locked: bool) {
        self.locked.set(locked);
    }

    /// Current zoom (view px per image px), 1.0 without an image.
    pub fn zoom(&self) -> f32 {
        self.state.borrow().view.map_or(1.0, |v| v.zoom)
    }

    /// A user zoom or pan: ignored while locked.
    fn unlocked_update(&self, next: impl FnOnce(View, (u32, u32), (f32, f32)) -> View) {
        if !self.locked.get() {
            self.update(next);
        }
    }

    /// Displays `pixels` at the fit zoom. Smaller levels are built on a worker thread and attached when ready.
    pub fn show(&self, pixels: &RgbaImage) {
        self.show_with(pixels, true);
    }

    /// Like `show` for a picture that will be played: level 0 only, so `set_frame` can swap the pixels without a pyramid to rebuild.
    pub fn show_animated(&self, pixels: &RgbaImage) {
        self.show_with(pixels, false);
    }

    /// Replaces the displayed pixels of an animated picture, keeping zoom and offsets. Ignored when the size differs from the shown image.
    pub fn set_frame(&self, pixels: Buffer) {
        let mut s = self.state.borrow_mut();
        if s.image != (pixels.width(), pixels.height()) || s.levels.len() != 1 {
            return;
        }
        let image = Image::from_rgba8(pixels);
        s.levels[0] = Some(image.clone());
        s.shown = Some(0);
        drop(s);
        if let Some(ui) = self.ui.upgrade() {
            ui.set_view_source(image);
        }
    }

    fn show_with(&self, pixels: &RgbaImage, pyramid: bool) {
        let (w, h) = pixels.dimensions();
        if w == 0 || h == 0 {
            return self.clear();
        }
        let sizes = if pyramid { level_sizes(w, h) } else { vec![(w, h)] };
        let first = first_displayable(&sizes);
        // One copy of the pixels, shared by level 0 and the worker (reference counted).
        let buffer = Buffer::clone_from_slice(pixels.as_raw(), w, h);
        let mut levels = vec![None; sizes.len()];
        if first == 0 {
            levels[0] = Some(Image::from_rgba8(buffer.clone()));
        } else if let Some(ui) = self.ui.upgrade() {
            // Level 0 is not displayable: do not keep the previous image stretched until the first level arrives.
            ui.set_view_source(Image::default());
        }
        let token = self.token.fetch_add(1, Ordering::SeqCst) + 1;
        *self.state.borrow_mut() = State { image: (w, h), levels, ..State::default() };
        self.fit();
        if sizes.len() > 1 {
            let latest = self.token.clone();
            std::thread::spawn(move || {
                let stale = || latest.load(Ordering::SeqCst) != token;
                let mut emit = |index: usize, level: Buffer| {
                    slint::invoke_from_event_loop(move || {
                        if let Some(host) = CURRENT.with(|c| c.borrow().upgrade()) {
                            host.attach(token, index, level);
                        }
                    })
                    .is_ok()
                };
                build_levels(w, h, buffer.as_bytes(), &sizes, first, &stale, &mut emit);
            });
        }
    }

    /// Removes the image and resets the zoom indicators.
    pub fn clear(&self) {
        self.token.fetch_add(1, Ordering::SeqCst);
        *self.state.borrow_mut() = State::default();
        if let Some(ui) = self.ui.upgrade() {
            ui.set_view_source(Image::default());
            ui.set_view_w(0.0);
            ui.set_view_h(0.0);
            let app = ui.global::<AppState>();
            app.set_zoom_text("100%".into());
            app.set_show_adjust(false);
        }
    }

    /// Zoom to fit the view at offset 0. With a view that has no area yet, the fit is applied at the first resize that gives it one.
    pub fn fit(&self) {
        if let Some(ui) = self.ui.upgrade() {
            self.state.borrow_mut().fit_pending = !has_area((ui.get_view_width(), ui.get_view_height()));
        }
        self.update(|_, image, view| View {
            zoom: viewport::fit_zoom(image, view),
            offset_x: 0.0,
            offset_y: 0.0,
        });
    }

    /// Fits at the next resize of the view: after a window state change (fullscreen) the view gets its new size only once the
    /// native resize events, queued behind the current handler, have been processed.
    pub fn fit_after_resize(&self) {
        self.state.borrow_mut().fit_pending = true;
    }

    fn resized(&self) {
        let Some(ui) = self.ui.upgrade() else { return };
        let (fit_now, pending) = resolve_pending(self.state.borrow().fit_pending, (ui.get_view_width(), ui.get_view_height()));
        self.state.borrow_mut().fit_pending = pending;
        // The cropper keeps the image fitted: its selection is in image pixels, so the overlay follows.
        if fit_now || self.locked.get() { self.fit() } else { self.update(viewport::clamp) }
    }

    fn attach(&self, token: u64, index: usize, buffer: Buffer) {
        if self.token.load(Ordering::SeqCst) != token {
            return;
        }
        {
            let mut s = self.state.borrow_mut();
            if index >= s.levels.len() {
                return;
            }
            s.levels[index] = Some(Image::from_rgba8(buffer));
            s.shown = None;
        }
        self.update(|v, _, _| v);
    }

    /// Computes the next view from the current one, clamps it and pushes everything to the UI. No-op without an image.
    fn update(&self, next: impl FnOnce(View, (u32, u32), (f32, f32)) -> View) {
        let Some(ui) = self.ui.upgrade() else { return };
        let view_size = (ui.get_view_width(), ui.get_view_height());
        let scale = ui.window().scale_factor();
        let mut s = self.state.borrow_mut();
        if s.levels.is_empty() {
            return;
        }
        let image = s.image;
        let current = s.view.unwrap_or(View { zoom: 1.0, offset_x: 0.0, offset_y: 0.0 });
        let view = viewport::clamp(next(current, image, view_size), image, view_size);
        s.view = Some(view);
        let (x, y, w, h) = viewport::image_rect(view, image, view_size);
        let present: Vec<bool> = s.levels.iter().map(Option::is_some).collect();
        let level = pick_level(&present, viewport::level_for(view.zoom * scale, present.len()));
        let source = if level != s.shown { level.and_then(|i| s.levels[i].clone()) } else { None };
        if source.is_some() {
            s.shown = level;
        }
        let adjust = !viewport::is_fit(&view, image, view_size);
        drop(s);
        ui.set_view_x(x);
        ui.set_view_y(y);
        ui.set_view_w(w);
        ui.set_view_h(h);
        ui.set_view_smooth(view.zoom < 4.0);
        if let Some(source) = source {
            ui.set_view_source(source);
        }
        let app = ui.global::<AppState>();
        app.set_zoom_text(SharedString::from(format!("{}%", (view.zoom * 100.0).round_ties_even())));
        app.set_show_adjust(adjust);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn run(w: u32, h: u32, bytes: &[u8], stale: &dyn Fn() -> bool) -> Vec<(usize, u32, u32, Vec<u8>)> {
        let sizes = level_sizes(w, h);
        let mut out = Vec::new();
        build_levels(w, h, bytes, &sizes, first_displayable(&sizes), stale, &mut |i, b| {
            out.push((i, b.width(), b.height(), b.as_bytes().to_vec()));
            true
        });
        out
    }

    #[test]
    fn level_sizes_halve_until_1024() {
        assert_eq!(level_sizes(800, 600), vec![(800, 600)]);
        assert_eq!(level_sizes(1024, 10), vec![(1024, 10)]);
        assert_eq!(level_sizes(4000, 3000), vec![(4000, 3000), (2000, 1500), (1000, 750)]);
        assert_eq!(level_sizes(5000, 1), vec![(5000, 1), (2500, 1), (1250, 1), (625, 1)]);
    }

    #[test]
    fn oversized_level_zero_is_skipped() {
        let sizes = level_sizes(20000, 100);
        assert_eq!(first_displayable(&sizes), 1);
        assert_eq!(first_displayable(&level_sizes(16384, 10)), 0);
    }

    #[test]
    fn pick_level_falls_back() {
        assert_eq!(pick_level(&[true, false, false], 2), Some(0));
        assert_eq!(pick_level(&[true, true, true], 1), Some(1));
        assert_eq!(pick_level(&[false, false, true], 0), Some(2));
        assert_eq!(pick_level(&[false], 0), None);
        assert_eq!(pick_level(&[], 0), None);
    }

    #[test]
    fn pending_fit_waits_for_a_view_with_area() {
        assert_eq!(resolve_pending(true, (0.0, 0.0)), (false, true));
        assert_eq!(resolve_pending(true, (800.0, 0.0)), (false, true));
        assert_eq!(resolve_pending(true, (800.0, 600.0)), (true, false));
        assert_eq!(resolve_pending(false, (800.0, 600.0)), (false, false));
        assert_eq!(resolve_pending(false, (0.0, 0.0)), (false, false));
    }

    #[test]
    fn levels_stop_at_1024_and_match_sizes() {
        let bytes = vec![255u8; 3000 * 2000 * 4];
        let out = run(3000, 2000, &bytes, &|| false);
        assert_eq!(out.iter().map(|l| l.0).collect::<Vec<_>>(), vec![1, 2]);
        let last = out.last().map(|l| (l.1, l.2)).unwrap_or_default();
        assert!(last.0.max(last.1) <= LEVEL_STOP, "{last:?}");
        assert!(out.iter().all(|l| l.3.len() == (l.1 * l.2 * 4) as usize));
    }

    #[test]
    fn stale_token_stops_the_build() {
        let bytes = vec![255u8; 3000 * 2000 * 4];
        assert!(run(3000, 2000, &bytes, &|| true).is_empty());
        // Fresh for the first check of level 1 and for its emit check, stale afterwards: only level 1 comes out.
        let calls = Cell::new(0);
        let out = run(3000, 2000, &bytes, &|| {
            calls.set(calls.get() + 1);
            calls.get() > 2
        });
        assert_eq!(out.iter().map(|l| l.0).collect::<Vec<_>>(), vec![1]);
    }

    #[test]
    fn transparent_edges_do_not_darken() {
        // Opaque white up to column 1022, transparent black from 1023: the half-covered block must stay white.
        let (w, h) = (2048u32, 4u32);
        let mut bytes = Vec::new();
        for _ in 0..h {
            for x in 0..w {
                bytes.extend_from_slice(if x < 1023 { &[255, 255, 255, 255] } else { &[0, 0, 0, 0] });
            }
        }
        let out = run(w, h, &bytes, &|| false);
        let level = &out[0];
        let px = &level.3[511 * 4..511 * 4 + 4];
        assert!(px[3] > 0 && px[3] < 255, "{px:?}");
        assert!(px[0] >= 250, "{px:?}");
    }
}
