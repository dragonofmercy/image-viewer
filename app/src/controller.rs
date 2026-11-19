//! Connects the `Document`, the `Loader`, the `ImageHost` and the `Commands` / `AppState` globals (the loading half of `Context` in 1.x).
//!
//! Rule: no `RefCell` borrow of the document is held across a call into Slint or `ImageHost`.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use std::time::{Duration, Instant, SystemTime};

use slint::platform::WindowEvent;
use slint::{ComponentHandle, ModelRc, SharedString, TimerMode, VecModel};
use viewer_core::culture::Strings;
use viewer_core::dib;
use viewer_core::files::{SAVE_FILE_TYPES, SUPPORTED_FILE_TYPES, is_supported};
use viewer_core::folder::FolderNav;
use viewer_core::picture::{Flip, LoadError, Picture, Rotate, SaveError};
use viewer_core::settings::Settings;

use crate::crop::{self, Handle, Sel};
use crate::document::{CropSession, Document, Loader, PictureCache, Route, Saver, Source, Stamp, frame_delay, next_icon_index, quality_for, reveal_offset, save_route, stamp};
use crate::file_info;
use crate::image_host::{ImageHost, buffer_of};
use crate::win::{clipboard, dialogs, locale, print, shell};
use crate::{AppState, Commands, IconSizeItem, MainWindow, frame, ui_state, window_title};
use windows::Win32::Foundation::HWND;

thread_local! {
    // Loader results come back through the event loop, which only carries `Send` closures: they reach the controller through this.
    static CURRENT: RefCell<Weak<Controller>> = const { RefCell::new(Weak::new()) };
}

/// `can_open` of the folder navigation: 1.x skips files that no longer exist.
fn exists(path: &Path) -> bool {
    path.is_file()
}

/// The file to put on the clipboard next to the image: only an animated picture, unchanged, loaded from a file that is still there.
fn clipboard_file(animated: bool, modified: bool, path: Option<&Path>, exists: bool) -> Option<&Path> {
    path.filter(|_| animated && !modified && exists)
}

/// The controller, from a worker callback.
fn current() -> Option<Rc<Controller>> {
    CURRENT.with(|c| c.borrow().upgrade())
}

/// A command handler that calls `f` on the controller.
fn bind(c: &Rc<Controller>, f: fn(&Controller)) -> impl Fn() + 'static {
    let c = c.clone();
    move || f(&c)
}

/// Like `bind`, but `f` runs from the event loop, after the handler returned.
fn deferred(c: &Rc<Controller>, f: fn(&Controller)) -> impl Fn() + 'static {
    let c = c.clone();
    move || {
        let c = c.clone();
        slint::Timer::single_shot(Duration::ZERO, move || f(&c));
    }
}

pub struct Controller {
    ui: slint::Weak<MainWindow>,
    host: Rc<ImageHost>,
    doc: RefCell<Document>,
    loader: Loader,
    /// Decodes the neighbor of the shown file ahead of time, on its own thread so it never delays a real load.
    preloader: Loader,
    /// The preload last requested and not yet delivered.
    preloading: RefCell<Option<(PathBuf, Stamp)>>,
    cache: RefCell<PictureCache>,
    /// Stamp of the shown file when its decode started; None when the picture no longer matches a file as decoded (after a save).
    stamp: Cell<Option<Stamp>>,
    /// Direction of the last navigation: the preload goes the same way.
    backward: Cell<bool>,
    saver: Saver,
    /// The window was asked to close during a save: it closes once the save succeeded.
    close_pending: Cell<bool>,
    /// A Save As dialog or the shell's recycle UI is up (`SavingProcess` in 1.x): a second save or delete queued meanwhile is dropped.
    modal: Cell<bool>,
    strings: Rc<RefCell<Strings>>,
    /// Frame timer of the playing animation. Held here so the controller dropping (or `stop_animation`) ends it.
    anim: slint::Timer,
    /// IMAGEVIEWER_TRACE set at startup: one timing line per load on stderr.
    trace: bool,
}

