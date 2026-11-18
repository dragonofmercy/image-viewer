use crate::files::{is_supported, natural_cmp};
use std::path::{Path, PathBuf};

/// Images of one folder plus the current position (index -1 = current file not in the list).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderNav {
    pub files: Vec<PathBuf>,
    pub index: isize,
}

impl FolderNav {
    /// Supported files of the current file's folder (top level only), natural-sorted; None when the folder is unreadable.
    pub fn list(current: &Path) -> Option<FolderNav> {
        let dir = current.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
        // file_type() comes with the directory listing on Windows; only a symlink needs a stat to know what it points at.
        let is_file = |e: &std::fs::DirEntry| e.file_type().is_ok_and(|t| t.is_file() || (t.is_symlink() && e.path().is_file()));
        let mut files: Vec<PathBuf> = std::fs::read_dir(dir).ok()?.filter_map(|e| e.ok()).filter(|e| is_file(e)).map(|e| e.path()).filter(|p| is_supported(p)).collect();
        files.sort_by(|a, b| natural_cmp(&a.to_string_lossy(), &b.to_string_lossy()));
        let cur = current.to_string_lossy().to_lowercase();
        let index = files.iter().position(|p| p.to_string_lossy().to_lowercase() == cur).map_or(-1, |i| i as isize);
        Some(FolderNav { files, index })
    }

    /// Step forward with wrap-around; unopenable files are dropped (the removal shifts the next file into place, so step back first).
    pub fn next(&mut self, mut can_open: impl FnMut(&Path) -> bool) -> Option<PathBuf> {
        while !self.files.is_empty() {
            self.index += 1;
            if self.index >= self.files.len() as isize {
                self.index = 0;
            }
            if can_open(&self.files[self.index as usize]) {
                return Some(self.files[self.index as usize].clone());
            }
            self.files.remove(self.index as usize);
            self.index -= 1;
        }
        None
    }

    /// Step backward with wrap-around; unopenable files are dropped (the index already points before the removed slot).
    pub fn prev(&mut self, mut can_open: impl FnMut(&Path) -> bool) -> Option<PathBuf> {
        while !self.files.is_empty() {
            self.index -= 1;
            if self.index < 0 {
                self.index = self.files.len() as isize - 1;
            }
            if can_open(&self.files[self.index as usize]) {
                return Some(self.files[self.index as usize].clone());
            }
            self.files.remove(self.index as usize);
        }
        None
    }

    pub fn first(&mut self, can_open: impl FnMut(&Path) -> bool) -> Option<PathBuf> {
        self.index = -1;
        self.next(can_open)
    }

    pub fn last(&mut self, can_open: impl FnMut(&Path) -> bool) -> Option<PathBuf> {
        self.index = 0;
        self.prev(can_open)
    }

    /// After the current file was deleted: drop it and step back so `next` lands on the file that took its place.
    pub fn remove_current(&mut self) {
        if !(0..self.files.len() as isize).contains(&self.index) {
            return;
        }
        self.files.remove(self.index as usize);
        self.index -= 1;
    }
}

