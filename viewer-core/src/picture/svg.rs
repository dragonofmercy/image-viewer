use std::path::Path;
use std::sync::{Arc, OnceLock};

use image::RgbaImage;
use resvg::{tiny_skia, usvg};

use super::LoadError;

const SVG_MAX_RASTER_SIZE: f32 = 1024.0;

fn system_fonts() -> Arc<usvg::fontdb::Database> {
    static FONTS: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut db = usvg::fontdb::Database::new();
            db.load_system_fonts();
            Arc::new(db)
        })
        .clone()
}

/// Rasterizes an SVG, scaled down to fit SVG_MAX_RASTER_SIZE on both edges and never up.
pub(crate) fn rasterize_svg(path: &Path) -> Result<RgbaImage, LoadError> {
    let data = std::fs::read(path)?;
    let options = usvg::Options {
        resources_dir: path.parent().map(Path::to_path_buf),
        fontdb: system_fonts(),
        ..Default::default()
    };
    let tree = usvg::Tree::from_data(&data, &options).map_err(|e| LoadError::Decode(e.to_string()))?;

    let size = tree.size();
    let (w, h) = (size.width(), size.height());
    if w <= 0.0 || h <= 0.0 {
        return Err(LoadError::Decode("SVG has no drawable area".into()));
    }

    let scale = (SVG_MAX_RASTER_SIZE / w).min(SVG_MAX_RASTER_SIZE / h).min(1.0);
    let (width, height) = (((w * scale) as u32).max(1), ((h * scale) as u32).max(1));
    let mut pixmap = tiny_skia::Pixmap::new(width, height).ok_or_else(|| LoadError::Decode("cannot allocate SVG raster".into()))?;
    resvg::render(&tree, tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());

    let mut bytes = Vec::with_capacity(pixmap.data().len());
    bytes.extend(pixmap.pixels().iter().flat_map(|p| {
        let c = p.demultiply();
        [c.red(), c.green(), c.blue(), c.alpha()]
    }));
    RgbaImage::from_raw(width, height, bytes).ok_or_else(|| LoadError::Decode("SVG raster size mismatch".into()))
}