impl Controller {
    pub fn new(ui: &MainWindow, host: Rc<ImageHost>, strings: Rc<RefCell<Strings>>) -> Rc<Self> {
        let controller = Rc::new(Self {
            ui: ui.as_weak(),
            host,
            doc: RefCell::default(),
            loader: Loader::new(),
            preloader: Loader::new(),
            preloading: RefCell::new(None),
            cache: RefCell::default(),
            stamp: Cell::new(None),
            backward: Cell::new(false),
            saver: Saver::new(),
            close_pending: Cell::new(false),
            modal: Cell::new(false),
            strings,
            anim: slint::Timer::default(),
            trace: std::env::var_os("IMAGEVIEWER_TRACE").is_some(),
        });
        CURRENT.with(|c| *c.borrow_mut() = Rc::downgrade(&controller));
        let weak = Rc::downgrade(&controller);
        controller.host.set_full_source(move || {
            let c = weak.upgrade()?;
            // Not available while a caller holds the document or took the picture out: the host then shows a smaller level.
            let doc = c.doc.try_borrow().ok()?;
            Some(buffer_of(doc.picture.as_ref()?.current()))
        });
        let commands = ui.global::<Commands>();
        let c = &controller;
        // Deferred as 1.x yields before the picker: the open and save dialogs do not run inside the key or menu handler.
        commands.on_open(deferred(c, Self::open_dialog));
        commands.on_save(deferred(c, Self::save));
        commands.on_save_as(deferred(c, Self::save_as));
        commands.on_next(bind(c, Self::next));
        commands.on_previous(bind(c, Self::previous));
        commands.on_first(bind(c, Self::first));
        commands.on_last(bind(c, Self::last));
        commands.on_file_info(bind(c, Self::open_info));
        // `MainWindow.xaml.cs` button handlers: the flip buttons pass the opposite ImageSharp mode, kept as is.
        commands.on_rotate_left(bind(c, |c| c.rotate_flip(Rotate::Rotate270, Flip::None)));
        commands.on_rotate_right(bind(c, |c| c.rotate_flip(Rotate::Rotate90, Flip::None)));
        commands.on_flip_horizontal(bind(c, |c| c.rotate_flip(Rotate::None, Flip::Vertical)));
        commands.on_flip_vertical(bind(c, |c| c.rotate_flip(Rotate::None, Flip::Horizontal)));
        commands.on_delete(bind(c, Self::delete));
        commands.on_print(deferred(c, Self::print));
        commands.on_copy(bind(c, Self::copy));
        commands.on_paste(bind(c, Self::paste));
        commands.on_escape(bind(c, Self::escape));
        commands.on_toggle_fullscreen(bind(c, Self::toggle_fullscreen));
        commands.on_crop(bind(c, Self::start_crop));
        ui.on_dialog_closed(deferred(c, Self::dialog_closed));
        ui.on_crop_reset(bind(c, Self::crop_reset));
        ui.on_crop_apply(bind(c, Self::crop_apply));
        ui.on_crop_cancel(bind(c, Self::crop_cancel));
        let c = controller.clone();
        ui.on_crop_drag_start(move |handle| c.crop_drag_start(handle));
        let c = controller.clone();
        ui.on_crop_drag_move(move |dx, dy| c.crop_drag_move(dx, dy));
        let c = controller.clone();
        ui.on_crop_ratio_chosen(move |index| c.crop_ratio_chosen(index));
        controller.push_crop_ratios();
        let c = controller.clone();
        ui.on_info_folder_clicked(move || c.open_info_folder());
        let c = controller.clone();
        ui.on_icon_reveal_offset(|current, index, count, pitch, viewport| reveal_offset(current, usize::try_from(index).unwrap_or(0), usize::try_from(count).unwrap_or(0), pitch, viewport));
        ui.on_icon_size_chosen(move |i| c.select_icon_size(i));
        let c = controller.clone();
        ui.on_icon_size_step(move |delta| c.step_icon_size(delta));
        let weak = Rc::downgrade(&controller);
        frame::on_fullscreen_changed(move |on| {
            if let Some(c) = weak.upgrade() {
                c.show_fullscreen(on);
            }
        });
        let weak = Rc::downgrade(&controller);
        frame::on_file_dropped(move |path| {
            if let Some(c) = weak.upgrade()
                && !c.dialog_open()
                && !c.modal.get()
            {
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
        if self.dialog_open() || self.doc.borrow().saving {
            return;
        }
        let Some(hwnd) = self.hwnd() else { return };
        if let Some(path) = self.exclusive(|| dialogs::pick_open(hwnd, &SUPPORTED_FILE_TYPES)).flatten()
            && is_supported(&path)
        {
            self.open_path(&path);
        }
    }

    fn hwnd(&self) -> Option<HWND> {
        self.ui.upgrade().and_then(|ui| frame::hwnd_of(ui.window()))
    }

    /// Runs `f`, a native dialog that pumps messages (`SavingProcess` in 1.x). None, without running it, when another one is up.
    fn exclusive<R>(&self, f: impl FnOnce() -> R) -> Option<R> {
        if self.modal.replace(true) {
            return None;
        }
        let result = f();
        self.modal.set(false);
        Some(result)
    }

    /// A dialog (settings, about, error) is up: commands that mutate or open things are dropped, whatever delivered them.
    fn dialog_open(&self) -> bool {
        self.ui.upgrade().is_some_and(|ui| ui.get_dialog_open())
    }

    pub fn next(&self) {
        self.backward.set(false);
        self.navigate(|f| f.next(exists));
    }

    pub fn previous(&self) {
        self.backward.set(true);
        self.navigate(|f| f.prev(exists));
    }

    pub fn first(&self) {
        self.backward.set(false);
        self.navigate(|f| f.first(exists));
    }

    pub fn last(&self) {
        self.backward.set(true);
        self.navigate(|f| f.last(exists));
    }

    fn navigate(&self, step: impl FnOnce(&mut FolderNav) -> Option<PathBuf>) {
        if self.dialog_open() {
            return;
        }
        let target = self.doc.borrow_mut().folder.as_mut().and_then(step);
        if let Some(path) = target {
            self.load(&path);
        }
    }

    /// Starts a load of `path` without touching the folder list. False when the file does not exist.
    /// The picture being left is kept for a way back, and a picture already decoded (kept, or preloaded) is shown at once.
    fn load(&self, path: &Path) -> bool {
        // A load would replace the picture the running save hands back.
        if self.doc.borrow().saving {
            return false;
        }
        // Only a picture that still is its file as decoded: an edit is not kept.
        let left = {
            let mut doc = self.doc.borrow_mut();
            let keep = !doc.loading && doc.picture.as_ref().is_some_and(|p| !p.modified());
            match (doc.path.clone(), self.stamp.get()) {
                (Some(path), Some(stamp)) if keep => doc.picture.take().map(|picture| (path, stamp, picture)),
                _ => None,
            }
        };
        let Some((generation, path)) = self.doc.borrow_mut().begin_path(path) else {
            if let Some((_, _, picture)) = left {
                self.doc.borrow_mut().picture = Some(picture);
            }
            return false;
        };
        if let Some((path, stamp, picture)) = left {
            self.cache.borrow_mut().put(path, stamp, picture);
        }
        self.stop_animation();
        let requested = self.trace.then(Instant::now);
        let on_disk = stamp(&path);
        let cached = self.cache.borrow_mut().take(&path, on_disk);
        if let Some((picture, stamp)) = cached {
            self.stamp.set(Some(stamp));
            self.finished(generation, Ok(picture), Duration::ZERO, requested);
            return true;
        }
        self.refresh_ui();
        // Already decoding as a preload: `preloaded` delivers it to this load.
        let preloading = self.preloading.borrow().as_ref().filter(|p| p.0 == path).map(|p| p.1);
        if let Some(stamp) = preloading {
            self.stamp.set(Some(stamp));
            return true;
        }
        self.stamp.set(on_disk);
        self.loader.request(Source::Path(path), generation, move |generation, result, decode| {
            if let Some(c) = current() {
                c.finished(generation, result, decode, requested);
            }
        });
        true
    }

    /// Decodes the next file in the direction of the last navigation, so that showing it takes no decode.
    fn preload_neighbor(&self) {
        let target = {
            let doc = self.doc.borrow();
            let Some(current) = doc.path.as_ref() else { return };
            // Stepped on a copy: the real position does not move.
            let Some(mut peek) = doc.folder.clone() else { return };
            let target = if self.backward.get() { peek.prev(exists) } else { peek.next(exists) };
            target.filter(|t| t != current)
        };
        let Some(target) = target else { return };
        if self.cache.borrow().contains(&target) || self.preloading.borrow().as_ref().is_some_and(|p| p.0 == target) {
            return;
        }
        let Some(stamp) = stamp(&target) else { return };
        *self.preloading.borrow_mut() = Some((target.clone(), stamp));
        let path = target.clone();
        self.preloader.request(Source::Path(target), 0, move |_, result, _| {
            if let Some(c) = current() {
                c.preloaded(path, stamp, result);
            }
        });
    }

    /// A preload finished: it completes the load waiting for it, or is kept for later.
    fn preloaded(&self, path: PathBuf, stamp: Stamp, result: Result<Picture, LoadError>) {
        {
            let mut preloading = self.preloading.borrow_mut();
            if preloading.as_ref().is_some_and(|p| p.0 == path) {
                *preloading = None;
            }
        }
        let (current, waiting) = {
            let doc = self.doc.borrow();
            let current = doc.path.as_deref() == Some(path.as_path());
            (current, (current && doc.loading).then_some(doc.generation))
        };
        match (waiting, result) {
            (Some(generation), result) => {
                self.stamp.set(Some(stamp));
                self.finished(generation, result, Duration::ZERO, None);
            }
            (None, Ok(picture)) if !current => self.cache.borrow_mut().put(path, stamp, picture),
            _ => {}
        }
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
        self.preload_neighbor();
    }

    /// `ButtonFileInfo_Click` + `PaneOpening`: shows the pane filled from the current file.
    fn open_info(&self) {
        if self.dialog_open() {
            return;
        }
        let Some(ui) = self.ui.upgrade() else { return };
        ui.global::<AppState>().set_info_open(true);
        self.update_info();
    }

    /// Closes the pane (Escape, paste, delete of the last file).
    fn close_info(&self) {
        if let Some(ui) = self.ui.upgrade() {
            ui.global::<AppState>().set_info_open(false);
        }
    }

    /// `Context.EscapeAction`: closes the front-most open layer, never quits.
    fn escape(&self) {
        let Some(ui) = self.ui.upgrade() else { return };
        let app = ui.global::<AppState>();
        let cropping = self.doc.borrow().crop.is_some();
        match ui_state::escape_target(cropping, app.get_info_open(), app.get_fullscreen()) {
            ui_state::Target::Cropper => self.crop_cancel(),
            ui_state::Target::InfoPane => self.close_info(),
            ui_state::Target::Fullscreen => self.set_fullscreen(false),
            ui_state::Target::Nothing => {}
        }
    }

    /// `ToggleFullScreen` (F11), from the window's own state so a drift of the UI state cannot swallow the key.
    fn toggle_fullscreen(&self) {
        if let Some(ui) = self.ui.upgrade() {
            self.set_fullscreen(!ui.window().is_fullscreen());
        }
    }

    /// `SetFullScreen`. The UI follows at once (no frame with the title bar over the monitor); `frame::on_fullscreen_changed`
    /// confirms or corrects it from the real window state afterwards.
    fn set_fullscreen(&self, on: bool) {
        let Some(ui) = self.ui.upgrade() else { return };
        self.show_fullscreen(on);
        ui.window().set_fullscreen(on);
    }

    /// The title bar, footer and icon strip hide with `AppState.fullscreen`; the image is refitted to the new view.
    fn show_fullscreen(&self, on: bool) {
        let Some(ui) = self.ui.upgrade() else { return };
        let app = ui.global::<AppState>();
        if app.get_fullscreen() == on {
            return;
        }
        app.set_fullscreen(on);
        self.host.fit_after_resize();
    }

    /// `Context.UpdateFileInfo`, only while the pane is open (opening refreshes it too).
    fn update_info(&self) {
        let Some(ui) = self.ui.upgrade() else { return };
        if !ui.global::<AppState>().get_info_open() {
            return;
        }
        let info = {
            let doc = self.doc.borrow();
            // The save worker holds the picture: the pane keeps what it shows until the save hands it back.
            if doc.saving && doc.picture.is_none() {
                return;
            }
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

    /// `Context.RotateFlip`.
    fn rotate_flip(&self, rotate: Rotate, flip: Flip) {
        if self.dialog_open() {
            return;
        }
        self.edit(|p| p.rotate_flip(rotate, flip));
    }

    /// `ButtonImageCrop_Click`, inline: the animation stops, the info pane closes, the image is fitted above the panel and locked
    /// (through `refresh_ui`), every command is off, the selection covers the whole image with the free ratio.
    fn start_crop(&self) {
        if self.dialog_open() {
            return;
        }
        let session = {
            let mut doc = self.doc.borrow_mut();
            let Some(image) = doc.picture.as_ref().filter(|_| doc.crop.is_none() && !doc.loading && !doc.saving).map(|p| (p.width(), p.height())) else {
                return;
            };
            let session = CropSession::new(image);
            doc.crop = Some(session);
            session
        };
        self.stop_animation();
        // The pane would cover the right handles.
        self.close_info();
        if let Some(ui) = self.ui.upgrade() {
            ui.set_crop_ratio_index(0);
        }
        // Locks the host, which refits the image above the panel.
        self.refresh_ui();
        self.push_crop(session.sel);
    }

    /// The selection, in image pixels, to the overlay (which maps it onto the image rectangle).
    fn push_crop(&self, sel: Sel) {
        let Some(ui) = self.ui.upgrade() else { return };
        let width = self.doc.borrow().picture.as_ref().map_or(1, Picture::width);
        ui.set_crop_image_width(width as f32);
        ui.set_crop_x(sel.x);
        ui.set_crop_y(sel.y);
        ui.set_crop_w(sel.w);
        ui.set_crop_h(sel.h);
    }

    /// Changes the open session and pushes its selection. Does nothing when the cropper is closed.
    fn update_crop(&self, change: impl FnOnce(&mut CropSession, (u32, u32))) {
        if self.dialog_open() {
            return;
        }
        let sel = {
            let mut doc = self.doc.borrow_mut();
            let Some(image) = doc.picture.as_ref().map(|p| (p.width(), p.height())) else { return };
            let Some(session) = doc.crop.as_mut() else { return };
            change(session, image);
            session.sel
        };
        self.push_crop(sel);
    }

    /// Handle order of the overlay: N, S, E, W, NE, NW, SE, SW, then the inside of the selection.
    fn crop_drag_start(&self, handle: i32) {
        let handles = [Handle::N, Handle::S, Handle::E, Handle::W, Handle::NE, Handle::NW, Handle::SE, Handle::SW, Handle::Move];
        let Some(&handle) = usize::try_from(handle).ok().and_then(|i| handles.get(i)) else { return };
        self.update_crop(|s, _| s.drag = (s.sel, handle));
    }

    /// (dx, dy): view pixels from the point where the drag began.
    fn crop_drag_move(&self, dx: f32, dy: f32) {
        let zoom = self.host.zoom();
        self.update_crop(|s, image| s.sel = crop::drag(s.drag.0, s.drag.1, dx / zoom, dy / zoom, crop::ratio(s.ratio, image), image));
    }

    fn crop_ratio_chosen(&self, index: i32) {
        let Ok(index) = usize::try_from(index) else { return };
        self.update_crop(|s, image| {
            s.ratio = index;
            s.sel = crop::apply_ratio(s.sel, crop::ratio(index, image), image);
        });
    }

    fn crop_reset(&self) {
        self.update_crop(|s, image| s.sel = crop::apply_ratio(crop::full(image), crop::ratio(s.ratio, image), image));
    }

    /// `ButtonCropValidate_Click` + `Context.Crop`: crops, closes the cropper, then the transform refresh.
    fn crop_apply(&self) {
        if self.dialog_open() {
            return;
        }
        let rect = {
            let mut doc = self.doc.borrow_mut();
            let Some(image) = doc.picture.as_ref().map(|p| (p.width(), p.height())) else { return };
            let Some(session) = doc.crop.take() else { return };
            crop::to_pixels(session.sel, image)
        };
        self.edit(|p| p.crop(rect.0, rect.1, rect.2, rect.3));
    }

    /// `Context.CloseCropper` without a change: the animation resumes.
    fn crop_cancel(&self) {
        if self.dialog_open() {
            return;
        }
        if self.doc.borrow_mut().crop.take().is_none() {
            return;
        }
        self.refresh_ui();
        if self.doc.borrow().picture.as_ref().is_some_and(Picture::is_animated) {
            self.schedule_frame(0);
        }
    }

    /// `ReloadImageView` after an edit: the view is rebuilt and fitted; an animation restarts from its first frame.
    fn edit(&self, change: impl FnOnce(&mut Picture)) {
        let Some(mut picture) = self.doc.borrow_mut().picture.take() else { return };
        change(&mut picture);
        let animated = picture.is_animated();
        if animated {
            self.stop_animation();
            self.host.show_animated(picture.current());
        } else {
            self.host.show(picture.current());
        }
        self.doc.borrow_mut().picture = Some(picture);
        self.refresh_ui();
        self.update_info();
        if animated {
            self.schedule_frame(0);
        }
    }

    /// `Context.Save`: overwrites the file in place, or routes to Save As (pasted content, svg, ico).
    fn save(&self) {
        if self.dialog_open() {
            return;
        }
        if self.modal.get() {
            return;
        }
        let route = {
            let doc = self.doc.borrow();
            save_route(doc.picture.as_ref().is_some_and(Picture::modified), doc.path.as_deref())
        };
        match route {
            Route::Nothing => {}
            Route::SaveAs => self.save_as(),
            Route::Direct(ext) => {
                let path = self.doc.borrow().path.clone();
                if let Some(path) = path {
                    self.start_save(path, ext, false);
                }
            }
        }
    }

    /// `Context.SaveAs`: the saved file becomes the current document and its folder is listed.
    fn save_as(&self) {
        if self.dialog_open() || !self.doc.borrow().can_save_as() {
            return;
        }
        // Exclusive through the dialog; once a save starts, `doc.saving` takes over until the worker hands the picture back.
        let Some((path, ext, renamed)) = self.exclusive(|| self.pick_save_target()).flatten() else { return };
        // The dialog confirmed an overwrite of the name as typed only: never replace a different existing file silently.
        if renamed && path.exists() {
            eprintln!("not overwriting {}: the dialog did not confirm that name", path.display());
            self.show_error("SYSTEM_SAVING_ERROR");
            return;
        }
        self.start_save(path, ext, true);
    }

    /// The Save As dialog: path, save type, and whether the name was changed after the dialog confirmed it.
    fn pick_save_target(&self) -> Option<(PathBuf, String, bool)> {
        let suggested = {
            let doc = self.doc.borrow();
            doc.picture.as_ref()?;
            doc.path.as_deref().and_then(Path::file_stem).map(|s| s.to_string_lossy().into_owned())
        };
        let suggested = suggested.unwrap_or_else(|| locale::save_stamp(SystemTime::now()));
        let hwnd = self.hwnd()?;
        // Modal: no borrow is held while it pumps messages.
        dialogs::pick_save(hwnd, &suggested, &SAVE_FILE_TYPES, &self.strings.borrow().get("FOOTER_TOOLBAR_MENU_FILE_SAVE_FORMAT"))
    }

    /// Hands the picture to the save worker. Until it comes back, image commands, navigation and loads are off and the UI keeps
    /// showing the state it had (title bullet included).
    fn start_save(&self, path: PathBuf, ext: String, adopt: bool) {
        let quality = quality_for(&ext, &Settings::app());
        {
            let mut doc = self.doc.borrow_mut();
            if doc.saving || doc.picture.is_none() {
                return;
            }
            doc.saving = true;
        }
        // Refreshed while the picture is still in the document, so the flags turn off and the rest stays as is.
        self.refresh_ui();
        self.stop_animation();
        let Some(picture) = self.doc.borrow_mut().picture.take() else { return };
        let target = adopt.then(|| path.clone());
        let sent = self.saver.request(picture, path, ext, quality, move |picture, result| {
            if let Some(c) = current() {
                c.saved(picture, result, target);
            }
        });
        if let Err(picture) = sent {
            self.saved(picture, Err(SaveError::Encode("the save thread is not running".into())), None);
        }
    }

    /// The worker handed the picture back. `adopt` is the Save As target.
    fn saved(&self, picture: Picture, result: Result<(), SaveError>, adopt: Option<PathBuf>) {
        let animated = picture.is_animated();
        let ok = result.is_ok();
        self.doc.borrow_mut().finish_save(picture, ok, adopt);
        match result {
            // The file was rewritten (lossy formats differ from the pixels held): this picture is not kept on leaving.
            Ok(()) => self.stamp.set(None),
            Err(e) => eprintln!("cannot save the image: {e}"),
        }
        self.refresh_ui();
        self.update_info();
        if animated {
            self.schedule_frame(0);
        }
        // A failed save keeps the window open so the edit is not lost.
        let close = self.close_pending.replace(false) && ok;
        if !ok {
            self.show_error("SYSTEM_SAVING_ERROR");
        } else if close && let Some(ui) = self.ui.upgrade() {
            ui.window().dispatch_event(WindowEvent::CloseRequested);
        }
    }

    /// Close handler: true (keep the window) while a save runs; the window then closes once the save succeeded.
    pub fn defer_close(&self) -> bool {
        let saving = self.doc.borrow().saving;
        if saving {
            self.close_pending.set(true);
        }
        saving
    }

    /// `Context.DeleteImage`: recycles the file, then shows the next file of the folder, or clears the view when none is left.
    /// Pasted content has no file: nothing is recycled. A failed recycle changes nothing (1.x swallows the exception).
    fn delete(&self) {
        if self.dialog_open() || !self.doc.borrow().can_delete() {
            return;
        }
        if self.modal.get() {
            return;
        }
        let path = self.doc.borrow().path.clone();
        if let Some(path) = path {
            let Some(hwnd) = self.hwnd() else { return };
            // The shell's error UI pumps messages: no borrow is held and a second delete is dropped meanwhile.
            if self.exclusive(|| shell::recycle(hwnd, &path)) != Some(true) {
                return;
            }
            self.doc.borrow_mut().remove_deleted();
        }
        let has_files = self.doc.borrow().folder.as_ref().is_some_and(|f| !f.files.is_empty());
        if has_files {
            self.next();
        }
        // Also when every remaining file failed to open: 1.x would keep showing the deleted picture.
        if self.doc.borrow().path.is_none() {
            self.stop_animation();
            self.doc.borrow_mut().picture = None;
            self.host.clear();
            self.set_icon_sizes(None);
            self.close_info();
            self.refresh_ui();
        }
    }

    /// `Context.Print`: the system print dialog, then the shown frame fitted on the page. The picture is copied out so no borrow spans the dialog.
    fn print(&self) {
        if self.dialog_open() {
            return;
        }
        let Some(hwnd) = self.hwnd() else { return };
        let (pixels, name) = {
            let doc = self.doc.borrow();
            let Some(picture) = doc.picture.as_ref() else { return };
            let name = match doc.path.as_deref().and_then(Path::file_name) {
                Some(n) => n.to_string_lossy().into_owned(),
                None => self.strings.borrow().get("SYSTEM_PASTED_CONTENT"),
            };
            (picture.current().clone(), name)
        };
        if let Some(Err(e)) = self.exclusive(move || print::print(hwnd, pixels, &name)) {
            eprintln!("cannot print the image: {e}");
            self.show_error("SYSTEM_PRINTING_ERROR");
        }
    }

    /// `CopyImageToClipboard`: the shown frame as CF_DIB (1.x) and CF_DIBV5 (an explicit alpha mask for readers that use it).
    /// A failure is only logged, as in 1.x. An unchanged animated picture also carries its file (CF_HDROP), so file targets paste the animation.
    fn copy(&self) {
        let Some(hwnd) = self.hwnd() else { return };
        let (v5, dib, file) = {
            let doc = self.doc.borrow();
            let Some(picture) = doc.picture.as_ref() else { return };
            let (bgra, width, height) = picture.bgra_pixels();
            let exists = doc.path.as_deref().is_some_and(exists);
            let file = clipboard_file(picture.is_animated(), picture.modified(), doc.path.as_deref(), exists).map(Path::to_path_buf);
            (dib::build_dib_v5(&bgra, width, height), dib::build_dib(&bgra, width, height), file)
        };
        if !clipboard::set_dibs(hwnd, &v5, &dib, file.as_deref()) {
            eprintln!("cannot copy the image to the clipboard");
        }
    }

    /// `Window_Paste`: only reacts when the clipboard holds a bitmap. The DIB is parsed on the decode worker under the loading state
    /// of a file load (commands off, ring shown); a payload that fails to parse puts the previous image back untouched. Refused while
    /// a load, a paste or a save runs (the loader keeps only the latest request, so a paste would drop the pending load's completion).
    fn paste(&self) {
        if self.dialog_open() {
            return;
        }
        {
            let doc = self.doc.borrow();
            if doc.loading || doc.saving {
                return;
            }
        }
        let Some(hwnd) = self.hwnd() else { return };
        let Some(bytes) = clipboard::get_dib(hwnd) else { return };
        self.close_info();
        self.stop_animation();
        let generation = self.doc.borrow_mut().begin_paste();
        self.refresh_ui();
        self.loader.request(Source::Dib(bytes), generation, move |generation, result, decode| {
            if let Some(c) = current() {
                c.pasted(generation, result, decode);
            }
        });
    }

    /// `LoadImageFromBuffer`: a parsed DIB becomes an in-memory document with no path (modified from the start). The folder list
    /// is kept, as 1.x only clears `CurrentFilePath`.
    fn pasted(&self, generation: u64, result: Result<Picture, LoadError>, decode: Duration) {
        match result {
            Ok(picture) => {
                // A load started meanwhile owns the view now (`finished` drops the stale result too, but the path must stay).
                {
                    let mut doc = self.doc.borrow_mut();
                    if doc.generation != generation {
                        return;
                    }
                    doc.path = None;
                }
                self.finished(generation, Ok(picture), decode, None);
            }
            Err(e) => {
                if !self.doc.borrow_mut().cancel_paste(generation) {
                    return;
                }
                eprintln!("cannot paste the image: {e}");
                self.refresh_ui();
                if self.doc.borrow().picture.as_ref().is_some_and(Picture::is_animated) {
                    self.schedule_frame(0);
                }
            }
        }
    }

    /// `ShowErrorAsync`: one dialog at a time, a second one waits for the open one to close.
    fn show_error(&self, key: &str) {
        let Some(ui) = self.ui.upgrade() else { return };
        if ui.get_dialog_open() {
            self.doc.borrow_mut().defer_error(key);
            return;
        }
        ui.set_error_message(self.strings.borrow().get(key).into());
        ui.set_error_open(true);
    }

    /// A dialog closed: shows the error that was raised behind it, if any.
    fn dialog_closed(&self) {
        let key = self.doc.borrow_mut().take_error();
        if let Some(key) = key {
            self.show_error(&key);
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
            if let Some(c) = current() {
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
            (index, buffer_of(&frames[index].pixels))
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
                            thumb: slint::Image::from_rgba8(buffer_of(pixels)),
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

    /// The texts Rust pushes itself, after the language changed: the crop ratio names and the window title.
    pub fn language_changed(&self) {
        self.push_crop_ratios();
        self.refresh_ui();
    }

    fn push_crop_ratios(&self) {
        let Some(ui) = self.ui.upgrade() else { return };
        let strings = self.strings.borrow();
        let labels: Vec<SharedString> = crop::RATIOS.iter().enumerate().map(|(i, (label, _))| if i < 2 { strings.get(label).into() } else { (*label).into() }).collect();
        ui.set_crop_ratios(ModelRc::new(VecModel::from(labels)));
    }

    /// `UpdateButtonsAccessiblity` + `LoadingDisplay`: command flags, title, dimensions, ring and error panel.
    pub fn refresh_ui(&self) {
        let Some(ui) = self.ui.upgrade() else { return };
        let app = ui.global::<AppState>();
        let (inputs, dimensions, error) = {
            let doc = self.doc.borrow();
            // The save worker holds the picture: keep the state set when the save started.
            if doc.saving && doc.picture.is_none() {
                return;
            }
            let inputs = doc.inputs();
            let dimensions = doc.picture.as_ref().map(|p| format!("{}x{}", p.width(), p.height())).unwrap_or_default();
            (inputs, dimensions, doc.error.clone())
        };
        self.host.set_locked(inputs.cropping);
        ui.set_cropping(inputs.cropping);
        let flags = ui_state::flags(&inputs);
        app.set_image_commands(flags.image_commands);
        app.set_save_direct(flags.save_direct);
        app.set_open(flags.open);
        app.set_file_info(flags.file_info);
        app.set_navigate(flags.navigate);
        app.set_icon_strip(flags.icon_strip);
        app.set_dimensions_text(dimensions.into());
        let prefix = ui_state::title_prefix(&inputs, &self.strings.borrow().get("SYSTEM_LOADING"), &self.strings.borrow().get("SYSTEM_PASTED_CONTENT"));
        ui.set_title_text(window_title(prefix.as_deref()).into());
        ui.set_loading(inputs.loading);
        ui.set_error_visible(error.is_some());
        ui.set_error_file(error.unwrap_or_default().into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipboard_file_needs_an_unchanged_animated_picture_whose_file_exists() {
        let path = Path::new(r"C:\a.gif");
        assert_eq!(clipboard_file(true, false, Some(path), true), Some(path));
        assert_eq!(clipboard_file(false, false, Some(path), true), None);
        assert_eq!(clipboard_file(true, true, Some(path), true), None);
        assert_eq!(clipboard_file(true, false, None, true), None);
        assert_eq!(clipboard_file(true, false, Some(path), false), None);
    }
}
