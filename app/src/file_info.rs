//! Fields of the info pane (`Context.UpdateFileInfo` in 1.x): all blank for a pasted image or a file that vanished.

use std::path::Path;

use viewer_core::format::humanize_bytes;
use viewer_core::picture::Picture;

use crate::InfoFields;
use crate::win::locale::format_file_time;

pub fn describe(path: Option<&Path>, picture: Option<&Picture>) -> InfoFields {
    let (Some(path), Some(picture)) = (path, picture) else { return InfoFields::default() };
    // The file may have been deleted since it was loaded: the date and the size then stay blank.
    let meta = std::fs::metadata(path).ok();
    InfoFields {
        name: path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default().into(),
        folder: path.parent().map(|p| p.display().to_string()).unwrap_or_default().into(),
        date: meta.as_ref().and_then(|m| m.modified().ok()).map(format_file_time).unwrap_or_default().into(),
        size: meta.as_ref().map(|m| humanize_bytes(m.len() as f64)).unwrap_or_default().into(),
        dimensions: picture.dimensions_string().into(),
        depth: picture.depth_string().into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use viewer_core::picture;

    #[test]
    fn describes_a_file_with_a_non_ascii_folder_and_blanks_the_rest() {
        let dir = tempfile::tempdir().expect("tempdir");
        let folder = dir.path().join("Été 写真");
        std::fs::create_dir(&folder).expect("mkdir");
        let file = folder.join("a.png");
        image::RgbaImage::from_pixel(3, 2, image::Rgba([1, 2, 3, 255])).save_with_format(&file, image::ImageFormat::Png).expect("png");
        let pic = picture::load_path(&file).expect("load");
        let info = describe(Some(&file), Some(&pic));
        assert_eq!(info.name.as_str(), "a.png");
        assert_eq!(info.folder.as_str(), folder.display().to_string());
        assert_eq!(info.dimensions.as_str(), "3 x 2");
        assert!(info.size.ends_with(" B") && !info.date.is_empty() && info.depth.ends_with(" bit"));
        assert_eq!(describe(None, Some(&pic)).name.as_str(), "");
        assert_eq!(describe(Some(&file), None).folder.as_str(), "");
        std::fs::remove_file(&file).expect("rm");
        let gone = describe(Some(&file), Some(&pic));
        assert!(gone.date.is_empty() && gone.size.is_empty() && gone.name.as_str() == "a.png");
    }
}
