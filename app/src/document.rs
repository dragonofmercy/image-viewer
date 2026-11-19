//! The open document (`Context.CurrentImage`, `CurrentFilePath`, `FolderFiles` in 1.x) and the worker that decodes it.
//!
//! `Document` lives on the UI thread. Each load gets a generation number: a result whose generation is not the current one is dropped,
//! so a slow decode never replaces a newer image (or a newer failure).

use crate::ui_state::Inputs;
use image::RgbaImage;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::time::{Duration, Instant};
use viewer_core::dib;
use viewer_core::files::{SAVE_FILE_TYPES, extension_of, normalize_extension};
use viewer_core::folder::FolderNav;
use viewer_core::picture::{self, LoadError, Picture, SaveError};
use viewer_core::settings::Settings;

pub enum Source {
    Path(PathBuf),
    #[allow(dead_code)] // only the worker test builds it; paste hands over the raw DIB so parsing stays off the UI thread
    Pixels(RgbaImage),
    /// A CF_DIB payload read from the clipboard (untrusted bytes).
    Dib(Vec<u8>),
}

#[derive(Default)]
pub struct Document {
    pub picture: Option<Picture>,
    pub path: Option<PathBuf>,
    pub folder: Option<FolderNav>,
    pub generation: u64,
    pub loading: bool,
    /// A save runs: the worker owns the picture until it hands it back, so `picture` is None meanwhile.
    pub saving: bool,
    /// File line of the error panel after a failed load (1.x shows the full path; empty for pasted content).
    pub error: Option<String>,
}

impl Document {
    /// Starts a new load: the previous picture is dropped (as `OpenImage` disposes it) and the error panel is cleared.
    pub fn begin_load(&mut self) -> u64 {
        self.generation += 1;
        self.loading = true;
        self.picture = None;
        self.error = None;
        self.generation
    }

    /// `LoadImageFromString`: None when the file does not exist; otherwise records the absolute path and starts a load.
    pub fn begin_path(&mut self, path: &Path) -> Option<(u64, PathBuf)> {
        if !path.is_file() {
            return None;
        }
        let path = std::path::absolute(path).ok()?;
        self.path = Some(path.clone());
        Some((self.begin_load(), path))
    }

    /// Applies a load result. Returns false, leaving everything untouched, when `generation` is not the current load.
    pub fn finish_load(&mut self, generation: u64, result: Result<Picture, LoadError>) -> bool {
        if generation != self.generation {
            return false;
        }
        self.loading = false;
        match result {
            Ok(picture) => self.picture = Some(picture),
            Err(e) => {
                eprintln!("cannot load the image: {e}");
                self.picture = None;
                self.error = Some(self.path.as_ref().map(|p| p.display().to_string()).unwrap_or_default());
            }
        }
        true
    }

    /// `LoadDirectoryFiles`: an unreadable folder disables navigation and keeps the image.
    pub fn list_folder(&mut self) {
        self.folder = self.path.as_deref().and_then(FolderNav::list);
    }

