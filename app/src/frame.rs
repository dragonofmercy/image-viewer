// Custom window frame: the system caption is removed and the Slint title bar takes its place.
// Caption buttons are drawn by Slint but hit-tested natively (HTMINBUTTON/HTMAXBUTTON/HTCLOSE), so Windows keeps
// snap layouts, the system menu and caption double-click behavior.

use std::cell::{Cell, RefCell};
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::ComponentHandle;
use viewer_core::settings::{Settings, WindowState};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute};
use windows::Win32::Graphics::Gdi::ScreenToClient;
use windows::Win32::System::Ole::RevokeDragDrop;
use windows::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
use windows::Win32::UI::Input::KeyboardAndMouse::{TME_LEAVE, TME_NONCLIENT, TRACKMOUSEEVENT, TrackMouseEvent};
use windows::Win32::UI::Shell::{DefSubclassProc, DragAcceptFiles, DragFinish, DragQueryFileW, HDROP, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::PCWSTR;

use crate::MainWindow;
use crate::geometry::Tracker;

const MIN_WIDTH: f32 = 768.0;
const MIN_HEIGHT: f32 = 400.0;

/// Title bar geometry in logical px, read once from `MainWindow` when the frame is installed.
#[derive(Clone, Copy, Default)]
struct Geometry {
    bar_height: f32,
    buttons_width: f32,
}

type DropHandler = Rc<dyn Fn(PathBuf)>;

thread_local! {
    static UI: RefCell<Option<slint::Weak<MainWindow>>> = const { RefCell::new(None) };
    static GEOMETRY: Cell<Geometry> = Cell::new(Geometry::default());
    // 0 = none, 1 = minimize, 2 = maximize, 3 = close
    static HOVER: Cell<i32> = const { Cell::new(0) };
    static PRESSED: Cell<i32> = const { Cell::new(0) };
    static OS_THEME_CHANGED: RefCell<Option<Rc<dyn Fn()>>> = const { RefCell::new(None) };
    static TRACKER: RefCell<Option<(Tracker, Settings)>> = const { RefCell::new(None) };
    static FILE_DROPPED: RefCell<Option<DropHandler>> = const { RefCell::new(None) };
}

pub fn hwnd_of(window: &slint::Window) -> Option<HWND> {
    match window.window_handle().window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(HWND(h.hwnd.get() as *mut _)),
        _ => None,
    }
}

/// Polls until the native window exists, installs the frame subclass once, then calls `on_ready`.
/// `RenderingSetup` cannot be relied on to fire once the window exists with every renderer, hence the 5 ms timer.
pub fn install_when_ready(ui: &MainWindow, on_ready: impl FnOnce(HWND) + 'static) {
    let weak = ui.as_weak();
    let mut on_ready = Some(on_ready);
    let timer = Rc::new(slint::Timer::default());
    let handle = timer.clone();
    timer.start(slint::TimerMode::Repeated, Duration::from_millis(5), move || {
        let Some(ui) = weak.upgrade() else { return };
        let Some(hwnd) = hwnd_of(ui.window()) else { return };
        handle.stop();
        install(&ui, hwnd);
        if let Some(on_ready) = on_ready.take() {
            on_ready(hwnd);
        }
    });
}

/// Switches the native title bar and system menu between light and dark (the Slint title bar is painted separately).
pub fn set_dark_title(hwnd: HWND, dark: bool) {
    let value = i32::from(dark);
    unsafe {
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, std::ptr::from_ref(&value).cast(), size_of::<i32>() as u32);
    }
}

/// Registers the callback run on the UI thread when the OS color mode or accent changes (`WM_SETTINGCHANGE` "ImmersiveColorSet").
pub fn on_os_theme_changed(f: impl Fn() + 'static) {
    OS_THEME_CHANGED.with(|c| *c.borrow_mut() = Some(Rc::new(f)));
}

/// Registers the callback run on the UI thread with the first file dropped on the window (`WM_DROPFILES`).
pub fn on_file_dropped(f: impl Fn(PathBuf) + 'static) {
    FILE_DROPPED.with(|c| *c.borrow_mut() = Some(Rc::new(f)));
}

