//! View controller of the image area: owns the `View` (zoom and offset), the display pyramid, and pushes the result to the Slint properties.
//!
//! Interface for the loading and editing tasks: `ImageHost::new(&ui)`, `show(&RgbaImage)` (displays at the fit zoom), `clear()`, `fit()`.
//! Zoom and pan callbacks (wheel, drag, resize, `Commands.adjust/zoom-full/zoom-in/zoom-out`) are registered by `new`.

use crate::viewport::{self, View};
use crate::{AppState, Commands, MainWindow};
use image::{RgbaImage, imageops};
use slint::{ComponentHandle, Image, Rgba8Pixel, SharedPixelBuffer, SharedString};
use std::cell::RefCell;
use std::rc::{Rc, Weak};

/// GPU texture side limit: a level above it is never uploaded.
const MAX_TEXTURE: u32 = 16384;
/// Levels stop once the longest edge is at or under this.
const LEVEL_STOP: u32 = 1024;

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

fn to_image(pixels: &RgbaImage) -> Image {
    // Rgba8Pixel buffers are non-premultiplied, as `RgbaImage`.
    Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(pixels.as_raw(), pixels.width(), pixels.height()))
}

#[derive(Default)]
struct State {
    image: (u32, u32),
    levels: Vec<Option<Image>>,
    shown: Option<usize>,
    view: Option<View>,
    /// Incremented by each `show`/`clear`: levels coming from an older one are dropped.
    token: u64,
}

pub struct ImageHost {
    ui: slint::Weak<MainWindow>,
    state: RefCell<State>,
}

impl ImageHost {
    pub fn new(ui: &MainWindow) -> Rc<Self> {
        let host = Rc::new(Self { ui: ui.as_weak(), state: RefCell::default() });
        CURRENT.with(|c| *c.borrow_mut() = Rc::downgrade(&host));
        let h = host.clone();
        ui.on_view_wheel(move |delta, x, y| h.update(|v, image, view| viewport::wheel(v, delta.round() as i32, (x, y), image, view)));
        let h = host.clone();
        ui.on_view_drag(move |dx, dy| h.update(|v, image, view| viewport::pan(v, dx, dy, image, view)));
        let h = host.clone();
        ui.on_view_resized(move || h.update(viewport::clamp));
        let commands = ui.global::<Commands>();
        let h = host.clone();
        commands.on_adjust(move || h.fit());
        let h = host.clone();
        commands.on_zoom_full(move || h.update(|_, image, view| viewport::centered(1.0, image, view)));
        let h = host.clone();
        commands.on_zoom_in(move || h.update(|v, image, view| viewport::step(v, 0.1, image, view)));
        let h = host.clone();
        commands.on_zoom_out(move || h.update(|v, image, view| viewport::step(v, -0.1, image, view)));
        host
    }

    /// Displays `pixels` at the fit zoom. Smaller levels are built on a worker thread and attached when ready.
    #[allow(dead_code)] // first caller: the document loading task
    pub fn show(&self, pixels: &RgbaImage) {
        let (w, h) = pixels.dimensions();
        if w == 0 || h == 0 {
            return self.clear();
        }
        let sizes = level_sizes(w, h);
        let first = first_displayable(&sizes);
        let mut levels = vec![None; sizes.len()];
        if first == 0 {
            levels[0] = Some(to_image(pixels));
        }
        let token = {
            let mut s = self.state.borrow_mut();
            s.token += 1;
            *s = State {
                image: (w, h),
                levels,
                shown: None,
                view: None,
                token: s.token,
            };
            s.token
        };
        self.fit();
        if sizes.len() > 1 {
            let base = pixels.clone();
            std::thread::spawn(move || build_levels(base, sizes, first, token));
        }
    }

    /// Removes the image and resets the zoom indicators.
    #[allow(dead_code)] // first caller: the document loading task
    pub fn clear(&self) {
        {
            let mut s = self.state.borrow_mut();
            *s = State { token: s.token + 1, ..State::default() };
        }
        if let Some(ui) = self.ui.upgrade() {
            ui.set_view_source(Image::default());
            ui.set_view_w(0.0);
            ui.set_view_h(0.0);
            let app = ui.global::<AppState>();
            app.set_zoom_text("100%".into());
            app.set_show_adjust(false);
        }
    }

    /// Zoom to fit the view at offset 0.
    pub fn fit(&self) {
        self.update(|_, image, view| View {
            zoom: viewport::fit_zoom(image, view),
            offset_x: 0.0,
            offset_y: 0.0,
        });
    }

    fn attach(&self, token: u64, index: usize, buffer: SharedPixelBuffer<Rgba8Pixel>) {
        {
            let mut s = self.state.borrow_mut();
            if s.token != token || index >= s.levels.len() {
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

/// Worker thread: halves the image down the pyramid and hands each level that fits a texture to the UI thread.
fn build_levels(base: RgbaImage, sizes: Vec<(u32, u32)>, first: usize, token: u64) {
    let mut current: Option<RgbaImage> = None;
    for (index, &(w, h)) in sizes.iter().enumerate().skip(1) {
        let next = imageops::thumbnail(current.as_ref().unwrap_or(&base), w, h);
        if index >= first {
            let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(next.as_raw(), next.width(), next.height());
            let sent = slint::invoke_from_event_loop(move || {
                if let Some(host) = CURRENT.with(|c| c.borrow().upgrade()) {
                    host.attach(token, index, buffer);
                }
            });
            if sent.is_err() {
                return;
            }
        }
        current = Some(next);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
