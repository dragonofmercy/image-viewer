//! Connects the `Document`, the `Loader`, the `ImageHost` and the `Commands` / `AppState` globals (the loading half of `Context` in 1.x).
//!
//! Rule: no `RefCell` borrow of the document is held across a call into Slint or `ImageHost`.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use std::time::{Duration, Instant};

use slint::{ComponentHandle, ModelRc, Rgba8Pixel, SharedPixelBuffer, TimerMode, VecModel};
use viewer_core::culture::Strings;
use viewer_core::files::{SUPPORTED_FILE_TYPES, is_supported};
use viewer_core::folder::FolderNav;
use viewer_core::picture::{LoadError, Picture};

use crate::document::{Document, Loader, Source, frame_delay, next_icon_index, reveal_offset};
use crate::file_info;
use crate::image_host::ImageHost;
use crate::win::{dialogs, shell};
use crate::{AppState, Commands, IconSizeItem, MainWindow, frame, ui_state, window_title};

thread_local! {
    // Loader results come back through the event loop, which only carries `Send` closures: they reach the controller through this.
    static CURRENT: RefCell<Weak<Controller>> = const { RefCell::new(Weak::new()) };
}

/// `can_open` of the folder navigation: 1.x skips files that no longer exist.
fn exists(path: &Path) -> bool {
    path.is_file()
}

pub struct Controller {
    ui: slint::Weak<MainWindow>,
    host: Rc<ImageHost>,
    doc: RefCell<Document>,
    loader: Loader,
    strings: Rc<Strings>,
    /// Frame timer of the playing animation. Held here so the controller dropping (or `stop_animation`) ends it.
    anim: slint::Timer,
    /// IMAGEVIEWER_TRACE set at startup: one timing line per load on stderr.
    trace: bool,
}

impl Controller {
    pub fn new(ui: &MainWindow, host: Rc<ImageHost>, strings: Rc<Strings>) -> Rc<Self> {
        let controller = Rc::new(Self {
            ui: ui.as_weak(),
            host,
            doc: RefCell::default(),
            loader: Loader::new(),
            strings,
            anim: slint::Timer::default(),
            trace: std::env::var_os("IMAGEVIEWER_TRACE").is_some(),
        });
        CURRENT.with(|c| *c.borrow_mut() = Rc::downgrade(&controller));
        let commands = ui.global::<Commands>();
        let c = controller.clone();
        // Deferred as 1.x yields before the picker: the dialog does not run inside the key or menu handler.
        commands.on_open(move || {
            let c = c.clone();
            slint::Timer::single_shot(Duration::ZERO, move || c.open_dialog());
        });
        let c = controller.clone();
        commands.on_next(move || c.next());
        let c = controller.clone();
        commands.on_previous(move || c.previous());
        let c = controller.clone();
        commands.on_first(move || c.first());
        let c = controller.clone();
        commands.on_last(move || c.last());
        let c = controller.clone();
        commands.on_file_info(move || c.open_info());
        let c = controller.clone();
        // Task 12 extends this with the rest of the Escape priority (crop, fullscreen).
        commands.on_escape(move || {
            c.close_info();
        });
        let c = controller.clone();
        ui.on_info_folder_clicked(move || c.open_info_folder());
        let c = controller.clone();
        ui.on_icon_reveal_offset(|current, index, count, pitch, viewport| reveal_offset(current, usize::try_from(index).unwrap_or(0), usize::try_from(count).unwrap_or(0), pitch, viewport));
        ui.on_icon_size_chosen(move |i| c.select_icon_size(i));
        let c = controller.clone();
        ui.on_icon_size_step(move |delta| c.step_icon_size(delta));
        let weak = Rc::downgrade(&controller);
        frame::on_file_dropped(move |path| {
            if let Some(c) = weak.upgrade() {
                c.open_path(&path);
            }
        });
        controller.refresh_ui();
        controller
    }

    /// `LoadDefaultImage`: the ring shows first, the load starts once the window had time to paint.
    pub fn open_startup(self: &Rc<Self>, path: PathBuf) {
        let generation = {
            let mut doc = self.doc.borrow_mut();
            doc.loading = true;
            doc.generation
        };
        self.refresh_ui();
        let c = self.clone();
        slint::Timer::single_shot(Duration::from_millis(50), move || {
            if c.open_path(&path) {
                return;
            }
            // A load started meanwhile (a drop) owns the loading state: it clears it when it finishes.
            let idle = {
                let mut doc = c.doc.borrow_mut();
                let idle = doc.generation == generation;
                if idle {
                    doc.loading = false;
                }
                idle
            };
            if idle {
                c.refresh_ui();
            }
        });
    }