/// Feeds `tracker` from WM_SIZE / WM_MOVE once the subclass is installed; state changes are written to `settings` right away, as 1.x.
pub fn track_geometry(tracker: Tracker, settings: Settings) {
    TRACKER.set(Some((tracker, settings)));
}

/// Enters or leaves borderless fullscreen (winit: drops WS_OVERLAPPEDWINDOW, then SetWindowPos to the monitor; leaving restores
/// the styles and the saved placement, maximized included). Winit does it synchronously, so WM_SIZE / WM_MOVE reach the subclass
/// inside `set_fullscreen`: the tracker stops recording before the window leaves its normal rectangle and resumes only once it is
/// back, so the monitor rectangle is never recorded. No TRACKER borrow is held across the call.
pub fn set_fullscreen(window: &slint::Window, fullscreen: bool) {
    let track = |on: bool| {
        TRACKER.with_borrow_mut(|t| {
            if let Some((tracker, _)) = t {
                tracker.set_fullscreen(on);
            }
        });
    };
    if fullscreen {
        track(true);
    }
    window.set_fullscreen(fullscreen);
    if !fullscreen {
        track(false);
    }
}

/// Winit's borderless fullscreen removes the caption style (our window always has it otherwise). Read from the window itself so
/// WM_NCCALCSIZE follows each style change winit makes, in both directions.
fn is_fullscreen(hwnd: HWND) -> bool {
    unsafe { GetWindowLongW(hwnd, GWL_STYLE) as u32 & WS_CAPTION.0 == 0 }
}

/// Writes the recorded position and size (call on close).
pub fn flush_geometry() {
    TRACKER.with_borrow(|t| {
        if let Some((tracker, settings)) = t {
            tracker.flush(settings);
        }
    });
}

fn install(ui: &MainWindow, hwnd: HWND) {
    UI.with(|u| *u.borrow_mut() = Some(ui.as_weak()));
    GEOMETRY.set(Geometry {
        bar_height: ui.get_title_bar_height(),
        buttons_width: ui.get_caption_buttons_width(),
    });
    unsafe {
        if !SetWindowSubclass(hwnd, Some(subclass_proc), 1, 0).as_bool() {
            eprintln!("frame: SetWindowSubclass failed, keeping the system caption");
            return;
        }
        // Re-run WM_NCCALCSIZE so the caption disappears right away.
        let _ = SetWindowPos(hwnd, None, 0, 0, 0, 0, SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
        // winit registers an OLE drop target, which would take precedence over WM_DROPFILES.
        let _ = RevokeDragDrop(hwnd);
        DragAcceptFiles(hwnd, true);
    }
}

fn scale(hwnd: HWND) -> f32 {
    unsafe { GetDpiForWindow(hwnd) as f32 / 96.0 }
}

/// Height of the resize border that the system adds above the client area (and that a maximized window pushes offscreen).
fn frame_thickness(hwnd: HWND) -> i32 {
    unsafe {
        let dpi = GetDpiForWindow(hwnd);
        GetSystemMetricsForDpi(SM_CYFRAME, dpi) + GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi)
    }
}

/// Width of the side resize borders, also used as the corner zone of the top resize strip.
fn frame_width(hwnd: HWND) -> i32 {
    unsafe {
        let dpi = GetDpiForWindow(hwnd);
        GetSystemMetricsForDpi(SM_CXFRAME, dpi) + GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi)
    }
}

fn button_of(hit: u32) -> i32 {
    match hit {
        HTMINBUTTON => 1,
        HTMAXBUTTON => 2,
        HTCLOSE => 3,
        _ => 0,
    }
}

/// Runs `f` on the UI from the event loop: the window procedure may run while Slint is mid-update.
fn with_ui(f: impl FnOnce(MainWindow) + Send + 'static) {
    UI.with(|u| {
        if let Some(weak) = u.borrow().as_ref() {
            let _ = weak.upgrade_in_event_loop(f);
        }
    });
}

fn push_caption_state() {
    let (hover, pressed) = (HOVER.get(), PRESSED.get());
    with_ui(move |ui| {
        ui.set_caption_hover(hover);
        ui.set_caption_pressed(pressed);
    });
}

