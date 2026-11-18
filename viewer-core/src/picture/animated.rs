use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use image::codecs::gif::GifDecoder;
use image::codecs::webp::WebPDecoder;
use image::{AnimationDecoder, DynamicImage, ImageDecoder, Limits};

use super::{Frame, LoadError, Picture};

/// Frames shorter than this (or zero) are shown for `DEFAULT_DELAY_MS`, as browsers do.
const MIN_DELAY_MS: u32 = 20;
const DEFAULT_DELAY_MS: u32 = 100;

/// Decodes every composited frame of a GIF or animated WebP. `None` means a still WebP: the caller decodes it normally.
pub(super) fn load_animated(path: &Path, gif: bool) -> Result<Option<Picture>, LoadError> {
    let input = BufReader::new(File::open(path)?);
    if gif {
        collect(GifDecoder::new(input)?).map(Some)
    } else {
        let decoder = WebPDecoder::new(input)?;
        if decoder.has_animation() { collect(decoder).map(Some) } else { Ok(None) }
    }
}

fn collect<'a, D: ImageDecoder + AnimationDecoder<'a>>(mut decoder: D) -> Result<Picture, LoadError> {
    decoder.set_limits(Limits::no_limits())?;
    let orientation = decoder.orientation().unwrap_or(image::metadata::Orientation::NoTransforms);
    let bits = decoder.color_type().bits_per_pixel();
    let mut frames = Vec::new();
    for frame in decoder.into_frames() {
        // A stream cut mid-frame still shows the frames decoded before the cut; only an empty result is an error.
        let frame = match frame {
            Ok(f) => f,
            Err(e) if frames.is_empty() => return Err(e.into()),
            Err(_) => break,
        };
        let (num, den) = frame.delay().numer_denom_ms();
        let ms = (f64::from(num) / f64::from(den.max(1))).round() as u32;
        let mut img = DynamicImage::ImageRgba8(frame.into_buffer());
        img.apply_orientation(orientation);
        frames.push(Frame {
            pixels: img.into_rgba8(),
            delay_ms: if ms < MIN_DELAY_MS { DEFAULT_DELAY_MS } else { ms },
        });
    }
    if frames.is_empty() {
        return Err(LoadError::Decode("no frames".to_string()));
    }
    Ok(Picture {
        frames,
        icon_frames: Vec::new(),
        icon_index: 0,
        bits_per_pixel: bits,
        modified: false,
    })
}
