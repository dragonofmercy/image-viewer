//! Update service: checks and downloads Velopack updates on worker threads, reporting on the UI thread; installs on an explicit click.
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use velopack::{UpdateCheck, UpdateInfo};
use viewer_core::settings::Settings;

use crate::install;

#[derive(Debug, Clone, PartialEq)]
pub enum State {
    Idle,
    Checking,
    UpToDate,
    Available,
    Downloading(u8),
    /// A downloaded package waits for the Install click, and no newer remote version is known.
    Ready,
    /// `message` is empty for a network failure (the UI shows its own text).
    Failed {
        message: String,
        during_download: bool,
    },
}

/// The toast a background check asks for: a new version to download, or a downloaded one to install.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Toast {
    Available,
    Ready,
}

/// The worker download, with its last reported percentage; `Abandoned` when About was closed meanwhile.
#[derive(Clone, Copy, PartialEq)]
enum Download {
    None,
    Active(u8),
    Abandoned(u8),
}

/// What a check found: `latest` is newer than the running version, `pending` is the downloaded package waiting.
struct Checked {
    latest: Option<String>,
    pending: Option<String>,
}

/// Blocking operations of one update source. Errors are the UI message: empty for a network failure.
trait Backend: Send + Sync {
    fn installed(&self) -> bool;
    /// A check already found the update that `download` fetches.
    fn has_latest(&self) -> bool;
    /// Version of the downloaded package not applied yet (no network).
    fn pending_version(&self) -> Option<String>;
    /// The remote latest version when it is newer than the running one, with the pending package looked up on the same worker.
    fn check(&self) -> Result<Checked, String>;
    fn download(&self, progress: &(dyn Fn(u8) + Sync)) -> Result<(), String>;
    /// Starts `Update.exe`, which waits for this process to exit, applies the pending package and restarts.
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
    latest: Mutex<Option<UpdateInfo>>,
}

impl Velopack {
    fn latest(&self) -> Option<UpdateInfo> {
        self.latest.lock().ok()?.clone()
    }
}

impl Backend for Velopack {
    fn installed(&self) -> bool {
        install::installed().is_some()
    }

    fn has_latest(&self) -> bool {
        self.latest().is_some()
    }

    fn pending_version(&self) -> Option<String> {
        install::installed()?.get_update_pending_restart().map(|asset| asset.Version)
    }

    fn check(&self) -> Result<Checked, String> {
        let um = install::installed().ok_or_else(|| "not installed".to_string())?;
        let found = match um.check_for_updates().map_err(message_of)? {
            UpdateCheck::UpdateAvailable(info) => Some(*info),
            _ => None,
        };
        if let Err(e) = Settings::app().touch_last_update_check() {
            eprintln!("cannot save the update check date: {e}");
        }
        let version = found.as_ref().map(|info| info.TargetFullRelease.Version.clone());
        if let Ok(mut latest) = self.latest.lock() {
            *latest = found;
        }
        Ok(Checked {
            latest: version,
            pending: self.pending_version(),
        })
    }

    fn download(&self, progress: &(dyn Fn(u8) + Sync)) -> Result<(), String> {
        let (um, info) = install::installed().zip(self.latest()).ok_or_else(|| "nothing to download".to_string())?;
        let (tx, rx) = mpsc::channel::<i16>();
        std::thread::scope(|s| {
            s.spawn(move || rx.iter().for_each(|p| progress(p.clamp(0, 100) as u8)));
            um.download_updates(&info, Some(tx)).map_err(message_of)
        })
    }

    fn apply(&self) -> Result<(), String> {
        let um = install::installed().ok_or_else(|| "not installed".to_string())?;
        let asset = um.get_update_pending_restart().ok_or_else(|| "nothing to apply".to_string())?;
        um.wait_exit_then_apply_updates(&asset, false, true, Vec::<String>::new()).map_err(message_of)
    }
}