fn hit_test(hwnd: HWND, lparam: LPARAM) -> u32 {
    let mut pt = POINT {
        x: (lparam.0 & 0xffff) as i16 as i32,
        y: ((lparam.0 >> 16) & 0xffff) as i16 as i32,
    };
    // No caption, no caption buttons, no resize borders in fullscreen.
    if is_fullscreen(hwnd) {
        return HTCLIENT;
    }
    let mut rc = RECT::default();
    unsafe {
        let _ = ScreenToClient(hwnd, &mut pt);
        let _ = GetClientRect(hwnd, &mut rc);
    }
    let s = scale(hwnd);
    let g = GEOMETRY.get();
    let zoomed = unsafe { IsZoomed(hwnd).as_bool() };
    // The top resize border lives inside the client area now that WM_NCCALCSIZE removed it.
    if !zoomed && pt.y < frame_thickness(hwnd) {
        let corner = frame_width(hwnd);
        return if pt.x < corner {
            HTTOPLEFT
        } else if pt.x >= rc.right - corner {
            HTTOPRIGHT
        } else {
            HTTOP
        };
    }
    let y = pt.y as f32;
    if y >= g.bar_height * s {
        return HTCLIENT;
    }
    let from_right = (rc.right - pt.x) as f32;
    let button = g.buttons_width / 3.0 * s;
    if from_right > 3.0 * button {
        HTCAPTION
    } else if from_right <= button {
        HTCLOSE
    } else if from_right <= 2.0 * button {
        HTMAXBUTTON
    } else {
        HTMINBUTTON
    }
}

