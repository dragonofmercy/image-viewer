//! HEIC / HEIF / AVIF through the bundled libheif.dll (1.20.2), bound at runtime.
//! Signatures and enum values come from the libheif 1.20.2 headers (heif_error.h, heif_image.h,
//! heif_context.h, heif_decoding.h, heif_encoding.h, heif_image_handle.h).

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::path::Path;
use std::ptr::{null, null_mut};
use std::sync::OnceLock;

use image::RgbaImage;
use libloading::Library;

use super::LoadError;

const COLORSPACE_RGB: c_int = 1;
const CHROMA_INTERLEAVED_RGB: c_int = 10;
const CHROMA_INTERLEAVED_RGBA: c_int = 11;
const CHANNEL_INTERLEAVED: c_int = 10;
const COMPRESSION_AV1: c_int = 4;

/// `struct heif_error`, returned by value: two C enums (int-sized on MSVC) and a message.
#[repr(C)]
struct HeifError {
    code: c_int,
    subcode: c_int,
    message: *const c_char,
}

type Ptr = *mut c_void;

struct Api {
    context_alloc: unsafe extern "C" fn() -> Ptr,
    context_free: unsafe extern "C" fn(Ptr),
    context_read_from_memory_without_copy: unsafe extern "C" fn(Ptr, *const c_void, usize, *const c_void) -> HeifError,
    context_get_primary_image_handle: unsafe extern "C" fn(Ptr, *mut Ptr) -> HeifError,
    image_handle_has_alpha_channel: unsafe extern "C" fn(Ptr) -> c_int,
    image_handle_release: unsafe extern "C" fn(Ptr),
    decode_image: unsafe extern "C" fn(Ptr, *mut Ptr, c_int, c_int, *const c_void) -> HeifError,
    image_get_width: unsafe extern "C" fn(Ptr, c_int) -> c_int,
    image_get_height: unsafe extern "C" fn(Ptr, c_int) -> c_int,
    image_get_plane_readonly2: unsafe extern "C" fn(Ptr, c_int, *mut usize) -> *const u8,
    image_release: unsafe extern "C" fn(Ptr),
    image_create: unsafe extern "C" fn(c_int, c_int, c_int, c_int, *mut Ptr) -> HeifError,
    image_add_plane: unsafe extern "C" fn(Ptr, c_int, c_int, c_int, c_int) -> HeifError,
    image_get_plane2: unsafe extern "C" fn(Ptr, c_int, *mut usize) -> *mut u8,
    context_get_encoder_for_format: unsafe extern "C" fn(Ptr, c_int, *mut Ptr) -> HeifError,
    encoder_set_lossy_quality: unsafe extern "C" fn(Ptr, c_int) -> HeifError,
    encoder_release: unsafe extern "C" fn(Ptr),
    context_encode_image: unsafe extern "C" fn(Ptr, Ptr, Ptr, *const c_void, *mut Ptr) -> HeifError,
    context_write_to_file: unsafe extern "C" fn(Ptr, *const c_char) -> HeifError,
    // Keeps the function pointers above valid; never unloaded since Api lives in a static.
    _lib: Library,
}

fn api() -> Result<&'static Api, LoadError> {
    static API: OnceLock<Result<Api, String>> = OnceLock::new();
    API.get_or_init(load_api).as_ref().map_err(|e| LoadError::Decode(e.clone()))
}