/// Worker results, delivered on the UI thread.
enum Msg {
    /// `quiet`: background check, nothing shown but a toast.
    Checked {
        result: Result<Checked, String>,
        quiet: bool,
    },
    Progress(u8),
    Done(State),
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

type OnState = Box<dyn Fn(State, bool)>;
type Found = Box<dyn FnOnce(Toast)>;

pub struct Updates {
    backend: Arc<dyn Backend>,
    post: Post,
    on_state: OnState,
    quit: Box<dyn Fn()>,
    state: RefCell<State>,
    found: RefCell<Option<Found>>,
    download: Cell<Download>,
    /// `Update.exe` already waits for the exit: a second Install click while a save delays the close must not start another.
    applied: Cell<bool>,
}

impl Updates {
    /// `on_state` runs on the UI thread with the state and `installable` (only looked up for a failure, the one state where About needs it).
    /// `quit` closes the app once `Update.exe` waits for it.
    pub fn new(on_state: impl Fn(State, bool) + 'static, quit: impl Fn() + 'static) -> Rc<Self> {
        let updates = Self::with_backend(Arc::new(Velopack::default()), Arc::new(post_to_ui), Box::new(on_state), Box::new(quit));
        CURRENT.with(|c| *c.borrow_mut() = Rc::downgrade(&updates));
        updates
    }

    fn with_backend(backend: Arc<dyn Backend>, post: Post, on_state: OnState, quit: Box<dyn Fn()>) -> Rc<Self> {
        let state = if backend.pending_version().is_some() { State::Ready } else { State::Idle };
        Rc::new(Self {
            backend,
            post,
            on_state,
            quit,
            state: RefCell::new(state),
            found: RefCell::new(None),
            download: Cell::new(Download::None),
            applied: Cell::new(false),
        })
    }

    pub fn state(&self) -> State {
        self.state.borrow().clone()
    }

    /// A failure still offers Install when a downloaded package is on disk. Looked up only for a failure: it reads the packages folder, too slow for every progress tick.
    pub fn installable_for(&self, state: &State) -> bool {
        matches!(state, State::Failed { .. }) && self.backend.pending_version().is_some()
    }

    fn set_state(&self, state: State) {
        *self.state.borrow_mut() = state.clone();
        let installable = self.installable_for(&state);
        (self.on_state)(state, installable);
    }