/// Return a copy of `v` without the element at `index`; an out-of-range index returns an unchanged copy.
pub fn remove_at_index<T: Clone>(v: &[T], index: isize) -> Vec<T> {
    let mut out = v.to_vec();
    if let Ok(i) = usize::try_from(index)
        && i < out.len()
    {
        out.remove(i);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_middle_first_and_last() {
        let v = ["a", "b", "c"];
        assert_eq!(remove_at_index(&v, 1), ["a", "c"]);
        assert_eq!(remove_at_index(&v, 0), ["b", "c"]);
        assert_eq!(remove_at_index(&v, 2), ["a", "b"]);
    }

    #[test]
    fn out_of_range_is_unchanged() {
        let v = ["a", "b", "c"];
        assert_eq!(remove_at_index(&v, -1), v);
        assert_eq!(remove_at_index(&v, 3), v);
    }

    fn nav(names: &[&str], index: isize) -> FolderNav {
        FolderNav {
            files: names.iter().map(PathBuf::from).collect(),
            index,
        }
    }
    fn ok(_: &Path) -> bool {
        true
    }
    fn names(n: &FolderNav) -> Vec<&str> {
        n.files.iter().map(|p| p.to_str().unwrap()).collect()
    }

    #[test]
    fn next_walks_and_wraps() {
        let mut n = nav(&["a", "b", "c"], 0);
        assert_eq!(n.next(ok), Some("b".into()));
        assert_eq!(n.next(ok), Some("c".into()));
        assert_eq!(n.next(ok), Some("a".into()));
        assert_eq!(n.index, 0);
    }

    #[test]
    fn prev_walks_and_wraps() {
        let mut n = nav(&["a", "b", "c"], 2);
        assert_eq!(n.prev(ok), Some("b".into()));
        assert_eq!(n.prev(ok), Some("a".into()));
        assert_eq!(n.prev(ok), Some("c".into()));
        assert_eq!(n.index, 2);
    }

    #[test]
    fn first_and_last_from_anywhere() {
        let mut n = nav(&["a", "b", "c"], 1);
        assert_eq!(n.first(ok), Some("a".into()));
        assert_eq!(n.index, 0);
        assert_eq!(n.last(ok), Some("c".into()));
        assert_eq!(n.index, 2);
        let mut m = nav(&["a", "b", "c"], -1);
        assert_eq!(m.next(ok), Some("a".into()));
    }

    #[test]
    fn next_skips_a_dead_file_and_drops_it() {
        let mut n = nav(&["a", "b", "c"], 0);
        assert_eq!(n.next(|p| p != Path::new("b")), Some("c".into()));
        assert_eq!(names(&n), ["a", "c"]);
        assert_eq!(n.index, 1);
    }

    #[test]
    fn next_dead_last_file_wraps_to_first() {
        let mut n = nav(&["a", "b", "c"], 1);
        assert_eq!(n.next(|p| p != Path::new("c")), Some("a".into()));
        assert_eq!(names(&n), ["a", "b"]);
        assert_eq!(n.index, 0);
    }

    #[test]
    fn prev_skips_a_dead_file() {
        let mut n = nav(&["a", "b", "c"], 2);
        assert_eq!(n.prev(|p| p != Path::new("b")), Some("a".into()));
        assert_eq!(names(&n), ["a", "c"]);
        assert_eq!(n.index, 0);
    }

    #[test]
    fn prev_dead_first_file_wraps_to_last() {
        let mut n = nav(&["a", "b", "c"], 1);
        assert_eq!(n.prev(|p| p != Path::new("a")), Some("c".into()));
        assert_eq!(names(&n), ["b", "c"]);
        assert_eq!(n.index, 1);
    }

    #[test]
    fn remove_current_then_next_lands_on_following_file() {
        let mut n = nav(&["a", "b", "c"], 1);
        n.remove_current();
        assert_eq!(names(&n), ["a", "c"]);
        assert_eq!(n.index, 0);
        assert_eq!(n.next(ok), Some("c".into()));
    }

    #[test]
    fn remove_last_then_next_wraps_to_first() {
        let mut n = nav(&["a", "b", "c"], 2);
        n.remove_current();
        assert_eq!(n.next(ok), Some("a".into()));
    }

    #[test]
    fn remove_current_out_of_range_is_a_no_op() {
        let mut n = nav(&["a"], -1);
        n.remove_current();
        assert_eq!(n, nav(&["a"], -1));
    }

    #[test]
    fn removing_the_only_file_leaves_nothing_to_open() {
        let mut n = nav(&["a"], 0);
        n.remove_current();
        assert_eq!(n.next(ok), None);
    }

    #[test]
    fn every_direction_when_every_file_fails_empties_list_and_stops() {
        for dir in 0..4 {
            let mut n = nav(&["a", "b", "c"], 1);
            let r = match dir {
                0 => n.next(|_| false),
                1 => n.prev(|_| false),
                2 => n.first(|_| false),
                _ => n.last(|_| false),
            };
            assert_eq!(r, None);
            assert!(n.files.is_empty());
        }
    }

    #[test]
    fn empty_list_returns_none() {
        let mut n = nav(&[], -1);
        assert_eq!(n.next(ok), None);
        assert_eq!(n.last(ok), None);
    }
}