fn load_api() -> Result<Api, String> {
    let path = std::env::current_exe().map_err(|e| e.to_string())?.with_file_name("libheif.dll");
    if !path.is_file() {
        return Err("libheif.dll not found".into());
    }
    // SAFETY: loading by absolute path the libheif build shipped with the app; its initializers are trusted.
    let lib = unsafe { Library::new(&path) }.map_err(|e| format!("libheif.dll: {e}"))?;
    macro_rules! sym {
        ($name:literal) => {
            // SAFETY: the field type this is assigned to matches the 1.20.2 header prototype of $name.
            *unsafe { lib.get(concat!($name, "\0").as_bytes()) }.map_err(|e| format!("libheif.dll: {}: {e}", $name))?
        };
    }
    Ok(Api {
        context_alloc: sym!("heif_context_alloc"),
        context_free: sym!("heif_context_free"),
        context_read_from_memory_without_copy: sym!("heif_context_read_from_memory_without_copy"),
        context_get_primary_image_handle: sym!("heif_context_get_primary_image_handle"),
        image_handle_has_alpha_channel: sym!("heif_image_handle_has_alpha_channel"),
        image_handle_release: sym!("heif_image_handle_release"),
        decode_image: sym!("heif_decode_image"),
        image_get_width: sym!("heif_image_get_width"),
        image_get_height: sym!("heif_image_get_height"),
        image_get_plane_readonly2: sym!("heif_image_get_plane_readonly2"),
        image_release: sym!("heif_image_release"),
        image_create: sym!("heif_image_create"),
        image_add_plane: sym!("heif_image_add_plane"),
        image_get_plane2: sym!("heif_image_get_plane2"),
        context_get_encoder_for_format: sym!("heif_context_get_encoder_for_format"),
        encoder_set_lossy_quality: sym!("heif_encoder_set_lossy_quality"),
        encoder_release: sym!("heif_encoder_release"),
        context_encode_image: sym!("heif_context_encode_image"),
        context_write_to_file: sym!("heif_context_write_to_file"),
        _lib: lib,
    })
}

fn check(err: HeifError) -> Result<(), String> {
    if err.code == 0 {
        return Ok(());
    }
    if err.message.is_null() {
        return Err(format!("libheif error {}.{}", err.code, err.subcode));
    }
    // SAFETY: non-null libheif messages are NUL-terminated and live at least until the next call on the context; copied now.
    Err(unsafe { CStr::from_ptr(err.message) }.to_string_lossy().into_owned())
}

/// Owns one libheif object and releases it on drop.
/// Declare guards parent first: locals drop in reverse order, so children are released before their context.
struct Guard {
    ptr: Ptr,
    release: unsafe extern "C" fn(Ptr),
}

impl Guard {
    fn new(ptr: Ptr, release: unsafe extern "C" fn(Ptr), what: &str) -> Result<Self, String> {
        if ptr.is_null() {
            return Err(format!("libheif returned no {what}"));
        }
        Ok(Guard { ptr, release })
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        // SAFETY: ptr is a live, non-null object from libheif, released exactly once with its matching function.
        unsafe { (self.release)(self.ptr) }
    }
}

/// Decodes the primary image, with libheif's default transformations (rotation, mirror, crop) applied.
/// Returns the pixels and whether the image carries an alpha channel.
pub(crate) fn decode_heif(bytes: &[u8]) -> Result<(RgbaImage, bool), LoadError> {
    let api = api()?;
    decode_with(api, bytes).map_err(LoadError::Decode)
}

