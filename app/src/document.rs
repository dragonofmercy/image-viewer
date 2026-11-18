//! The open document (`Context.CurrentImage`, `CurrentFilePath`, `FolderFiles` in 1.x) and the worker that decodes it.
//!
//! `Document` lives on the UI thread. Each load gets a generation number: a result whose generation is not the current one is dropped,
//! so a slow decode never replaces a newer image (or a newer failure).

use crate::ui_state::Inputs;
use image::RgbaImage;
use std::path::{Path, PathBuf};
use viewer_core::folder::FolderNav;
use viewer_core::picture::{self, LoadError, Picture};

pub enum Source {
    Path(PathBuf),
    #[allow(dead_code)] // first caller: the clipboard task
    Pixels(RgbaImage),
}

#[derive(Default)]
pub struct Document {
    pub picture: Option<Picture>,
    pub path: Option<PathBuf>,
    pub folder: Option<FolderNav>,
    pub generation: u64,
    pub loading: bool,
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
            modified: picture.is_some_and(Picture::modified),
            has_path: self.path.is_some(),
            folder_count: self.folder.as_ref().map_or(0, |f| f.files.len()),
            has_icon_sizes: picture.is_some_and(Picture::has_icon_sizes),
            file_name: self.path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()),
            ..Inputs::default()
        }
    }
}

/// `StepIconSize`: moves by `delta` in the size strip, wrapping around both ends.
pub fn next_icon_index(current: usize, delta: i32, count: usize) -> usize {
    if count == 0 {
        return 0;
    }
    (current as i64 + i64::from(delta)).rem_euclid(count as i64) as usize
}

/// Frame delay of an animation: a delay of 0 plays at 100 ms.
pub fn frame_delay(delay_ms: u32) -> std::time::Duration {
    std::time::Duration::from_millis(if delay_ms == 0 { 100 } else { u64::from(delay_ms) })
}

/// Decodes on a worker thread, one thread per request.
pub struct Loader;

impl Loader {
    pub fn request(&self, source: Source, generation: u64, done: impl FnOnce(u64, Result<Picture, LoadError>) + Send + 'static) {
        std::thread::spawn(move || {
            let result = match source {
                Source::Path(path) => picture::load_path(&path),
                Source::Pixels(pixels) => Ok(picture::load_from_rgba(pixels)),
            };
            // Fails only once the event loop is gone: nothing left to show the result in.
            let _ = slint::invoke_from_event_loop(move || done(generation, result));
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn zero_delay_plays_at_100_ms() {
        assert_eq!(frame_delay(0).as_millis(), 100);
        assert_eq!(frame_delay(40).as_millis(), 40);
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
