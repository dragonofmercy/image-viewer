//! Update service: checks, downloads and applies Velopack updates on worker threads, reporting on the UI thread.
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use velopack::{UpdateCheck, UpdateInfo};
use viewer_core::settings::Settings;

use crate::{frame, install};

#[derive(Debug, Clone, PartialEq)]
pub enum State {
    Idle,
    Checking,
    UpToDate,
    Available,
    Downloading(u8),
    /// `message` is empty for a network failure (the UI shows its own text).
    Failed {
        message: String,
        during_download: bool,
    },
}

/// Blocking operations of one update source. Errors are the UI message: empty for a network failure.
trait Backend: Send + Sync {
    fn installed(&self) -> bool;
    fn has_pending(&self) -> bool;
    fn check(&self) -> Result<bool, String>;
    fn download(&self, progress: &(dyn Fn(u8) + Sync)) -> Result<(), String>;
    fn apply(&self) -> Result<(), String>;
}

/// ureq 3 formats a non-2xx response as "http status: <code>" (inside velopack's "Network error: Http error: ...").
// shortcut: matches the error text, switch to the typed error if velopack exposes it.
fn is_server_response(text: &str) -> bool {
    text.contains("http status: ")
}

fn message_of(e: velopack::Error) -> String {
    let text = e.to_string();
    match e {
        velopack::Error::Network(_) if !is_server_response(&text) => String::new(),
        _ => text,
    }
}

#[derive(Default)]
struct Velopack {
    pending: Mutex<Option<UpdateInfo>>,
}

impl Velopack {
    fn pending(&self) -> Option<UpdateInfo> {
        self.pending.lock().ok()?.clone()
    }
}

impl Backend for Velopack {
    fn installed(&self) -> bool {
        install::installed().is_some()
    }

    fn has_pending(&self) -> bool {
        self.pending().is_some()
    }

    fn check(&self) -> Result<bool, String> {
        let um = install::installed().ok_or_else(|| "not installed".to_string())?;
        let found = match um.check_for_updates().map_err(message_of)? {
            UpdateCheck::UpdateAvailable(info) => Some(*info),
            _ => None,
        };
        if let Err(e) = Settings::app().touch_last_update_check() {
            eprintln!("cannot save the update check date: {e}");
        }
        let available = found.is_some();
        if let Ok(mut pending) = self.pending.lock() {
            *pending = found;
        }
        Ok(available)
    }

    fn download(&self, progress: &(dyn Fn(u8) + Sync)) -> Result<(), String> {
        let (um, info) = install::installed().zip(self.pending()).ok_or_else(|| "nothing to download".to_string())?;
        let (tx, rx) = mpsc::channel::<i16>();
        std::thread::scope(|s| {
            s.spawn(move || rx.iter().for_each(|p| progress(p.clamp(0, 100) as u8)));
            um.download_updates(&info, Some(tx)).map_err(message_of)
        })
    }

    fn apply(&self) -> Result<(), String> {
        let (um, info) = install::installed().zip(self.pending()).ok_or_else(|| "nothing to apply".to_string())?;
        um.wait_exit_then_apply_updates(&info, false, true, Vec::<String>::new()).map_err(message_of)
    }
}

enum Outcome {
    Ready,
    UpToDate,
    Failed { message: String, during_download: bool },
}

/// Worker results, delivered on the UI thread.
enum Msg {
    /// `quiet`: background check, nothing shown but a found update.
    Checked {
        result: Result<bool, String>,
        quiet: bool,
    },
    Progress(u8),
    Done(Outcome),
}

type Post = Arc<dyn Fn(Msg) + Send + Sync>;

thread_local! {
    // The event loop only carries `Send` closures: worker results reach the service through this.
    static CURRENT: RefCell<Weak<Updates>> = const { RefCell::new(Weak::new()) };
}

fn post_to_ui(msg: Msg) {
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(updates) = CURRENT.with(|c| c.borrow().upgrade()) {
            updates.on_msg(msg);
        }
    });
}

pub struct Updates {
    backend: Arc<dyn Backend>,
    post: Post,
    on_state: Box<dyn Fn(State)>,
    state: RefCell<State>,
    found: RefCell<Option<Box<dyn FnOnce()>>>,
    downloading: Cell<bool>,
    abandoned: Cell<bool>,
    progress: Cell<u8>,
}

impl Updates {
    /// `on_state` runs on the UI thread.
    pub fn new(on_state: impl Fn(State) + 'static) -> Rc<Self> {
        let updates = Self::with_backend(Arc::new(Velopack::default()), Arc::new(post_to_ui), Box::new(on_state));
        CURRENT.with(|c| *c.borrow_mut() = Rc::downgrade(&updates));
        updates
    }

