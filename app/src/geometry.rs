// Window geometry persistence, as 1.x (App.xaml.cs): tracked in memory, written once on close, restored against the monitor layout.
// Units, as 1.x stores them: PosX/PosY are the physical top-left of the window rect (AppWindow.Position), Width/Height are
// the client size in DIPs (Window.SizeChanged args.Size).

use viewer_core::settings::{Settings, WindowState};
use windows::Win32::Foundation::{LPARAM, POINT, RECT};
use windows::Win32::Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTOPRIMARY, MONITORINFO, MonitorFromPoint, MonitorFromRect};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, GetSystemMetricsForDpi, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::{SM_CXFRAME, SM_CXPADDEDBORDER, SM_CYCAPTION, SM_CYFRAME};
use windows::core::BOOL;

/// Below this, Windows has parked a minimized window offscreen.
const PARKED: i32 = -32000;

/// Physical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Saved {
    pub pos: Option<(i32, i32)>,
    pub size: (u32, u32),
    pub state: WindowState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    At { x: i32, y: i32, w: u32, h: u32 },
    Centered { w: u32, h: u32 },
}

pub fn load(settings: &Settings) -> Saved {
    Saved {
        pos: settings.pos_x().zip(settings.pos_y()),
        size: (settings.width(), settings.height()),
        state: settings.window_state(),
    }
}

/// `start + len`, saturating: the registry can hold absurd sizes.
fn end(start: i32, len: u32) -> i32 {
    start.saturating_add(i32::try_from(len).unwrap_or(i32::MAX))
}

/// The saved position is kept only when its rectangle still intersects a monitor (1.x: `DisplayArea.GetFromRect` with no fallback,
/// fed with the stored size as is).
pub fn placement(saved: &Saved, monitors: &[Rect]) -> Placement {
    let (w, h) = saved.size;
    match saved.pos {
        Some((x, y)) if x > PARKED && y > PARKED && monitors.iter().any(|m| x < m.right && end(x, w) > m.left && y < m.bottom && end(y, h) > m.top) => Placement::At { x, y, w, h },
        _ => Placement::Centered { w, h },
    }
}

/// In-memory geometry: drags and resizes fire on every tick, so only the last values are written, on close.
pub struct Tracker {
    state: WindowState,
    fullscreen: bool,
    pos: Option<(i32, i32)>,
    size: Option<(u32, u32)>,
}

impl Tracker {
    pub fn new(state: WindowState) -> Self {
        Self { state, fullscreen: false, pos: None, size: None }
    }

    /// True when the state changed: 1.x writes it to the registry right away.
    pub fn on_state(&mut self, state: WindowState) -> bool {
        std::mem::replace(&mut self.state, state) != state
    }

    fn recording(&self) -> bool {
        self.state == WindowState::Normal && !self.fullscreen
    }

    /// `actual` is the window's real state at the time of the move (IsZoomed / IsIconic): Windows sends WM_MOVE before
    /// WM_SIZE, so the tracked state still says Normal while the window is already maximized or minimized, and still says
    /// Maximized while it is already restored.
    pub fn on_moved(&mut self, x: i32, y: i32, actual: WindowState) {
        if actual == WindowState::Normal && !self.fullscreen && x > PARKED && y > PARKED {
            self.pos = Some((x, y));
        }
    }

    pub fn on_resized(&mut self, w: u32, h: u32) {
        if self.recording() {
            self.size = Some((w, h));
        }
    }

    // shortcut: no caller until the fullscreen task wires it, drop the allow then.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn set_fullscreen(&mut self, fullscreen: bool) {
        self.fullscreen = fullscreen;
    }

    pub fn flush(&self, settings: &Settings) {
        let mut results = Vec::new();
        if let Some((x, y)) = self.pos {
            results.push(settings.set_pos_x(Some(x)));
            results.push(settings.set_pos_y(Some(y)));
        }
        if let Some((w, h)) = self.size {
            results.push(settings.set_width(w));
            results.push(settings.set_height(h));
        }
        for e in results.into_iter().filter_map(Result::err) {
            eprintln!("geometry: cannot save the window geometry: {e}");
        }
    }
}

/// Monitor rectangles in physical pixels.
pub fn monitors() -> Vec<Rect> {
    unsafe extern "system" fn push(_: HMONITOR, _: HDC, rc: *mut RECT, data: LPARAM) -> BOOL {
        unsafe {
            let rc = &*rc;
            (*(data.0 as *mut Vec<Rect>)).push(Rect {
                left: rc.left,
                top: rc.top,
                right: rc.right,
                bottom: rc.bottom,
            });
        }
        true.into()
    }
    let mut out: Vec<Rect> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(push), LPARAM(std::ptr::from_mut(&mut out) as isize));
    }
    out
}