unsafe extern "system" fn subclass_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM, _id: usize, _data: usize) -> LRESULT {
    unsafe {
        match msg {
            // Fullscreen: the whole window is client area (a window maximized before keeps IsZoomed, so no maximized inset either).
            WM_NCCALCSIZE if wparam.0 != 0 && is_fullscreen(hwnd) => LRESULT(0),
            WM_NCCALCSIZE if wparam.0 != 0 => {
                // Let the system compute the left/right/bottom resize borders, then give the caption back to the client area.
                let params = lparam.0 as *mut NCCALCSIZE_PARAMS;
                let top = (*params).rgrc[0].top;
                let result = DefSubclassProc(hwnd, msg, wparam, lparam);
                (*params).rgrc[0].top = top + if IsZoomed(hwnd).as_bool() { frame_thickness(hwnd) } else { 0 };
                result
            }
            WM_NCHITTEST => {
                let result = DefSubclassProc(hwnd, msg, wparam, lparam);
                if result.0 != HTCLIENT as isize {
                    return result;
                }
                LRESULT(hit_test(hwnd, lparam) as isize)
            }
            WM_GETMINMAXINFO => {
                let result = DefSubclassProc(hwnd, msg, wparam, lparam);
                let info = lparam.0 as *mut MINMAXINFO;
                let s = scale(hwnd);
                let min = (*info).ptMinTrackSize;
                (*info).ptMinTrackSize = POINT {
                    x: min.x.max((MIN_WIDTH * s).round() as i32),
                    y: min.y.max((MIN_HEIGHT * s).round() as i32),
                };
                result
            }
            WM_SETTINGCHANGE if lparam.0 != 0 && PCWSTR(lparam.0 as *const u16).to_string().is_ok_and(|s| s == "ImmersiveColorSet") => {
                // Deferred: do not touch Slint properties from inside the window procedure.
                let _ = slint::invoke_from_event_loop(|| {
                    if let Some(f) = OS_THEME_CHANGED.with(|c| c.borrow().clone()) {
                        f();
                    }
                });
                DefSubclassProc(hwnd, msg, wparam, lparam)
            }
            WM_ACTIVATE => {
                let active = (wparam.0 & 0xffff) as u32 != WA_INACTIVE;
                with_ui(move |ui| ui.set_window_active(active));
                DefSubclassProc(hwnd, msg, wparam, lparam)
            }
            WM_NCMOUSEMOVE => {
                let hover = button_of(wparam.0 as u32);
                if hover != HOVER.get() {
                    HOVER.set(hover);
                    push_caption_state();
                }
                let mut tme = TRACKMOUSEEVENT {
                    cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE | TME_NONCLIENT,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                let _ = TrackMouseEvent(&mut tme);
                DefSubclassProc(hwnd, msg, wparam, lparam)
            }
            WM_NCMOUSELEAVE => {
                HOVER.set(0);
                PRESSED.set(0);
                push_caption_state();
                DefSubclassProc(hwnd, msg, wparam, lparam)
            }
            // DefWindowProc would run its modal button loop and paint classic caption buttons over ours.
            WM_NCLBUTTONDOWN | WM_NCLBUTTONDBLCLK if button_of(wparam.0 as u32) != 0 => {
                PRESSED.set(button_of(wparam.0 as u32));
                push_caption_state();
                LRESULT(0)
            }
            WM_NCLBUTTONUP if button_of(wparam.0 as u32) != 0 => {
                let button = button_of(wparam.0 as u32);
                if button == PRESSED.get() {
                    let command = match button {
                        1 => SC_MINIMIZE,
                        2 if IsZoomed(hwnd).as_bool() => SC_RESTORE,
                        2 => SC_MAXIMIZE,
                        _ => SC_CLOSE,
                    };
                    let _ = PostMessageW(Some(hwnd), WM_SYSCOMMAND, WPARAM(command as usize), LPARAM(0));
                }
                PRESSED.set(0);
                HOVER.set(0);
                push_caption_state();
                LRESULT(0)
            }
            WM_SIZE => {
                let state = match wparam.0 as u32 {
                    SIZE_RESTORED => Some(WindowState::Normal),
                    SIZE_MINIMIZED => Some(WindowState::Minimized),
                    SIZE_MAXIMIZED => Some(WindowState::Maximized),
                    _ => None,
                };
                if let Some(state) = state {
                    // Client size in physical px, stored in DIPs like 1.x. Slint syncs `MainWindow.maximized` itself on resize.
                    let s = scale(hwnd);
                    let (w, h) = ((lparam.0 & 0xffff) as f32 / s, ((lparam.0 >> 16) & 0xffff) as f32 / s);
                    TRACKER.with_borrow_mut(|t| {
                        if let Some((tracker, settings)) = t {
                            if tracker.on_state(state)
                                && let Err(e) = settings.set_window_state(state)
                            {
                                eprintln!("frame: cannot save the window state: {e}");
                            }
                            tracker.on_resized(w.round() as u32, h.round() as u32);
                        }
                    });
                }
                DefSubclassProc(hwnd, msg, wparam, lparam)
            }
            WM_MOVE => {
                // WM_MOVE gives the client origin; 1.x stores the window rect origin.
                let mut rc = RECT::default();
                if GetWindowRect(hwnd, &mut rc).is_ok() {
                    TRACKER.with_borrow_mut(|t| {
                        if let Some((tracker, _)) = t {
                            let actual = if IsIconic(hwnd).as_bool() {
                                WindowState::Minimized
                            } else if IsZoomed(hwnd).as_bool() {
                                WindowState::Maximized
                            } else {
                                WindowState::Normal
                            };
                            tracker.on_moved(rc.left, rc.top, actual);
                        }
                    });
                }
                DefSubclassProc(hwnd, msg, wparam, lparam)
            }
            WM_DROPFILES => {
                let drop = HDROP(wparam.0 as *mut _);
                let len = DragQueryFileW(drop, 0, None) as usize;
                let mut buf = vec![0u16; len + 1];
                let copied = DragQueryFileW(drop, 0, Some(&mut buf)) as usize;
                DragFinish(drop);
                if copied > 0 {
                    let path = PathBuf::from(OsString::from_wide(&buf[..copied.min(len)]));
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(f) = FILE_DROPPED.with(|c| c.borrow().clone()) {
                            f(path);
                        }
                    });
                }
                LRESULT(0)
            }
            WM_NCDESTROY => {
                let _ = RemoveWindowSubclass(hwnd, Some(subclass_proc), 1);
                DefSubclassProc(hwnd, msg, wparam, lparam)
            }
            _ => DefSubclassProc(hwnd, msg, wparam, lparam),
        }
    }
}
