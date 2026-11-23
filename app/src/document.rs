//! The open document (`Context.CurrentImage`, `CurrentFilePath`, `FolderFiles` in 1.x) and the worker that decodes it.
//!
//! `Document` lives on the UI thread. Each load gets a generation number: a result whose generation is not the current one is dropped,
//! so a slow decode never replaces a newer image (or a newer failure).

use crate::crop::{self, Handle, Sel};
use crate::ui_state::Inputs;
#[cfg(test)]
use image::RgbaImage;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime};
use viewer_core::dib;
use viewer_core::files::{SAVE_FILE_TYPES, extension_of, normalize_extension};
use viewer_core::folder::FolderNav;
use viewer_core::picture::{self, LoadError, Picture, SaveError};
use viewer_core::settings::Settings;

pub enum Source {
    Path(PathBuf),
    #[cfg(test)] // only the worker test builds it
    Pixels(RgbaImage),
    /// A CF_DIBV5 or CF_DIB payload read from the clipboard (untrusted bytes).
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
    /// A paste is parsing: the picture and error panel it replaced, put back when the DIB fails to parse.
    paste_backup: Option<(Option<Picture>, Option<String>)>,
    /// The inline cropper is open. A new load or paste closes it, as `OpenImage` closes the 1.x cropper.
    pub crop: Option<CropSession>,
    /// Message key of an error raised while another dialog was open: shown when that dialog closes.
    pending_error: Option<String>,
}

/// State of the open cropper, in image pixels.
#[derive(Debug, Clone, Copy)]
pub struct CropSession {
    pub sel: Sel,
    /// Entry of `crop::RATIOS`.
    pub ratio: usize,
    /// Selection and handle when the current drag began: drag deltas are measured from it.
    pub drag: (Sel, Handle),
}

impl CropSession {
    pub fn new(image: (u32, u32)) -> Self {
        let sel = crop::full(image);
        Self { sel, ratio: 0, drag: (sel, Handle::Move) }
    }
}

impl Document {
    /// Starts a new load: the previous picture is dropped (as `OpenImage` disposes it) and the error panel is cleared.
    pub fn begin_load(&mut self) -> u64 {
        self.generation += 1;
        self.loading = true;
        self.picture = None;
        self.error = None;
        self.paste_backup = None;
        self.crop = None;
        self.generation
    }

    /// Starts a paste like a load (commands off, ring shown), keeping the current picture and error aside for `cancel_paste`.
    pub fn begin_paste(&mut self) -> u64 {
        let backup = (self.picture.take(), self.error.take());
        let generation = self.begin_load();
        self.paste_backup = Some(backup);
        generation
    }

    /// The pasted DIB did not parse: the previous picture and error come back untouched. False when a newer load took over.
    pub fn cancel_paste(&mut self, generation: u64) -> bool {
        if generation != self.generation {
            return false;
        }
        let Some((picture, error)) = self.paste_backup.take() else { return false };
        self.loading = false;
        self.picture = picture;
        self.error = error;
        true
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
        // Not loading: this load was already finished (by a preloaded picture, with the same generation).
        if generation != self.generation || !self.loading {
            return false;
        }
        self.loading = false;
        self.paste_backup = None;
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

    /// Keeps the first error raised behind an open dialog; a later one does not replace it.
    pub fn defer_error(&mut self, key: &str) {
        self.pending_error.get_or_insert_with(|| key.to_owned());
    }

    pub fn take_error(&mut self) -> Option<String> {
        self.pending_error.take()
    }

    /// Save As needs a picture in hand: none while a save runs (the worker owns it) or nothing is open.
    pub fn can_save_as(&self) -> bool {
        !self.saving && self.picture.is_some()
    }

    /// Delete also applies to pasted content (no path: nothing is recycled, the view clears), but never during a load or a save.
    pub fn can_delete(&self) -> bool {
        !self.saving && !self.loading && self.picture.is_some()
    }

    /// The worker handed the picture back. A successful save adopts `adopt` (the Save As target) and lists its folder.
    pub fn finish_save(&mut self, picture: Picture, ok: bool, adopt: Option<PathBuf>) {
        self.picture = Some(picture);
        self.saving = false;
        if ok && let Some(path) = adopt {
            self.path = Some(path);
            self.list_folder();
        }
    }

    /// After the current file was recycled: no current path, and the folder list loses it. `next` then lands on its successor.
    pub fn remove_deleted(&mut self) {
        self.path = None;
        if let Some(folder) = self.folder.as_mut() {
            folder.remove_current();
        }
    }

    pub fn inputs(&self) -> Inputs {
        let picture = self.picture.as_ref();
        Inputs {
            loaded: picture.is_some(),
            loading: self.loading,
            saving: self.saving,
            cropping: self.crop.is_some(),
            modified: picture.is_some_and(Picture::modified),
            has_path: self.path.is_some(),
            folder_count: self.folder.as_ref().map_or(0, |f| f.files.len()),
            has_icon_sizes: picture.is_some_and(Picture::has_icon_sizes),
            file_name: self.path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()),
        }
    }
}