    /// `LoadImageFromString(path, true)`: loads the file and lists its folder. False when the file does not exist.
    pub fn open_path(&self, path: &Path) -> bool {
        if !self.load(path) {
            return false;
        }
        self.doc.borrow_mut().list_folder();
        self.refresh_ui();
        true
    }

    /// `LoadImageFromPicker`: a file whose extension is not supported is ignored.
    pub fn open_dialog(&self) {
        let Some(hwnd) = self.ui.upgrade().and_then(|ui| frame::hwnd_of(ui.window())) else { return };
        if let Some(path) = dialogs::pick_open(hwnd, &SUPPORTED_FILE_TYPES)
            && is_supported(&path)
        {
            self.open_path(&path);
        }
    }

    pub fn next(&self) {
        self.navigate(|f| f.next(exists));
    }

    pub fn previous(&self) {
        self.navigate(|f| f.prev(exists));
    }

    pub fn first(&self) {
        self.navigate(|f| f.first(exists));
    }

    pub fn last(&self) {
        self.navigate(|f| f.last(exists));
    }

    fn navigate(&self, step: impl FnOnce(&mut FolderNav) -> Option<PathBuf>) {
        let target = self.doc.borrow_mut().folder.as_mut().and_then(step);
        if let Some(path) = target {
            self.load(&path);
        }
    }

    /// Starts a load of `path` without touching the folder list. False when the file does not exist.
    fn load(&self, path: &Path) -> bool {
        let started = self.doc.borrow_mut().begin_path(path);
        if started.is_some() {
            self.stop_animation();
        }
        let Some((generation, path)) = started else { return false };
        self.refresh_ui();
        let requested = self.trace.then(Instant::now);
        self.loader.request(Source::Path(path), generation, move |generation, result, decode| {
            if let Some(c) = CURRENT.with(|c| c.borrow().upgrade()) {
                c.finished(generation, result, decode, requested);
            }
        });
        true
    }

    /// `requested` is set only when tracing.
    fn finished(&self, generation: u64, result: Result<Picture, LoadError>, decode: Duration, requested: Option<Instant>) {
        if !self.doc.borrow_mut().finish_load(generation, result) {
            return;
        }
        // Taken out for the call so no borrow is held while the host pushes to Slint.
        let picture = self.doc.borrow_mut().picture.take();
        let show_start = requested.map(|_| Instant::now());
        match &picture {
            Some(p) if p.is_animated() => self.host.show_animated(p.current()),
            Some(p) => self.host.show(p.current()),
            None => self.host.clear(),
        }
        if let (Some(requested), Some(show_start)) = (requested, show_start) {
            let end = Instant::now();
            let file = self.doc.borrow().inputs().file_name.unwrap_or_default();
            let (w, h) = picture.as_ref().map_or((0, 0), |p| (p.width(), p.height()));
            let ms = |d: Duration| d.as_millis();
            eprintln!("trace load file={file} decode_ms={} show_ms={} total_ms={} size={w}x{h}", ms(decode), ms(end - show_start), ms(end - requested));
        }
        self.set_icon_sizes(picture.as_ref());
        let animated = picture.as_ref().is_some_and(Picture::is_animated);
        self.doc.borrow_mut().picture = picture;
        self.refresh_ui();
        self.update_info();
        if animated {
            self.schedule_frame(0);
        }
    }

    /// `ButtonFileInfo_Click` + `PaneOpening`: shows the pane filled from the current file.
    fn open_info(&self) {
        let Some(ui) = self.ui.upgrade() else { return };
        ui.global::<AppState>().set_info_open(true);
        self.update_info();
    }

    /// Closes the pane. Returns false when it was not open (the Escape handler uses it to fall through). Also called by paste.
    pub fn close_info(&self) -> bool {
        let Some(ui) = self.ui.upgrade() else { return false };
        let app = ui.global::<AppState>();
        let was_open = app.get_info_open();
        app.set_info_open(false);
        was_open
    }

    /// `Context.UpdateFileInfo`, only while the pane is open (opening refreshes it too).
    fn update_info(&self) {
        let Some(ui) = self.ui.upgrade() else { return };
        if !ui.global::<AppState>().get_info_open() {
            return;
        }
        let info = {
            let doc = self.doc.borrow();
            file_info::describe(doc.path.as_deref(), doc.picture.as_ref())
        };
        ui.set_info(info);
    }

    /// `TextBlockInfoFolder_Click`: the folder of the current file in Explorer.
    fn open_info_folder(&self) {
        let folder = self.doc.borrow().path.as_deref().and_then(Path::parent).map(Path::to_path_buf);
        if let Some(folder) = folder {
            shell::open_in_explorer(&folder);
        }
    }

