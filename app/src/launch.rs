//! Command line of the exe: a file to open, the update protocol URI (from the update toast), or nothing.
use std::ffi::OsString;
use std::path::PathBuf;

pub const PROTOCOL: &str = "dragon-imageviewer";
pub const UPDATE_URI: &str = "dragon-imageviewer:update";
pub const ABOUT_URI: &str = "dragon-imageviewer:about";

#[derive(Debug, PartialEq, Eq)]
pub enum Launch {
    File(PathBuf),
    Update,
    About,
    None,
}

/// `args` excludes the program name. Only the first argument counts.
pub fn parse_launch(args: &[OsString]) -> Launch {
    let Some(arg) = args.first() else { return Launch::None };
    let text = arg.to_string_lossy();
    // Relaunch by the Windows App SDK activator for a toast left by 1.x: nothing to open.
    if text.starts_with("----") || text.eq_ignore_ascii_case("-Embedding") {
        return Launch::None;
    }
    let scheme_len = PROTOCOL.len() + 1;
    if text.get(..scheme_len).is_some_and(|s| s.eq_ignore_ascii_case(&format!("{PROTOCOL}:"))) {
        // Anything else on our protocol is dropped, never opened as a file.
        let rest = &text[scheme_len..];
        return if rest == &UPDATE_URI[scheme_len..] {
            Launch::Update
        } else if rest == &ABOUT_URI[scheme_len..] {
            Launch::About
        } else {
            Launch::None
        };
    }
    Launch::File(PathBuf::from(arg))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_arguments() {
        let p = |a: &[&str]| parse_launch(&a.iter().map(OsString::from).collect::<Vec<_>>());
        assert_eq!(p(&[]), Launch::None);
        assert_eq!(p(&[r"C:\a b\photo.jpg"]), Launch::File(PathBuf::from(r"C:\a b\photo.jpg")));
        assert_eq!(p(&["dragon-imageviewer:update"]), Launch::Update);
        assert_eq!(p(&["DRAGON-IMAGEVIEWER:update"]), Launch::Update);
        assert_eq!(p(&["dragon-imageviewer:about"]), Launch::About);
        assert_eq!(p(&["DRAGON-IMAGEVIEWER:about"]), Launch::About);
        assert_eq!(p(&["dragon-imageviewer:about/"]), Launch::None);
        // Anything else on our protocol is dropped, never opened as a file.
        assert_eq!(p(&["dragon-imageviewer:update/../x"]), Launch::None);
        assert_eq!(p(&[r"dragon-imageviewer:C:\Windows\x.png"]), Launch::None);
        // A toast left by 1.x relaunches the exe through the Windows App SDK activator.
        assert_eq!(p(&["----AppNotificationActivated:", "-Embedding"]), Launch::None);
        assert_eq!(p(&["-Embedding"]), Launch::None);
    }
}