    fn with_backend(backend: Arc<dyn Backend>, post: Post, on_state: Box<dyn Fn(State)>) -> Rc<Self> {
        Rc::new(Self {
            backend,
            post,
            on_state,
            state: RefCell::new(State::Idle),
            found: RefCell::new(None),
            downloading: Cell::new(false),
            abandoned: Cell::new(false),
            progress: Cell::new(0),
        })
    }

    pub fn state(&self) -> State {
        self.state.borrow().clone()
    }

    fn set_state(&self, state: State) {
        *self.state.borrow_mut() = state.clone();
        (self.on_state)(state);
    }

    fn busy(&self) -> bool {
        self.downloading.get() || *self.state.borrow() == State::Checking
    }

    /// Manual check (About button).
    pub fn check(self: &Rc<Self>) {
        if self.busy() {
            return;
        }
        if !self.backend.installed() {
            return self.set_state(State::UpToDate);
        }
        self.set_state(State::Checking);
        self.spawn_check(false);
    }

    /// Background check at startup: `found` runs on the UI thread when an update exists.
    pub fn check_if_due(self: &Rc<Self>, found: impl FnOnce() + 'static) {
        if self.busy() || !self.backend.installed() || !Settings::app().update_check_due() {
            return;
        }
        *self.found.borrow_mut() = Some(Box::new(found));
        self.spawn_check(true);
    }

    fn spawn_check(&self, quiet: bool) {
        let (backend, post) = (self.backend.clone(), self.post.clone());
        std::thread::spawn(move || post(Msg::Checked { result: backend.check(), quiet }));
    }

    /// Checks first when nothing is pending.
    pub fn download_and_restart(self: &Rc<Self>) {
        // About was reopened while the abandoned download still runs: adopt it.
        if self.downloading.get() && self.abandoned.replace(false) {
            return self.set_state(State::Downloading(self.progress.get()));
        }
        if self.busy() {
            return;
        }
        if !self.backend.installed() {
            return self.set_state(State::UpToDate);
        }
        self.downloading.set(true);
        self.progress.set(0);
        self.set_state(State::Downloading(0));
        let (backend, post) = (self.backend.clone(), self.post.clone());
        std::thread::spawn(move || post(Msg::Done(run_download(&*backend, &post))));
    }

    /// About closed during a download: the update is not applied.
    pub fn abandon(&self) {
        if self.downloading.get() {
            self.abandoned.set(true);
            *self.state.borrow_mut() = State::Available;
        }
    }

    fn on_msg(&self, msg: Msg) {
        match msg {
            Msg::Checked { result, quiet } => {
                let found = self.found.borrow_mut().take();
                // A background check ended while the user started a download: it must not overwrite the download state.
                if self.downloading.get() {
                    return;
                }
                match result {
                    Ok(true) => {
                        self.set_state(State::Available);
                        if let Some(f) = found {
                            f();
                        }
                    }
                    Ok(false) if !quiet => self.set_state(State::UpToDate),
                    Ok(false) => {}
                    Err(message) if quiet => eprintln!("background update check failed: {message}"),
                    Err(message) => self.set_state(State::Failed { message, during_download: false }),
                }
            }
            Msg::Progress(p) => {
                self.progress.set(p);
                if !self.abandoned.get() {
                    self.set_state(State::Downloading(p));
                }
            }
            Msg::Done(outcome) => {
                self.downloading.set(false);
                if self.abandoned.replace(false) {
                    // Nobody is watching: record the outcome without notifying, never apply.
                    *self.state.borrow_mut() = match outcome {
                        Outcome::Ready => State::Available,
                        Outcome::UpToDate => State::UpToDate,
                        Outcome::Failed { message, during_download } => State::Failed { message, during_download },
                    };
                    return;
                }
                match outcome {
                    Outcome::UpToDate => self.set_state(State::UpToDate),
                    Outcome::Failed { message, during_download } => self.set_state(State::Failed { message, during_download }),
                    Outcome::Ready => match self.backend.apply() {
                        Ok(()) => {
                            frame::flush_geometry();
                            crate::win::toast::clear();
                            let _ = slint::quit_event_loop();
                        }
                        Err(message) => self.set_state(State::Failed { message, during_download: true }),
                    },
                }
            }
        }
    }
}