    /// Stops the frame timer: new load, crop start, window close.
    pub fn stop_animation(&self) {
        self.anim.stop();
    }

    /// Waits the delay of frame `index`, then shows the frame after it (looping).
    fn schedule_frame(&self, index: usize) {
        let delay = self.doc.borrow().picture.as_ref().and_then(|p| p.frames().get(index)).map(|f| frame_delay(f.delay_ms));
        let Some(delay) = delay else { return };
        self.anim.start(TimerMode::SingleShot, delay, move || {
            if let Some(c) = CURRENT.with(|c| c.borrow().upgrade()) {
                c.show_frame(index + 1);
            }
        });
    }

    /// One animation step. Frames are converted when shown (one copy, no second cache of the whole animation).
    fn show_frame(&self, index: usize) {
        let buffer = {
            let doc = self.doc.borrow();
            let Some(p) = doc.picture.as_ref().filter(|p| p.is_animated()) else { return };
            let frames = p.frames();
            let index = if index < frames.len() { index } else { 0 };
            let pixels = &frames[index].pixels;
            (index, SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(pixels.as_raw(), pixels.width(), pixels.height()))
        };
        self.host.set_frame(buffer.1);
        self.schedule_frame(buffer.0);
    }

    /// `PopulateIconSizes`: thumbnails are the decoded frames themselves, built once per opened file.
    fn set_icon_sizes(&self, picture: Option<&Picture>) {
        let Some(ui) = self.ui.upgrade() else { return };
        let items: Vec<IconSizeItem> = picture
            .filter(|p| p.has_icon_sizes())
            .map(|p| {
                (0..p.icon_size_count())
                    .map(|i| {
                        let pixels = p.icon_frame(i);
                        let (w, h) = pixels.dimensions();
                        IconSizeItem {
                            thumb: slint::Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(pixels.as_raw(), w, h)),
                            label: w.to_string().into(),
                            tooltip: format!("{w} x {h}").into(),
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        ui.set_icon_sizes(ModelRc::new(VecModel::from(items)));
        ui.set_icon_index(picture.map_or(0, |p| p.icon_size_index() as i32));
    }

    /// `SelectIconSize`: the frames are already decoded, so only the view is rebuilt (fit, levels, dimensions).
    fn select_icon_size(&self, index: i32) {
        let Ok(index) = usize::try_from(index) else { return };
        let Some(mut picture) = self.doc.borrow_mut().picture.take() else { return };
        let changed = picture.has_icon_sizes() && index < picture.icon_size_count() && index != picture.icon_size_index();
        if changed {
            picture.select_icon_size(index);
            self.host.show(picture.current());
        }
        let shown = picture.icon_size_index();
        self.doc.borrow_mut().picture = Some(picture);
        if let Some(ui) = self.ui.upgrade() {
            ui.set_icon_index(shown as i32);
        }
        if changed {
            self.refresh_ui();
        }
    }

    /// `StepIconSize`: wraps around both ends.
    fn step_icon_size(&self, delta: i32) {
        let target = self.doc.borrow().picture.as_ref().filter(|p| p.has_icon_sizes()).map(|p| next_icon_index(p.icon_size_index(), delta, p.icon_size_count()));
        if let Some(i) = target {
            self.select_icon_size(i as i32);
        }
    }

    /// `UpdateButtonsAccessiblity` + `LoadingDisplay`: command flags, title, dimensions, ring and error panel.
    pub fn refresh_ui(&self) {
        let Some(ui) = self.ui.upgrade() else { return };
        let app = ui.global::<AppState>();
        let (inputs, dimensions, error) = {
            let doc = self.doc.borrow();
            let inputs = ui_state::Inputs {
                fullscreen: app.get_fullscreen(),
                ..doc.inputs()
            };
            let dimensions = doc.picture.as_ref().map(|p| format!("{}x{}", p.width(), p.height())).unwrap_or_default();
            (inputs, dimensions, doc.error.clone())
        };
        let flags = ui_state::flags(&inputs);
        app.set_image_commands(flags.image_commands);
        app.set_save_direct(flags.save_direct);
        app.set_file_info(flags.file_info);
        app.set_navigate(flags.navigate);
        app.set_icon_strip(flags.icon_strip);
        app.set_dimensions_text(dimensions.into());
        let prefix = ui_state::title_prefix(&inputs, &self.strings.get("SYSTEM_LOADING"), &self.strings.get("SYSTEM_PASTED_CONTENT"));
        ui.set_title_text(window_title(prefix.as_deref()).into());
        ui.set_loading(inputs.loading);
        ui.set_error_visible(error.is_some());
        ui.set_error_file(error.unwrap_or_default().into());
    }
}
