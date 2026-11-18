#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod frame;

slint::include_modules!();

const PRODUCT_NAME: &str = "Image Viewer";

/// Same format as `MainWindow.UpdateTitle` in 1.x: "prefix - Image Viewer", or the bare product name.
fn window_title(prefix: Option<&str>) -> String {
    match prefix {
        Some(prefix) if !prefix.is_empty() => format!("{prefix} - {PRODUCT_NAME}"),
        _ => PRODUCT_NAME.to_string(),
    }
}

fn main() -> Result<(), slint::PlatformError> {
    slint::BackendSelector::new().backend_name("winit".into()).renderer_name("skia".into()).select()?;

    let ui = MainWindow::new()?;
    ui.set_title_text(window_title(None).into());
    frame::install_when_ready(&ui, |_| {});
    ui.run()
}

#[cfg(test)]
mod tests {
    use super::window_title;

    #[test]
    fn title_matches_update_title_format() {
        assert_eq!(window_title(None), "Image Viewer");
        assert_eq!(window_title(Some("")), "Image Viewer");
        assert_eq!(window_title(Some("photo.jpg")), "photo.jpg - Image Viewer");
    }
}