/// Size and modification time of a file: a cached picture is used only while its file still has the stamp it was decoded under.
pub type Stamp = (u64, SystemTime);

pub fn stamp(path: &Path) -> Option<Stamp> {
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.len(), meta.modified().ok()?))
}

/// Decoded pictures kept for instant navigation: the one just left and the preloaded neighbor. Bounded in count and in size,
/// since each entry is a full decoded image (96 MiB for 24 Mpx).
#[derive(Default)]
pub struct PictureCache {
    entries: Vec<(PathBuf, Stamp, Picture)>,
}

impl PictureCache {
    const ENTRIES: usize = 2;
    const ENTRY_BYTES: usize = 256 * 1024 * 1024;

    fn picture_bytes(picture: &Picture) -> usize {
        picture.frames().iter().map(|f| f.pixels.as_raw().len()).sum()
    }

    /// Whether `put` would keep this picture: a neighbor preload is pointless next to one the cache refuses.
    pub fn fits(picture: &Picture) -> bool {
        Self::picture_bytes(picture) <= Self::ENTRY_BYTES
    }

    /// Keeps `picture`, dropping the oldest entry when full. A picture over the size limit is not kept.
    pub fn put(&mut self, path: PathBuf, stamp: Stamp, picture: Picture) {
        if !Self::fits(&picture) {
            return;
        }
        self.entries.retain(|e| e.0 != path);
        if self.entries.len() >= Self::ENTRIES {
            self.entries.remove(0);
        }
        self.entries.push((path, stamp, picture));
    }

    pub fn contains(&self, path: &Path) -> bool {
        self.entries.iter().any(|e| e.0 == path)
    }

    /// Takes the picture of `path` first, then keeps `left`: storing first could evict the very entry about to be taken.
    pub fn swap(&mut self, left: Option<(PathBuf, Stamp, Picture)>, path: &Path, on_disk: Option<Stamp>) -> Option<(Picture, Stamp)> {
        let Some((p, stamp, picture)) = left else { return self.take(path, on_disk) };
        // Reloading the picture being left (one-image folder, re-open of the current file): it is the target itself.
        if p == path {
            self.entries.retain(|e| e.0 != p);
            return (Some(stamp) == on_disk).then_some((picture, stamp));
        }
        let taken = self.take(path, on_disk);
        self.put(p, stamp, picture);
        taken
    }