    pub fn inputs(&self) -> Inputs {
        let picture = self.picture.as_ref();
        Inputs {
            loaded: picture.is_some(),
            loading: self.loading,
            saving: self.saving,
            modified: picture.is_some_and(Picture::modified),
            has_path: self.path.is_some(),
            folder_count: self.folder.as_ref().map_or(0, |f| f.files.len()),
            has_icon_sizes: picture.is_some_and(Picture::has_icon_sizes),
            file_name: self.path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()),
            ..Inputs::default()
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Route {
    Nothing,
    /// Overwrite the file in place, with its normalized save type.
    Direct(String),
    SaveAs,
}

/// `Context.Save`: unmodified does nothing; pasted content and formats that cannot be written back (svg, ico) go through Save As.
pub fn save_route(modified: bool, path: Option<&Path>) -> Route {
    if !modified {
        return Route::Nothing;
    }
    let Some(path) = path else { return Route::SaveAs };
    let ext = normalize_extension(&extension_of(path).unwrap_or_default());
    if SAVE_FILE_TYPES.contains(&ext.as_str()) { Route::Direct(ext) } else { Route::SaveAs }
}

/// `SaveService.QualityForType`: JPEG and WebP use their configured quality, other formats ignore it.
pub fn quality_for(ext: &str, settings: &Settings) -> Option<u8> {
    match ext {
        ".jpg" => Some(settings.jpeg_quality()),
        ".webp" => Some(settings.webp_quality()),
        _ => None,
    }
}

/// `StepIconSize`: moves by `delta` in the size strip, wrapping around both ends.
pub fn next_icon_index(current: usize, delta: i32, count: usize) -> usize {
    if count == 0 {
        return 0;
    }
    (current as i64 + i64::from(delta)).rem_euclid(count as i64) as usize
}

/// Scroll offset (zero or negative) of the icon strip list that keeps item `index` fully visible, moving as little as possible.
/// `current` is the present offset, `pitch` the width of one item slot, `viewport` the visible width of the list.
pub fn reveal_offset(current: f32, index: usize, count: usize, pitch: f32, viewport: f32) -> f32 {
    if viewport <= 0.0 || count == 0 {
        return current.min(0.0);
    }
    let lowest = (viewport - count as f32 * pitch).min(0.0);
    let index = index.min(count - 1) as f32;
    // The item is fully visible for offsets in [left, right].
    let left = -index * pitch;
    let right = viewport - (index + 1.0) * pitch;
    // An item wider than the viewport shows its start.
    current.min(right).max(left).clamp(lowest, 0.0)
}

/// Frame delay of an animation: a delay of 0 plays at 100 ms.
pub fn frame_delay(delay_ms: u32) -> std::time::Duration {
    std::time::Duration::from_millis(if delay_ms == 0 { 100 } else { u64::from(delay_ms) })
}

/// Receives the generation, the result and the time spent decoding.
type Done = Box<dyn FnOnce(u64, Result<Picture, LoadError>, Duration) + Send>;

struct Job {
    source: Source,
    generation: u64,
    done: Done,
}

#[derive(Default)]
struct Slot {
    job: Option<Job>,
    closed: bool,
}

type Shared = Arc<(Mutex<Slot>, Condvar)>;

fn decode(source: Source) -> Result<Picture, LoadError> {
    match source {
        Source::Path(path) => picture::load_path(&path),
        Source::Pixels(pixels) => Ok(picture::load_from_rgba(pixels)),
        Source::Dib(bytes) => {
            let parsed = dib::parse_dib(&bytes);
            // The payload can be 100+ MB: released before the picture is built.
            drop(bytes);
            parsed.map(picture::load_from_rgba).map_err(LoadError::Decode)
        }
    }
}

/// Waits for the next job; None once the loader is dropped.
fn next_job(shared: &Shared) -> Option<Job> {
    let (lock, ready) = &**shared;
    let mut slot = lock.lock().ok()?;
    loop {
        if slot.closed {
            return None;
        }
        if let Some(job) = slot.job.take() {
            return Some(job);
        }
        slot = ready.wait(slot).ok()?;
    }
}

/// Decodes on one long-lived worker thread. Only the latest request waits: a newer one replaces it, so holding an arrow key
/// never queues decodes. A decode already running finishes; its result is then dropped by the generation check.
pub struct Loader {
    shared: Shared,
}

impl Loader {
    pub fn new() -> Self {
        Self::with_decoder(decode)
    }

    fn with_decoder(decode: impl Fn(Source) -> Result<Picture, LoadError> + Send + 'static) -> Self {
        let shared: Shared = Arc::default();
        let worker = shared.clone();
        let spawned = std::thread::Builder::new().name("decode".into()).spawn(move || {
            while let Some(job) = next_job(&worker) {
                let started = Instant::now();
                // A decoder panic must not kill the worker: every later load would then hang.
                let result = panic::catch_unwind(AssertUnwindSafe(|| decode(job.source))).unwrap_or_else(|_| Err(LoadError::Decode("decoder panicked".into())));
                let elapsed = started.elapsed();
                let (generation, done) = (job.generation, job.done);
                // Fails only once the event loop is gone: nothing left to show the result in.
                let _ = slint::invoke_from_event_loop(move || done(generation, result, elapsed));
            }
        });
        if let Err(e) = spawned {
            eprintln!("cannot start the decode thread: {e}");
        }
        Self { shared }
    }

    pub fn request(&self, source: Source, generation: u64, done: impl FnOnce(u64, Result<Picture, LoadError>, Duration) + Send + 'static) {
        let (lock, ready) = &*self.shared;
        if let Ok(mut slot) = lock.lock() {
            slot.job = Some(Job { source, generation, done: Box::new(done) });
            ready.notify_one();
        }
    }
}

impl Drop for Loader {
    fn drop(&mut self) {
        let (lock, ready) = &*self.shared;
        if let Ok(mut slot) = lock.lock() {
            slot.closed = true;
            ready.notify_one();
        }
    }
}

/// Receives the picture back with the save result.
type SaveDone = Box<dyn FnOnce(Picture, Result<(), SaveError>) + Send>;
/// Delivers a finished save to the UI thread.
type Post = Box<dyn Fn(Box<dyn FnOnce() + Send>) + Send>;

struct SaveJob {
    picture: Picture,
    path: PathBuf,
    ext: String,
    quality: Option<u8>,
    done: SaveDone,
}

/// Encodes on its own long-lived thread, separate from the `Loader` so a save is never replaced by a newer load request. The picture
/// is moved to the worker and back (no pixel copy): `Picture::save` needs `&mut` to clear the modified flag.
pub struct Saver {
    jobs: mpsc::Sender<SaveJob>,
}

impl Saver {
    pub fn new() -> Self {
        Self::with_poster(|f| {
            // Fails only once the event loop is gone: nothing left to show the result in.
            let _ = slint::invoke_from_event_loop(f);
        })
    }

    fn with_poster(post: impl Fn(Box<dyn FnOnce() + Send>) + Send + 'static) -> Self {
        let (jobs, queue) = mpsc::channel::<SaveJob>();
        let post: Post = Box::new(post);
        let spawned = std::thread::Builder::new().name("save".into()).spawn(move || {
            for job in queue {
                let SaveJob { mut picture, path, ext, quality, done } = job;
                // The picture is only borrowed by the closure: an encoder panic still hands it back, edit included.
                let result = panic::catch_unwind(AssertUnwindSafe(|| picture.save(&path, &ext, quality))).unwrap_or_else(|_| Err(SaveError::Encode("encoder panicked".into())));
                post(Box::new(move || done(picture, result)));
            }
        });
        if let Err(e) = spawned {
            eprintln!("cannot start the save thread: {e}");
        }
        Self { jobs }
    }

    /// Queues a save. Gives the picture back when the worker is not running.
    pub fn request(&self, picture: Picture, path: PathBuf, ext: String, quality: Option<u8>, done: impl FnOnce(Picture, Result<(), SaveError>) + Send + 'static) -> Result<(), Picture> {
        let job = SaveJob {
            picture,
            path,
            ext,
            quality,
            done: Box::new(done),
        };
        self.jobs.send(job).map_err(|e| e.0.picture)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use viewer_core::picture::{Flip, Rotate};

    fn png(path: &Path) {
        RgbaImage::from_pixel(3, 2, image::Rgba([1, 2, 3, 255])).save_with_format(path, image::ImageFormat::Png).expect("write png");
    }

    #[test]
    fn icon_index_wraps_both_ways() {
        assert_eq!(next_icon_index(0, -1, 3), 2);
        assert_eq!(next_icon_index(2, 1, 3), 0);
        assert_eq!(next_icon_index(1, 1, 3), 2);
        assert_eq!(next_icon_index(1, -1, 3), 0);
        assert_eq!(next_icon_index(0, 1, 0), 0);
    }

    // Reviewer cases: viewport 200, pitch 56, 10 items (content 560, lowest offset -360).
    #[test]
    fn reveal_first_item_stays_at_zero() {
        assert_eq!(reveal_offset(0.0, 0, 10, 56.0, 200.0), 0.0);
    }

    #[test]
    fn reveal_last_item_scrolls_to_the_end() {
        assert_eq!(reveal_offset(0.0, 9, 10, 56.0, 200.0), -360.0);
    }

    #[test]
    fn reveal_item_hidden_on_the_left_moves_just_enough() {
        // Item 3 spans 168..224; offset -190 hides its first 22px.
        assert_eq!(reveal_offset(-190.0, 3, 10, 56.0, 200.0), -168.0);
    }

    #[test]
    fn reveal_item_hidden_on_the_right_moves_just_enough() {
        // Item 5 spans 280..336; at offset 0 only 0..200 is visible.
        assert_eq!(reveal_offset(0.0, 5, 10, 56.0, 200.0), -136.0);
    }

    #[test]
    fn reveal_visible_item_keeps_the_offset() {
        assert_eq!(reveal_offset(-100.0, 3, 10, 56.0, 200.0), -100.0);
    }

    #[test]
    fn reveal_content_narrower_than_viewport_is_zero() {
        assert_eq!(reveal_offset(0.0, 2, 3, 56.0, 200.0), 0.0);
        assert_eq!(reveal_offset(-50.0, 0, 3, 56.0, 200.0), 0.0);
    }

    #[test]
    fn reveal_wraps_around_both_ways() {
        assert_eq!(reveal_offset(-360.0, 0, 10, 56.0, 200.0), 0.0);
        assert_eq!(reveal_offset(0.0, 9, 10, 56.0, 200.0), -360.0);
    }

    #[test]
    fn reveal_without_viewport_or_items_only_clamps() {
        assert_eq!(reveal_offset(-30.0, 4, 10, 56.0, 0.0), -30.0);
        assert_eq!(reveal_offset(25.0, 4, 10, 56.0, -5.0), 0.0);
        assert_eq!(reveal_offset(-30.0, 0, 0, 56.0, 200.0), -30.0);
        assert_eq!(reveal_offset(12.0, 0, 0, 56.0, 200.0), 0.0);
    }

    #[test]
    fn zero_delay_plays_at_100_ms() {
        assert_eq!(frame_delay(0).as_millis(), 100);
        assert_eq!(frame_delay(40).as_millis(), 40);
    }

    #[test]
    fn worker_keeps_only_the_latest_request() {
        use std::sync::mpsc;
        let (started_tx, started) = mpsc::channel::<u32>();
        let (open_gate, gate) = mpsc::channel::<()>();
        // The image width is the request id; request 1 blocks until the gate opens.
        let loader = Loader::with_decoder(move |source| {
            let Source::Pixels(pixels) = source else { return Err(LoadError::Decode("unexpected".into())) };
            let _ = started_tx.send(pixels.width());
            if pixels.width() == 1 {
                let _ = gate.recv();
            }
            Ok(picture::load_from_rgba(pixels))
        });
        let request = |id: u32| loader.request(Source::Pixels(RgbaImage::new(id, 1)), u64::from(id), |_, _, _| {});
        let wait = Duration::from_secs(5);
        request(1);
        assert_eq!(started.recv_timeout(wait).ok(), Some(1));
        request(2);
        request(3);
        request(4);
        let _ = open_gate.send(());
        assert_eq!(started.recv_timeout(wait).ok(), Some(4));
        assert!(started.recv_timeout(Duration::from_millis(200)).is_err());
    }

    #[test]
    fn quality_only_for_jpeg_and_webp() {
        // A key that does not exist: the getters return their defaults, nothing is written.
        let settings = Settings::at(r"SOFTWARE\Dragon Industries\Image Viewer Tests\app-quality-missing");
        assert_eq!(quality_for(".jpg", &settings), Some(settings.jpeg_quality()));
        assert_eq!(quality_for(".webp", &settings), Some(settings.webp_quality()));
        for ext in [".png", ".bmp", ".gif", ".tiff", ".tga", ".jxl"] {
            assert_eq!(quality_for(ext, &settings), None, "{ext}");
        }
    }

    #[test]
    fn save_routing_follows_context_save() {
        let dir = Path::new(r"C:\Pictures");
        assert_eq!(save_route(false, Some(&dir.join("a.png"))), Route::Nothing);
        assert_eq!(save_route(false, None), Route::Nothing);
        assert_eq!(save_route(true, None), Route::SaveAs);
        assert_eq!(save_route(true, Some(&dir.join("a.svg"))), Route::SaveAs);
        assert_eq!(save_route(true, Some(&dir.join("a.ico"))), Route::SaveAs);
        assert_eq!(save_route(true, Some(&dir.join("noext"))), Route::SaveAs);
        assert_eq!(save_route(true, Some(&dir.join("photo.JPEG"))), Route::Direct(".jpg".into()));
        assert_eq!(save_route(true, Some(&dir.join("scan.TIF"))), Route::Direct(".tiff".into()));
        assert_eq!(save_route(true, Some(&dir.join("Été \u{5199}.webp"))), Route::Direct(".webp".into()));
    }

    type Saved = (Picture, Result<(), SaveError>);

    /// Runs the done callback on the worker itself (no event loop in tests) and waits for it.
    fn save_with_worker(pic: Picture, path: PathBuf) -> Saved {
        let saver = Saver::with_poster(|f| f());
        let (tx, rx) = std::sync::mpsc::channel::<Saved>();
        assert!(saver.request(pic, path, ".png".into(), None, move |p, r| tx.send((p, r)).expect("send")).is_ok());
        rx.recv_timeout(Duration::from_secs(10)).expect("save finished")
    }

    #[test]
    fn saver_writes_and_hands_the_picture_back() {
        let dir = tempfile::tempdir().expect("tempdir");
        let target = dir.path().join("Été \u{5199}.PNG");
        let mut pic = picture::load_from_rgba(RgbaImage::from_pixel(3, 2, image::Rgba([1, 2, 3, 255])));
        pic.rotate_flip(Rotate::Rotate90, Flip::None);
        let (pic, result) = save_with_worker(pic, target.clone());
        assert!(result.is_ok(), "{result:?}");
        assert!(!pic.modified());
        assert_eq!((pic.width(), pic.height()), (2, 3));
        let saved = picture::load_path(&target).expect("saved file");
        assert_eq!((saved.width(), saved.height()), (2, 3));
    }

    #[test]
    fn failed_save_keeps_the_picture_and_its_edit() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut pic = picture::load_from_rgba(RgbaImage::new(3, 2));
        pic.rotate_flip(Rotate::Rotate90, Flip::None);
        let (pic, result) = save_with_worker(pic, dir.path().join("missing").join("a.png"));
        assert!(result.is_err());
        assert!(pic.modified());
    }

    #[test]
    fn pasted_dib_decodes_to_a_modified_picture() {
        let bgra = [1, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255, 10, 11, 12, 255, 13, 14, 15, 255, 16, 17, 18, 255];
        let pic = decode(Source::Dib(dib::build_dib(&bgra, 3, 2))).expect("valid DIB");
        assert!(pic.modified());
        assert_eq!((pic.width(), pic.height()), (3, 2));
        assert_eq!(pic.current().get_pixel(0, 0).0, [3, 2, 1, 255]);
        assert!(decode(Source::Dib(vec![0; 8])).is_err());
    }

    #[test]
    fn picture_crosses_threads() {
        fn send<T: Send>() {}
        send::<Picture>();
        send::<LoadError>();
    }

    #[test]
    fn stale_result_is_ignored() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (a, b) = (dir.path().join("a.png"), dir.path().join("b.png"));
        png(&a);
        png(&b);
        let mut doc = Document::default();
        let (old, _) = doc.begin_path(&a).expect("a exists");
        let (new, _) = doc.begin_path(&b).expect("b exists");
        assert!(doc.finish_load(new, picture::load_path(&b)));
        assert!(!doc.finish_load(old, Err(LoadError::Decode("late".into()))));
        assert!(doc.picture.is_some() && !doc.loading && doc.error.is_none());
        assert_eq!(doc.path.as_deref(), Some(b.as_path()));
        // A stale success after a newer failure must not resurrect a picture either.
        let (newest, _) = doc.begin_path(&a).expect("a exists");
        assert!(doc.finish_load(newest, Err(LoadError::Decode("bad".into()))));
        assert!(!doc.finish_load(new, picture::load_path(&b)));
        assert!(doc.picture.is_none() && !doc.loading);
        assert_eq!(doc.path.as_deref(), Some(a.as_path()));
        assert_eq!(doc.error.as_deref(), Some(a.display().to_string().as_str()));
    }

    #[test]
    fn failed_load_clears_picture_and_keeps_folder() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (good, bad) = (dir.path().join("good.png"), dir.path().join("bad.png"));
        png(&good);
        std::fs::write(&bad, b"not a png").expect("write");
        let mut doc = Document::default();
        let (g, _) = doc.begin_path(&good).expect("exists");
        doc.list_folder();
        assert!(doc.finish_load(g, picture::load_path(&good)));
        let (g, path) = doc.begin_path(&bad).expect("exists");
        assert!(doc.finish_load(g, picture::load_path(&path)));
        assert!(doc.picture.is_none() && !doc.loading);
        assert_eq!(doc.error.as_deref(), Some(bad.display().to_string().as_str()));
        assert_eq!(doc.folder.as_ref().map(|f| f.files.len()), Some(2));
        let inputs = doc.inputs();
        assert!(!inputs.loaded && inputs.has_path && inputs.folder_count == 2);
    }

    #[test]
    fn missing_file_does_not_start_a_load() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut doc = Document::default();
        assert!(doc.begin_path(&dir.path().join("gone.png")).is_none());
        assert_eq!(doc.generation, 0);
        assert!(doc.path.is_none() && !doc.loading);
    }

    #[test]
    fn non_ascii_folder_and_upper_case_extension() {
        let dir = tempfile::tempdir().expect("tempdir");
        let folder = dir.path().join("Été 2024 \u{1F4F7} 写真");
        std::fs::create_dir(&folder).expect("mkdir");
        png(&folder.join("a.png"));
        let photo = folder.join("photo.PNG");
        png(&photo);
        let mut doc = Document::default();
        let (g, path) = doc.begin_path(&photo).expect("photo exists");
        assert_eq!(path, photo);
        doc.list_folder();
        let folder_nav = doc.folder.as_ref().expect("folder listed");
        assert_eq!(folder_nav.files.len(), 2);
        assert_eq!(folder_nav.index, 1);
        assert!(doc.finish_load(g, picture::load_path(&path)));
        assert_eq!(doc.inputs().file_name.as_deref(), Some("photo.PNG"));
    }
}
