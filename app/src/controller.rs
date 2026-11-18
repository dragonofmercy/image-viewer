//! Connects the `Document`, the `Loader`, the `ImageHost` and the `Commands` / `AppState` globals (the loading half of `Context` in 1.x).
//!
//! Rule: no `RefCell` borrow of the document is held across a call into Slint or `ImageHost`.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use std::time::Duration;

use slint::ComponentHandle;
use viewer_core::culture::Strings;
use viewer_core::files::{SUPPORTED_FILE_TYPES, is_supported};
use viewer_core::folder::FolderNav;
use viewer_core::picture::{LoadError, Picture};

use crate::document::{Document, Loader, Source};
use crate::image_host::ImageHost;
use crate::win::dialogs;
use crate::{AppState, Commands, MainWindow, frame, ui_state, window_title};

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
}

impl Controller {
    pub fn new(ui: &MainWindow, host: Rc<ImageHost>, strings: Rc<Strings>) -> Rc<Self> {
        let controller = Rc::new(Self {
            ui: ui.as_weak(),
            host,
            doc: RefCell::default(),
            loader: Loader,
            strings,
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
        self.doc.borrow_mut().loading = true;
        self.refresh_ui();
        let c = self.clone();
        slint::Timer::single_shot(Duration::from_millis(50), move || {
            if !c.open_path(&path) {
                c.doc.borrow_mut().loading = false;
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
        let Some((generation, path)) = started else { return false };
        self.refresh_ui();
        self.loader.request(Source::Path(path), generation, |generation, result| {
            if let Some(c) = CURRENT.with(|c| c.borrow().upgrade()) {
                c.finished(generation, result);
            }
        });
        true
    }

    fn finished(&self, generation: u64, result: Result<Picture, LoadError>) {
        if !self.doc.borrow_mut().finish_load(generation, result) {
            return;
        }
        // Taken out for the call so no borrow is held while the host pushes to Slint.
        let picture = self.doc.borrow_mut().picture.take();
        match &picture {
            Some(p) => self.host.show(p.current()),
            None => self.host.clear(),
        }
        self.doc.borrow_mut().picture = picture;
        self.refresh_ui();
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