    /// Removes and returns the picture of `path` when the file still has the stamp `current`; a stale entry is dropped.
    pub fn take(&mut self, path: &Path, current: Option<Stamp>) -> Option<(Picture, Stamp)> {
        let index = self.entries.iter().position(|e| e.0 == path)?;
        let (_, stamp, picture) = self.entries.remove(index);
        (Some(stamp) == current).then_some((picture, stamp))
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

fn decode(source: Source) -> Result<Picture, LoadError> {
    match source {
        Source::Path(path) => picture::load_path(&path),
        #[cfg(test)]
        Source::Pixels(pixels) => Ok(picture::load_from_rgba(pixels)),
        Source::Dib(bytes) => {
            let parsed = dib::parse_dib(&bytes);
            // The payload can be 100+ MB: released before the picture is built.
            drop(bytes);
            parsed.map(picture::load_from_rgba).map_err(LoadError::Decode)
        }
    }
}

/// Decodes on one long-lived worker thread. Only the latest request waits: a newer one replaces it, so holding an arrow key
/// never queues decodes. A decode already running finishes; its result is then dropped by the generation check.
pub struct Loader {
    jobs: mpsc::Sender<Job>,
}

impl Loader {
    pub fn new() -> Self {
        Self::with_decoder(decode)
    }

    fn with_decoder(decode: impl Fn(Source) -> Result<Picture, LoadError> + Send + 'static) -> Self {
        let (jobs, queue) = mpsc::channel::<Job>();
        let spawned = std::thread::Builder::new().name("decode".into()).spawn(move || {
            // Ends when the loader, and with it the sender, is dropped.
            while let Ok(job) = queue.recv() {
                // Requests that queued up meanwhile: only the latest one is decoded.
                let job = queue.try_iter().last().unwrap_or(job);
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
        Self { jobs }
    }

    pub fn request(&self, source: Source, generation: u64, done: impl FnOnce(u64, Result<Picture, LoadError>, Duration) + Send + 'static) {
        // Fails only when the worker could not start: the load then never completes, as before.
        let _ = self.jobs.send(Job { source, generation, done: Box::new(done) });
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

    fn pic() -> Picture {
        picture::load_from_rgba(RgbaImage::new(1, 1))
    }

    fn stamp(n: u64) -> Stamp {
        (n, SystemTime::UNIX_EPOCH)
    }

    #[test]
    fn cache_keeps_two_fresh_pictures() {
        let (a, b, c) = (PathBuf::from("a"), PathBuf::from("b"), PathBuf::from("c"));
        let mut cache = PictureCache::default();
        cache.put(a.clone(), stamp(1), pic());
        cache.put(b.clone(), stamp(2), pic());
        cache.put(c.clone(), stamp(3), pic());
        assert!(!cache.contains(&a) && cache.contains(&b) && cache.contains(&c));
        // The file changed since: the entry is dropped, not returned.
        assert!(cache.take(&b, Some(stamp(9))).is_none());
        assert!(!cache.contains(&b));
        assert!(cache.take(&c, Some(stamp(3))).is_some());
        assert!(cache.take(&c, Some(stamp(3))).is_none());
    }

    #[test]
    fn swap_returns_the_target_even_when_full() {
        let (a, b, c) = (PathBuf::from("a"), PathBuf::from("b"), PathBuf::from("c"));
        let mut cache = PictureCache::default();
        cache.put(a.clone(), stamp(1), pic());
        cache.put(c.clone(), stamp(3), pic());
        let got = cache.swap(Some((b.clone(), stamp(2), pic())), &a, Some(stamp(1)));
        assert_eq!(got.map(|g| g.1), Some(stamp(1)));
        assert!(!cache.contains(&a) && cache.contains(&b) && cache.contains(&c));
    }

    #[test]
    fn swap_drops_a_stale_target_and_still_keeps_left() {
        let (a, b) = (PathBuf::from("a"), PathBuf::from("b"));
        let mut cache = PictureCache::default();
        cache.put(a.clone(), stamp(1), pic());
        assert!(cache.swap(Some((b.clone(), stamp(2), pic())), &a, Some(stamp(9))).is_none());
        assert!(!cache.contains(&a) && cache.contains(&b));
    }

    #[test]
    fn swap_returns_the_picture_being_left_when_it_is_the_target() {
        let a = PathBuf::from("a");
        let mut cache = PictureCache::default();
        assert_eq!(cache.swap(Some((a.clone(), stamp(1), pic())), &a, Some(stamp(1))).map(|g| g.1), Some(stamp(1)));
        assert!(!cache.contains(&a));
        // The file changed on disk: it must be decoded again.
        assert!(cache.swap(Some((a.clone(), stamp(1), pic())), &a, Some(stamp(2))).is_none());
        assert!(!cache.contains(&a));
    }

    #[test]
    fn fits_follows_the_entry_limit() {
        assert!(PictureCache::fits(&pic()));
        assert_eq!(PictureCache::picture_bytes(&picture::load_from_rgba(RgbaImage::new(3, 2))), 24);
    }

    #[test]
    fn a_load_finishes_only_once() {
        let mut doc = Document::default();
        let g = doc.begin_load();
        assert!(doc.finish_load(g, Ok(picture::load_from_rgba(RgbaImage::new(1, 1)))));
        assert!(!doc.finish_load(g, Err(LoadError::Decode("duplicate".into()))));
        assert!(doc.picture.is_some() && doc.error.is_none());
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
    fn failed_paste_restores_the_previous_state() {
        let dir = tempfile::tempdir().expect("tempdir");
        let a = dir.path().join("a.png");
        png(&a);
        let mut doc = Document::default();
        let (g, _) = doc.begin_path(&a).expect("a exists");
        assert!(doc.finish_load(g, picture::load_path(&a)));
        let paste = doc.begin_paste();
        // Busy like a load: no picture, so every image command is off.
        let inputs = doc.inputs();
        assert!(inputs.loading && !inputs.loaded && inputs.has_path);
        assert!(doc.cancel_paste(paste));
        assert!(doc.picture.is_some() && !doc.loading && doc.error.is_none());
        assert!(!doc.cancel_paste(paste), "only once");
        // An error panel comes back too.
        let mut doc = Document { error: Some("x".into()), ..Document::default() };
        let paste = doc.begin_paste();
        assert!(doc.error.is_none());
        assert!(doc.cancel_paste(paste));
        assert_eq!(doc.error.as_deref(), Some("x"));
    }

    #[test]
    fn a_new_load_closes_the_cropper() {
        let mut doc = Document {
            crop: Some(CropSession::new((4, 3))),
            ..Document::default()
        };
        assert!(doc.inputs().cropping);
        doc.begin_paste();
        assert!(doc.crop.is_none() && !doc.inputs().cropping);
    }

    #[test]
    fn load_during_a_paste_drops_the_backup() {
        let dir = tempfile::tempdir().expect("tempdir");
        let a = dir.path().join("a.png");
        png(&a);
        let mut doc = Document::default();
        let paste = doc.begin_paste();
        let (g, _) = doc.begin_path(&a).expect("a exists");
        assert!(!doc.cancel_paste(paste));
        assert!(doc.loading);
        assert!(doc.finish_load(g, picture::load_path(&a)));
        assert!(!doc.cancel_paste(g), "no backup left");
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

    #[test]
    fn a_deferred_error_is_taken_once_and_the_first_one_wins() {
        let mut doc = Document::default();
        assert_eq!(doc.take_error(), None);
        doc.defer_error("A");
        doc.defer_error("B");
        assert_eq!(doc.take_error().as_deref(), Some("A"));
        assert_eq!(doc.take_error(), None);
    }

    #[test]
    fn save_as_and_delete_refuse_while_saving_or_without_a_picture() {
        let mut doc = Document::default();
        assert!(!doc.can_save_as() && !doc.can_delete());
        doc.picture = Some(picture::load_from_rgba(RgbaImage::new(2, 2)));
        // Pasted content: no path, still deletable (the view clears).
        assert!(doc.can_save_as() && doc.can_delete());
        doc.loading = true;
        assert!(doc.can_save_as() && !doc.can_delete());
        doc.loading = false;
        doc.saving = true;
        assert!(!doc.can_save_as() && !doc.can_delete());
    }

    fn folder_doc(dir: &Path, names: &[&str], current: usize) -> Document {
        for n in names {
            png(&dir.join(n));
        }
        let mut doc = Document {
            path: Some(dir.join(names[current])),
            ..Document::default()
        };
        doc.list_folder();
        doc
    }

    #[test]
    fn delete_in_the_middle_shows_the_next_entry() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut doc = folder_doc(dir.path(), &["a.png", "b.png", "c.png"], 1);
        doc.remove_deleted();
        assert!(doc.path.is_none());
        let next = doc.folder.as_mut().and_then(|f| f.next(|_| true));
        assert_eq!(next, Some(dir.path().join("c.png")));
    }

    #[test]
    fn delete_of_the_last_entry_wraps_to_the_first() {
        // FolderNav wraps around, as 1.x does: the previous entry is not chosen.
        let dir = tempfile::tempdir().expect("tempdir");
        let mut doc = folder_doc(dir.path(), &["a.png", "b.png", "c.png"], 2);
        doc.remove_deleted();
        let next = doc.folder.as_mut().and_then(|f| f.next(|_| true));
        assert_eq!(next, Some(dir.path().join("a.png")));
    }

    #[test]
    fn delete_of_the_only_entry_leaves_no_picture() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut doc = folder_doc(dir.path(), &["a.png"], 0);
        doc.remove_deleted();
        assert!(doc.path.is_none());
        assert_eq!(doc.folder.as_mut().and_then(|f| f.next(|_| true)), None);
    }

    #[test]
    fn a_failed_save_keeps_the_edit_and_the_old_path() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut doc = folder_doc(dir.path(), &["a.png"], 0);
        let old = doc.path.clone();
        let mut pic = picture::load_from_rgba(RgbaImage::new(3, 2));
        pic.rotate_flip(Rotate::Rotate90, Flip::None);
        doc.saving = true;
        doc.finish_save(pic, false, Some(dir.path().join("b.png")));
        assert!(!doc.saving && doc.picture.as_ref().is_some_and(Picture::modified));
        assert_eq!(doc.path, old);
    }

    #[test]
    fn a_successful_save_as_adopts_the_new_path_and_clears_the_edit() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut doc = folder_doc(dir.path(), &["a.png"], 0);
        let target = dir.path().join("b.png");
        let mut pic = picture::load_from_rgba(RgbaImage::new(3, 2));
        pic.rotate_flip(Rotate::Rotate90, Flip::None);
        doc.saving = true;
        let (pic, result) = save_with_worker(pic, target.clone());
        assert!(result.is_ok());
        doc.finish_save(pic, true, Some(target.clone()));
        assert!(!doc.saving && doc.picture.as_ref().is_some_and(|p| !p.modified()));
        assert_eq!(doc.path.as_deref(), Some(target.as_path()));
        assert_eq!(doc.folder.as_ref().map(|f| (f.files.len(), f.index)), Some((2, 1)));
    }
}
