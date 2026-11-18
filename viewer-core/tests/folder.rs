use std::fs;
use viewer_core::folder::FolderNav;

fn touch_all(dir: &std::path::Path, names: &[&str]) {
    for n in names {
        fs::write(dir.join(n), b"x").unwrap();
    }
}

#[test]
fn listing_includes_uppercase_extensions() {
    let dir = tempfile::tempdir().unwrap();
    touch_all(dir.path(), &["img2.PNG", "notes.txt"]);
    fs::create_dir(dir.path().join("sub.png")).unwrap();
    let nav = FolderNav::list(&dir.path().join("img2.PNG")).unwrap();
    assert_eq!(nav.files, [dir.path().join("img2.PNG")]);
}

#[test]
fn listing_is_natural_sorted_and_finds_current() {
    let dir = tempfile::tempdir().unwrap();
    touch_all(dir.path(), &["img10.png", "img2.PNG", "notes.txt", "img1.jpg"]);
    let nav = FolderNav::list(&dir.path().join("IMG2.png")).unwrap();
    let got: Vec<_> = nav.files.iter().map(|p| p.file_name().unwrap().to_str().unwrap()).collect();
    assert_eq!(got, ["img1.jpg", "img2.PNG", "img10.png"]);
    assert_eq!(nav.index, 1);
}

#[test]
fn missing_current_gives_index_minus_one_and_missing_folder_gives_none() {
    let dir = tempfile::tempdir().unwrap();
    touch_all(dir.path(), &["a.png"]);
    assert_eq!(FolderNav::list(&dir.path().join("gone.png")).unwrap().index, -1);
    assert!(FolderNav::list(&dir.path().join("nodir").join("a.png")).is_none());
}