fn decode_with(api: &Api, bytes: &[u8]) -> Result<(RgbaImage, bool), String> {
    // SAFETY (whole block): every call matches its 1.20.2 prototype; every out pointer is checked for null and
    // wrapped in a Guard before use; `bytes` is borrowed for the whole call and `ctx` never escapes it, so the buffer
    // outlives the context, as the without_copy API requires.
    unsafe {
        let ctx = Guard::new((api.context_alloc)(), api.context_free, "context")?;
        check((api.context_read_from_memory_without_copy)(ctx.ptr, bytes.as_ptr().cast(), bytes.len(), null()))?;

        let mut handle = null_mut();
        check((api.context_get_primary_image_handle)(ctx.ptr, &mut handle))?;
        let handle = Guard::new(handle, api.image_handle_release, "image handle")?;

        let alpha = (api.image_handle_has_alpha_channel)(handle.ptr) != 0;
        let chroma = if alpha { CHROMA_INTERLEAVED_RGBA } else { CHROMA_INTERLEAVED_RGB };
        let mut img = null_mut();
        check((api.decode_image)(handle.ptr, &mut img, COLORSPACE_RGB, chroma, null()))?;
        let img = Guard::new(img, api.image_release, "decoded image")?;

        // The decoded plane's size, not the handle's: it is post-rotation.
        let w = usize::try_from((api.image_get_width)(img.ptr, CHANNEL_INTERLEAVED)).unwrap_or(0);
        let h = usize::try_from((api.image_get_height)(img.ptr, CHANNEL_INTERLEAVED)).unwrap_or(0);
        let mut stride = 0usize;
        let data = (api.image_get_plane_readonly2)(img.ptr, CHANNEL_INTERLEAVED, &mut stride);
        let bpp = if alpha { 4 } else { 3 };
        let row = w.checked_mul(bpp).ok_or("image too large")?;
        if data.is_null() || w == 0 || h == 0 || stride < row {
            return Err(format!("invalid decoded plane ({w} x {h}, stride {stride})"));
        }
        let len = stride.checked_mul(h - 1).and_then(|n| n.checked_add(row)).ok_or("image too large")?;
        // SAFETY: libheif allocates the plane as at least stride * height bytes, and len <= that; img is alive.
        let plane = std::slice::from_raw_parts(data, len);

        let mut out = Vec::with_capacity(w * h * 4);
        for y in 0..h {
            let src = &plane[y * stride..y * stride + row];
            if alpha {
                out.extend_from_slice(src);
            } else {
                src.as_chunks::<3>().0.iter().for_each(|p| out.extend_from_slice(&[p[0], p[1], p[2], 255]));
            }
        }
        let (w, h) = (u32::try_from(w).map_err(|e| e.to_string())?, u32::try_from(h).map_err(|e| e.to_string())?);
        let pixels = RgbaImage::from_raw(w, h, out).ok_or("pixel buffer size mismatch")?;
        Ok((pixels, alpha))
    }
}

/// Test aid: writes `rgba` as an AV1-coded HEIF (quality 90), with an alpha plane when any pixel is not opaque.
/// libheif's file API takes an ANSI path, so `path` must be ASCII.
#[doc(hidden)]
pub fn encode_av1_heif(rgba: &RgbaImage, path: &Path) -> Result<(), String> {
    let api = api().map_err(|e| e.to_string())?;
    let path = path.to_str().filter(|s| s.is_ascii()).ok_or("path must be ASCII")?;
    let path = CString::new(path).map_err(|e| e.to_string())?;
    let alpha = rgba.pixels().any(|p| p[3] != 255);
    let (chroma, bpp) = if alpha { (CHROMA_INTERLEAVED_RGBA, 4) } else { (CHROMA_INTERLEAVED_RGB, 3) };
    let (w, h) = rgba.dimensions();
    let (cw, ch) = (c_int::try_from(w).map_err(|e| e.to_string())?, c_int::try_from(h).map_err(|e| e.to_string())?);
    let row = w as usize * bpp;

    // SAFETY (whole block): every call matches its 1.20.2 prototype; out pointers are null-checked and guarded;
    // each row write stays within the plane (stride >= row, height rows of stride bytes).
    unsafe {
        let ctx = Guard::new((api.context_alloc)(), api.context_free, "context")?;
        let mut img = null_mut();
        check((api.image_create)(cw, ch, COLORSPACE_RGB, chroma, &mut img))?;
        let img = Guard::new(img, api.image_release, "image")?;
        check((api.image_add_plane)(img.ptr, CHANNEL_INTERLEAVED, cw, ch, 8))?;
        let mut stride = 0usize;
        let data = (api.image_get_plane2)(img.ptr, CHANNEL_INTERLEAVED, &mut stride);
        if data.is_null() || stride < row {
            return Err("invalid image plane".into());
        }
        for (y, src) in rgba.rows().enumerate() {
            let dst = std::slice::from_raw_parts_mut(data.add(y * stride), row);
            for (d, s) in dst.chunks_exact_mut(bpp).zip(src) {
                d.copy_from_slice(&s.0[..bpp]);
            }
        }

        let mut enc = null_mut();
        check((api.context_get_encoder_for_format)(ctx.ptr, COMPRESSION_AV1, &mut enc))?;
        let enc = Guard::new(enc, api.encoder_release, "AV1 encoder")?;
        check((api.encoder_set_lossy_quality)(enc.ptr, 90))?;
        check((api.context_encode_image)(ctx.ptr, img.ptr, enc.ptr, null(), null_mut()))?;
        check((api.context_write_to_file)(ctx.ptr, path.as_ptr()))
    }
}