/// Sets the initial position, size and state on the Slint window before it is shown, so it opens in place without a jump.
pub fn apply(window: &slint::Window, saved: &Saved) {
    let (w, h) = saved.size;
    let place = placement(saved, &monitors());
    let monitor = unsafe {
        match place {
            Placement::At { x, y, .. } => MonitorFromRect(
                &RECT {
                    left: x,
                    top: y,
                    right: end(x, w),
                    bottom: end(y, h),
                },
                MONITOR_DEFAULTTONEAREST,
            ),
            Placement::Centered { .. } => MonitorFromPoint(POINT::default(), MONITOR_DEFAULTTOPRIMARY),
        }
    };
    let (mut dpi, mut dpi_y) = (96, 96);
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    let (side, bottom, caption) = unsafe {
        let _ = GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi, &mut dpi_y);
        let _ = GetMonitorInfoW(monitor, &mut info);
        let padded = GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi);
        (GetSystemMetricsForDpi(SM_CXFRAME, dpi) + padded, GetSystemMetricsForDpi(SM_CYFRAME, dpi) + padded, GetSystemMetricsForDpi(SM_CYCAPTION, dpi))
    };
    let s = dpi as f32 / 96.0;
    let (pw, ph) = ((w as f32 * s).round() as i32, (h as f32 * s).round() as i32);
    // The frame subclass turns the caption and the top border into client area: ask for that much less so the final client is w x h DIPs.
    window.set_size(slint::PhysicalSize::new(pw.max(1) as u32, (ph - bottom - caption).max(1) as u32));
    let (x, y) = match place {
        Placement::At { x, y, .. } => (x, y),
        // Final window rect: client plus the side borders and the bottom border.
        Placement::Centered { .. } => {
            let work = info.rcWork;
            (
                work.left + (work.right - work.left).saturating_sub(pw.saturating_add(2 * side)) / 2,
                work.top + (work.bottom - work.top).saturating_sub(ph.saturating_add(bottom)) / 2,
            )
        }
    };
    window.set_position(slint::PhysicalPosition::new(x, y));
    if saved.state == WindowState::Maximized {
        window.set_maximized(true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use viewer_core::settings::{APP_KEY, Settings, WindowState};
    use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RegDeleteKeyW, RegDeleteTreeW};
    use windows::core::HSTRING;

    const TESTS_PARENT: &str = r"SOFTWARE\Dragon Industries\Image Viewer Tests";

    /// Unique key under the shared tests parent (same pattern as viewer-core/tests/settings.rs), deleted on drop.
    struct Sandbox {
        path: String,
    }

    impl Sandbox {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
            let path = format!(r"{TESTS_PARENT}\app-geometry-{}-{}-{}", std::process::id(), nanos, NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
            assert_ne!(path, APP_KEY);
            Self { path }
        }

        fn settings(&self) -> Settings {
            Settings::at(&self.path)
        }
    }

    impl Drop for Sandbox {
        fn drop(&mut self) {
            unsafe {
                let _ = RegDeleteTreeW(HKEY_CURRENT_USER, &HSTRING::from(&self.path));
                // Fails while a parallel test still has a child, which is fine.
                let _ = RegDeleteKeyW(HKEY_CURRENT_USER, &HSTRING::from(TESTS_PARENT));
            }
        }
    }

    const MONITORS: [Rect; 2] = [Rect { left: 0, top: 0, right: 1920, bottom: 1080 }, Rect { left: 1920, top: 0, right: 3840, bottom: 1080 }];

    fn saved(pos: Option<(i32, i32)>) -> Saved {
        Saved {
            pos,
            size: (1280, 768),
            state: WindowState::Normal,
        }
    }

    #[test]
    fn placement_restores_a_position_on_a_present_monitor() {
        assert_eq!(placement(&saved(Some((2000, 100))), &MONITORS), Placement::At { x: 2000, y: 100, w: 1280, h: 768 });
        // Partly offscreen still intersects.
        assert_eq!(placement(&saved(Some((-500, -10))), &MONITORS), Placement::At { x: -500, y: -10, w: 1280, h: 768 });
    }

    #[test]
    fn placement_centers_when_the_monitor_is_gone() {
        assert_eq!(placement(&saved(Some((4000, 100))), &MONITORS), Placement::Centered { w: 1280, h: 768 });
        assert_eq!(placement(&saved(Some((-1280, 0))), &MONITORS), Placement::Centered { w: 1280, h: 768 });
        // The minimized parking position is never restored, even if a monitor reached that far.
        let far = [Rect { left: -40000, top: -40000, right: 0, bottom: 0 }];
        assert_eq!(placement(&saved(Some((-32000, -32000))), &far), Placement::Centered { w: 1280, h: 768 });
    }

    #[test]
    fn placement_survives_an_absurd_saved_size() {
        let huge = Saved {
            pos: Some((i32::MAX - 10, 100)),
            size: (u32::MAX, u32::MAX),
            state: WindowState::Normal,
        };
        assert_eq!(placement(&huge, &MONITORS), Placement::Centered { w: u32::MAX, h: u32::MAX });
        let huge = Saved {
            pos: Some((100, 100)),
            size: (u32::MAX, u32::MAX),
            state: WindowState::Normal,
        };
        assert_eq!(placement(&huge, &MONITORS), Placement::At { x: 100, y: 100, w: u32::MAX, h: u32::MAX });
    }

    #[test]
    fn placement_centers_without_a_saved_position() {
        assert_eq!(placement(&saved(None), &MONITORS), Placement::Centered { w: 1280, h: 768 });
    }

    #[test]
    fn tracker_ignores_parking_position() {
        let mut t = Tracker::new(WindowState::Normal);
        t.on_moved(-32000, 10, WindowState::Normal);
        t.on_moved(10, -32000, WindowState::Normal);
        assert_eq!(t.pos, None);
        t.on_moved(-31999, 5, WindowState::Normal);
        assert_eq!(t.pos, Some((-31999, 5)));
    }

    #[test]
    fn tracker_ignores_moves_and_resizes_unless_normal() {
        let mut t = Tracker::new(WindowState::Maximized);
        t.on_moved(-8, -8, WindowState::Maximized);
        t.on_resized(800, 600);
        assert_eq!((t.pos, t.size), (None, None));
        assert!(t.on_state(WindowState::Minimized));
        t.on_moved(10, 20, WindowState::Minimized);
        assert_eq!(t.pos, None);
        assert!(t.on_state(WindowState::Normal));
        t.on_moved(10, 20, WindowState::Normal);
        t.on_resized(800, 600);
        assert_eq!((t.pos, t.size), (Some((10, 20)), Some((800, 600))));
    }

    #[test]
    fn move_sent_before_the_state_change_follows_the_actual_state() {
        // Maximize: WM_MOVE (-8, -8) arrives while the tracked state is still Normal.
        let mut t = Tracker::new(WindowState::Normal);
        t.on_moved(100, 50, WindowState::Normal);
        t.on_moved(-8, -8, WindowState::Maximized);
        assert!(t.on_state(WindowState::Maximized));
        assert_eq!(t.pos, Some((100, 50)));
        // Minimize through a path that skips SC_MINIMIZE: the move lands before WM_SIZE(SIZE_MINIMIZED).
        t.on_state(WindowState::Normal);
        t.on_moved(-31000, -31000, WindowState::Minimized);
        assert_eq!(t.pos, Some((100, 50)));
        // Restore (drag out of maximized): WM_MOVE arrives while the tracked state is still Maximized.
        t.on_state(WindowState::Maximized);
        t.on_moved(300, 40, WindowState::Normal);
        assert!(t.on_state(WindowState::Normal));
        assert_eq!(t.pos, Some((300, 40)));
    }

    #[test]
    fn tracker_ignores_everything_while_fullscreen() {
        let mut t = Tracker::new(WindowState::Normal);
        t.set_fullscreen(true);
        t.on_moved(10, 20, WindowState::Normal);
        t.on_resized(800, 600);
        assert_eq!((t.pos, t.size), (None, None));
        t.set_fullscreen(false);
        t.on_resized(800, 600);
        assert_eq!(t.size, Some((800, 600)));
    }

    #[test]
    fn on_state_reports_only_changes() {
        let mut t = Tracker::new(WindowState::Normal);
        assert!(!t.on_state(WindowState::Normal));
        assert!(t.on_state(WindowState::Maximized));
        assert!(!t.on_state(WindowState::Maximized));
        assert!(t.on_state(WindowState::Normal));
    }

    #[test]
    fn load_reads_the_settings() {
        let sb = Sandbox::new();
        let s = sb.settings();
        assert_eq!(
            load(&s),
            Saved {
                pos: None,
                size: (1280, 768),
                state: WindowState::Normal
            }
        );
        s.set_pos_x(Some(-7)).unwrap();
        s.set_pos_y(Some(30)).unwrap();
        s.set_width(900).unwrap();
        s.set_height(500).unwrap();
        s.set_window_state(WindowState::Maximized).unwrap();
        assert_eq!(
            load(&s),
            Saved {
                pos: Some((-7, 30)),
                size: (900, 500),
                state: WindowState::Maximized
            }
        );
        // Half a position is no position.
        s.set_pos_y(None).unwrap();
        assert_eq!(load(&s).pos, None);
    }

    #[test]
    fn flush_writes_only_recorded_fields() {
        let sb = Sandbox::new();
        let s = sb.settings();
        s.set_pos_x(Some(1)).unwrap();
        s.set_pos_y(Some(2)).unwrap();
        s.set_width(900).unwrap();
        s.set_height(500).unwrap();

        let mut t = Tracker::new(WindowState::Normal);
        t.on_moved(300, 200, WindowState::Normal);
        t.flush(&s);
        assert_eq!((s.pos_x(), s.pos_y(), s.width(), s.height()), (Some(300), Some(200), 900, 500));

        let mut t = Tracker::new(WindowState::Normal);
        t.on_resized(1000, 700);
        t.flush(&s);
        assert_eq!((s.pos_x(), s.pos_y(), s.width(), s.height()), (Some(300), Some(200), 1000, 700));
    }
}