    fn busy(&self) -> bool {
        self.download.get() != Download::None || *self.state.borrow() == State::Checking
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

    /// Background check at startup: `found` runs on the UI thread with the toast to show, if any.
    pub fn check_if_due(self: &Rc<Self>, found: impl FnOnce(Toast) + 'static) {
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

    /// Downloads the update, checking first when none is known; ends in `Ready`, never applies.
    pub fn download(self: &Rc<Self>) {
        // About was reopened while the abandoned download still runs: adopt it.
        if let Download::Abandoned(p) = self.download.get() {
            self.download.set(Download::Active(p));
            return self.set_state(State::Downloading(p));
        }
        // Ready is final only when this process checked: one started by the toast must look for a version newer than the pending package.
        if self.busy() || (*self.state.borrow() == State::Ready && self.backend.has_latest()) {
            return;
        }
        if !self.backend.installed() {
            return self.set_state(State::UpToDate);
        }
        self.download.set(Download::Active(0));
        self.set_state(State::Downloading(0));
        let (backend, post) = (self.backend.clone(), self.post.clone());
        std::thread::spawn(move || post(Msg::Done(run_download(&*backend, &post))));
    }

    /// Install click: `Update.exe` waits for the app to exit, so it quits through the normal close path (a running save finishes first).
    pub fn install(&self) {
        if self.applied.get() {
            return;
        }
        match self.backend.apply() {
            Ok(()) => {
                self.applied.set(true);
                (self.quit)();
            }
            Err(message) => self.set_state(State::Failed { message, during_download: true }),
        }
    }

    /// About closed during a download: it goes on unwatched.
    pub fn abandon(&self) {
        if let Download::Active(p) | Download::Abandoned(p) = self.download.get() {
            self.download.set(Download::Abandoned(p));
            *self.state.borrow_mut() = State::Available;
        }
    }

    fn on_msg(&self, msg: Msg) {
        match msg {
            Msg::Checked { result, quiet } => {
                // Only a background check shows a toast.
                let found = if quiet { self.found.borrow_mut().take() } else { None };
                // A background check ended while the user started a download: it must not overwrite the download state.
                if self.download.get() != Download::None {
                    return;
                }
                let state = match result {
                    Err(message) if quiet => return eprintln!("background update check failed: {message}"),
                    Err(message) => return self.set_state(State::Failed { message, during_download: false }),
                    Ok(Checked { latest, pending }) => match pending {
                        // Plain equality: Velopack deletes older packages and the feed's latest is never older than a downloaded one.
                        Some(pending) if latest.as_ref().is_none_or(|v| *v == pending) => State::Ready,
                        _ if latest.is_some() => State::Available,
                        _ if quiet => return,
                        _ => State::UpToDate,
                    },
                };
                let toast = match (&state, &*self.state.borrow()) {
                    (State::Ready, State::Ready) => Some(Toast::Ready),
                    (State::Available, before) if *before != State::Available => Some(Toast::Available),
                    _ => None,
                };
                self.set_state(state);
                if let Some((f, toast)) = found.zip(toast) {
                    f(toast);
                }
            }
            Msg::Progress(p) => match self.download.get() {
                Download::Active(_) => {
                    self.download.set(Download::Active(p));
                    self.set_state(State::Downloading(p));
                }
                Download::Abandoned(_) => self.download.set(Download::Abandoned(p)),
                Download::None => {}
            },
            Msg::Done(state) => {
                if matches!(self.download.replace(Download::None), Download::Abandoned(_)) {
                    // Nobody is watching: record the outcome without notifying.
                    *self.state.borrow_mut() = state;
                    return;
                }
                self.set_state(state);
            }
        }
    }
}

/// Worker body of `download`.
fn run_download(backend: &dyn Backend, post: &Post) -> State {
    if !backend.has_latest() {
        match backend.check() {
            Ok(Checked { latest: Some(_), .. }) => {}
            Ok(Checked { pending: Some(_), .. }) => return State::Ready,
            Ok(Checked { .. }) => return State::UpToDate,
            Err(message) => return State::Failed { message, during_download: false },
        }
    }
    match backend.download(&|p| post(Msg::Progress(p))) {
        Ok(()) => State::Ready,
        Err(message) => State::Failed { message, during_download: true },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Fake {
        installed: bool,
        has_latest: bool,
        check: Result<Option<String>, String>,
        pending: Option<String>,
        apply: Result<(), String>,
        applied: AtomicUsize,
        downloads: AtomicUsize,
    }

    /// An installed app whose check answers `check`, with a downloaded package of version `pending`.
    fn fake(check: Result<Option<&str>, &str>, pending: Option<&str>) -> Fake {
        Fake {
            installed: true,
            has_latest: true,
            check: check.map(|v| v.map(Into::into)).map_err(Into::into),
            pending: pending.map(Into::into),
            apply: Ok(()),
            applied: AtomicUsize::new(0),
            downloads: AtomicUsize::new(0),
        }
    }

    impl Backend for Fake {
        fn installed(&self) -> bool {
            self.installed
        }
        fn has_latest(&self) -> bool {
            self.has_latest
        }
        fn pending_version(&self) -> Option<String> {
            self.pending.clone()
        }
        fn check(&self) -> Result<Checked, String> {
            self.check.clone().map(|latest| Checked { latest, pending: self.pending.clone() })
        }
        fn download(&self, progress: &(dyn Fn(u8) + Sync)) -> Result<(), String> {
            self.downloads.fetch_add(1, Ordering::SeqCst);
            progress(50);
            Ok(())
        }
        fn apply(&self) -> Result<(), String> {
            self.applied.fetch_add(1, Ordering::SeqCst);
            self.apply.clone()
        }
    }

    struct Rig {
        updates: Rc<Updates>,
        rx: mpsc::Receiver<Msg>,
        /// Every `on_state` call: the state and its `installable` flag.
        seen: Rc<RefCell<Vec<(State, bool)>>>,
        quits: Rc<Cell<usize>>,
        backend: Arc<Fake>,
    }

    fn rig(backend: Fake) -> Rig {
        let backend = Arc::new(backend);
        let (tx, rx) = mpsc::channel();
        let tx = Mutex::new(tx);
        let seen = Rc::new(RefCell::new(Vec::new()));
        let quits = Rc::new(Cell::new(0));
        let (log, q) = (seen.clone(), quits.clone());
        let updates = Updates::with_backend(
            backend.clone(),
            Arc::new(move |m| drop(tx.lock().unwrap().send(m))),
            Box::new(move |s, i| log.borrow_mut().push((s, i))),
            Box::new(move || q.set(q.get() + 1)),
        );
        Rig { updates, rx, seen, quits, backend }
    }

    impl Rig {
        /// Delivers worker messages until the operation ends (`Done`, or the single `Checked`).
        fn pump(&self) {
            loop {
                // A missing worker message fails the test instead of hanging it.
                let msg = self.rx.recv_timeout(std::time::Duration::from_secs(10)).expect("no worker message");
                let last = matches!(msg, Msg::Done(_) | Msg::Checked { .. });
                self.updates.on_msg(msg);
                if last {
                    return;
                }
            }
        }

        fn applied(&self) -> usize {
            self.backend.applied.load(Ordering::SeqCst)
        }

        /// Registers a quiet check's `found` callback, recording the toasts it is asked for.
        fn expect_toast(&self) -> Rc<RefCell<Vec<Toast>>> {
            let toasts = Rc::new(RefCell::new(Vec::new()));
            let t = toasts.clone();
            *self.updates.found.borrow_mut() = Some(Box::new(move |toast| t.borrow_mut().push(toast)));
            toasts
        }
    }

    fn checked(latest: Option<&str>, pending: Option<&str>, quiet: bool) -> Msg {
        Msg::Checked {
            result: Ok(Checked {
                latest: latest.map(Into::into),
                pending: pending.map(Into::into),
            }),
            quiet,
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
        let r = rig(Fake {
            installed: false,
            ..fake(Ok(Some("2.1.0")), None)
        });
        r.updates.check();
        assert_eq!(r.updates.state(), State::UpToDate);
        assert_eq!(*r.seen.borrow(), vec![(State::UpToDate, false)]);
    }

    #[test]
    fn check_error_fails_outside_download() {
        let r = rig(fake(Err("boom"), None));
        r.updates.check();
        assert_eq!(r.updates.state(), State::Checking);
        r.pump();
        assert_eq!(r.updates.state(), State::Failed { message: "boom".into(), during_download: false });
        assert_eq!(r.seen.borrow().len(), 2);
    }

    #[test]
    fn check_finds_an_update() {
        let r = rig(fake(Ok(Some("2.1.0")), None));
        r.updates.check();
        r.pump();
        assert_eq!(r.updates.state(), State::Available);
    }

    #[test]
    fn download_ends_ready_and_applies_nothing() {
        let r = rig(fake(Ok(Some("2.1.0")), None));
        r.updates.download();
        r.pump();
        assert_eq!(r.updates.state(), State::Ready);
        assert_eq!(r.applied(), 0);
        assert_eq!(r.quits.get(), 0);
    }

    #[test]
    fn install_applies_then_quits_once() {
        let r = rig(fake(Ok(Some("2.1.0")), Some("2.1.0")));
        assert_eq!(r.updates.state(), State::Ready);
        r.updates.install();
        assert_eq!(r.applied(), 1);
        assert_eq!(r.quits.get(), 1);
    }

    #[test]
    fn second_install_does_nothing() {
        let r = rig(fake(Ok(Some("2.1.0")), Some("2.1.0")));
        r.updates.install();
        r.updates.install();
        assert_eq!(r.applied(), 1);
        assert_eq!(r.quits.get(), 1);
    }

    #[test]
    fn failed_apply_reports_and_stays_open() {
        let r = rig(Fake {
            apply: Err("x".into()),
            ..fake(Ok(Some("2.1.0")), Some("2.1.0"))
        });
        r.updates.install();
        assert_eq!(r.updates.state(), State::Failed { message: "x".into(), during_download: true });
        assert_eq!(r.quits.get(), 0);
    }

    #[test]
    fn starts_ready_with_a_pending_package() {
        assert_eq!(rig(fake(Ok(None), Some("2.1.0"))).updates.state(), State::Ready);
        assert_eq!(rig(fake(Ok(None), None)).updates.state(), State::Idle);
    }

    #[test]
    fn check_finding_the_pending_version_stays_ready() {
        let r = rig(fake(Ok(Some("2.1.0")), Some("2.1.0")));
        r.updates.check();
        r.pump();
        assert_eq!(r.updates.state(), State::Ready);
    }

    #[test]
    fn check_finding_a_newer_version_is_available() {
        let r = rig(fake(Ok(Some("2.2.0")), Some("2.1.0")));
        r.updates.check();
        r.pump();
        assert_eq!(r.updates.state(), State::Available);
    }

    #[test]
    fn failed_check_keeps_the_package_installable() {
        let r = rig(fake(Err(""), Some("2.1.0")));
        r.updates.check();
        r.pump();
        assert_eq!(r.updates.state(), State::Failed { message: String::new(), during_download: false });
        assert!(r.updates.installable_for(&r.updates.state()));
        assert_eq!(r.seen.borrow().last(), Some(&(State::Failed { message: String::new(), during_download: false }, true)));
    }

    #[test]
    fn manual_reply_never_shows_a_toast() {
        let r = rig(fake(Ok(Some("2.1.0")), None));
        let toasts = r.expect_toast();
        r.updates.on_msg(checked(Some("2.1.0"), None, false));
        assert_eq!(r.updates.state(), State::Available);
        r.updates.on_msg(checked(Some("2.1.0"), None, true));
        assert!(toasts.borrow().is_empty());
    }

    #[test]
    fn quiet_reply_over_ready_picks_the_toast() {
        let r = rig(fake(Ok(None), Some("2.1.0")));
        let toasts = r.expect_toast();
        r.updates.on_msg(checked(Some("2.1.0"), Some("2.1.0"), true));
        assert_eq!(*toasts.borrow(), vec![Toast::Ready]);
        assert_eq!(r.updates.state(), State::Ready);

        let r = rig(fake(Ok(None), Some("2.1.0")));
        let toasts = r.expect_toast();
        r.updates.on_msg(checked(Some("2.2.0"), Some("2.1.0"), true));
        assert_eq!(*toasts.borrow(), vec![Toast::Available]);
        assert_eq!(r.updates.state(), State::Available);
    }

    #[test]
    fn download_when_ready_does_nothing() {
        let r = rig(fake(Ok(Some("2.1.0")), Some("2.1.0")));
        r.updates.download();
        assert_eq!(r.updates.state(), State::Ready);
        assert!(r.seen.borrow().is_empty());
        assert!(r.rx.recv_timeout(std::time::Duration::from_millis(100)).is_err());
    }

    #[test]
    fn toast_download_fetches_a_newer_version_over_a_pending_one() {
        // A new process started by the toast: a package waits and no check ran yet.
        let r = rig(Fake {
            has_latest: false,
            ..fake(Ok(Some("2.2.0")), Some("2.1.0"))
        });
        assert_eq!(r.updates.state(), State::Ready);
        r.updates.download();
        assert_eq!(r.updates.state(), State::Downloading(0));
        r.pump();
        assert_eq!(r.backend.downloads.load(Ordering::SeqCst), 1);
        assert_eq!(r.updates.state(), State::Ready);
    }

    #[test]
    fn toast_download_finding_nothing_newer_keeps_the_pending_package() {
        let r = rig(Fake {
            has_latest: false,
            ..fake(Ok(None), Some("2.1.0"))
        });
        r.updates.download();
        assert_eq!(r.updates.state(), State::Downloading(0));
        r.pump();
        assert_eq!(r.backend.downloads.load(Ordering::SeqCst), 0);
        assert_eq!(r.updates.state(), State::Ready);
    }

    #[test]
    fn abandon_during_download_does_not_apply() {
        let r = rig(fake(Ok(Some("2.1.0")), None));
        r.updates.download();
        assert_eq!(r.updates.state(), State::Downloading(0));
        r.updates.abandon();
        r.pump();
        assert_eq!(r.updates.state(), State::Ready);
        assert_eq!(r.applied(), 0);
        assert_eq!(r.seen.borrow().last(), Some(&(State::Downloading(0), false)));
    }

    #[test]
    fn quiet_check_during_download_does_not_unabandon_it() {
        let r = rig(fake(Ok(Some("2.1.0")), None));
        r.updates.download();
        r.updates.on_msg(checked(Some("2.1.0"), None, true));
        assert_eq!(r.updates.state(), State::Downloading(0));
        r.updates.abandon();
        r.pump();
        assert_eq!(r.applied(), 0);
    }

    #[test]
    fn reopening_adopts_an_abandoned_download() {
        let r = rig(fake(Ok(Some("2.1.0")), None));
        r.updates.download();
        r.updates.abandon();
        r.updates.download();
        assert!(matches!(r.updates.state(), State::Downloading(_)));
        r.pump();
        assert_eq!(r.updates.state(), State::Ready);
        assert_eq!(r.backend.downloads.load(Ordering::SeqCst), 1);
    }
}
