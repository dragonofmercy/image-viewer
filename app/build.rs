fn main() {
    slint_build::compile("ui/main.slint").expect("Slint compilation failed");

    // FileVersion and ProductVersion (string and numeric) default to CARGO_PKG_VERSION.
    let mut res = winresource::WindowsResource::new();
    res.set_icon("assets/ImageViewer.ico")
        .set_manifest_file("app.manifest")
        .set("ProductName", "Image Viewer")
        .set("CompanyName", "Dragon Industries")
        .set("FileDescription", "Image Viewer");
    res.compile().expect("Windows resource compilation failed");
}