/// Worker body of `download_and_restart`.
fn run_download(backend: &dyn Backend, post: &Post) -> Outcome {
    if !backend.has_pending() {
        match backend.check() {
            Ok(true) => {}
            Ok(false) => return Outcome::UpToDate,
            Err(message) => return Outcome::Failed { message, during_download: false },
        }
    }
    match backend.download(&|p| post(Msg::Progress(p))) {
        Ok(()) => Outcome::Ready,
        Err(message) => Outcome::Failed { message, during_download: true },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Fake {
        installed: bool,
        check: Result<bool, String>,
        applied: AtomicUsize,
    }

    impl Fake {
        fn new(installed: bool, check: Result<bool, String>) -> Arc<Self> {
            Arc::new(Self { installed, check, applied: AtomicUsize::new(0) })
        }
    }

    impl Backend for Fake {
        fn installed(&self) -> bool {
            self.installed
        }
        fn has_pending(&self) -> bool {
            true
        }
        fn check(&self) -> Result<bool, String> {
            self.check.clone()
        }
        fn download(&self, progress: &(dyn Fn(u8) + Sync)) -> Result<(), String> {
            progress(50);
            Ok(())
        }
        fn apply(&self) -> Result<(), String> {
            self.applied.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    type Rig = (Rc<Updates>, mpsc::Receiver<Msg>, Rc<RefCell<Vec<State>>>);

    fn rig(backend: Arc<Fake>) -> Rig {
        let (tx, rx) = mpsc::channel();
        let tx = Mutex::new(tx);
        let seen = Rc::new(RefCell::new(Vec::new()));
        let log = seen.clone();
        let updates = Updates::with_backend(backend, Arc::new(move |m| drop(tx.lock().unwrap().send(m))), Box::new(move |s| log.borrow_mut().push(s)));
        (updates, rx, seen)
    }

    /// Delivers worker messages until the operation ends (`Done`, or the single `Checked`).
    fn pump(updates: &Updates, rx: &mpsc::Receiver<Msg>) {
        loop {
            let msg = rx.recv().unwrap();
            let last = matches!(msg, Msg::Done(_) | Msg::Checked { .. });
            updates.on_msg(msg);
            if last {
                return;
            }
        }
    }

    #[test]
    fn server_status_errors_are_told_apart_from_connection_failures() {
        assert!(is_server_response("Network error: Http error: http status: 403"));
        assert!(is_server_response("Network error: Http error: http status: 404"));
        assert!(!is_server_response("Network error: Http error: io: failed to lookup address information: No such host is known. (os error 11001)"));
        assert!(!is_server_response("Network error: Http error: timeout: global"));
    }

    #[test]
    fn not_installed_is_up_to_date() {
        let (updates, _rx, seen) = rig(Fake::new(false, Ok(true)));
        updates.check();
        assert_eq!(updates.state(), State::UpToDate);
        assert_eq!(*seen.borrow(), vec![State::UpToDate]);
    }

    #[test]
    fn check_error_fails_outside_download() {
        let (updates, rx, seen) = rig(Fake::new(true, Err("boom".into())));
        updates.check();
        assert_eq!(updates.state(), State::Checking);
        pump(&updates, &rx);
        assert_eq!(updates.state(), State::Failed { message: "boom".into(), during_download: false });
        assert_eq!(seen.borrow().len(), 2);
    }

    #[test]
    fn check_finds_an_update() {
        let (updates, rx, _seen) = rig(Fake::new(true, Ok(true)));
        updates.check();
        pump(&updates, &rx);
        assert_eq!(updates.state(), State::Available);
    }

    #[test]
    fn abandon_during_download_does_not_apply() {
        let backend = Fake::new(true, Ok(true));
        let (updates, rx, _seen) = rig(backend.clone());
        updates.download_and_restart();
        assert_eq!(updates.state(), State::Downloading(0));
        updates.abandon();
        pump(&updates, &rx);
        assert_eq!(updates.state(), State::Available);
        assert_eq!(backend.applied.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn quiet_check_during_download_does_not_unabandon_it() {
        let backend = Fake::new(true, Ok(true));
        let (updates, rx, _seen) = rig(backend.clone());
        updates.download_and_restart();
        updates.on_msg(Msg::Checked { result: Ok(true), quiet: true });
        assert_eq!(updates.state(), State::Downloading(0));
        updates.abandon();
        pump(&updates, &rx);
        assert_eq!(backend.applied.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn reopening_adopts_an_abandoned_download() {
        let backend = Fake::new(true, Ok(true));
        let (updates, rx, _seen) = rig(backend.clone());
        updates.download_and_restart();
        updates.abandon();
        updates.download_and_restart();
        assert!(matches!(updates.state(), State::Downloading(_)));
        // The fake's Done(Ready) would now apply and quit the loop: only check the adoption, not the apply.
        drop(rx);
    }
}
