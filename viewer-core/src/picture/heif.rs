//! HEIC / HEIF / AVIF through the bundled libheif.dll (1.20.2), bound at runtime.
//! Signatures and enum values come from the libheif 1.20.2 headers (heif_error.h, heif_image.h,
//! heif_context.h, heif_decoding.h, heif_encoding.h, heif_image_handle.h, heif_metadata.h, heif_color.h).

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
/// `enum heif_color_profile_type` values are big-endian fourccs (`heif_fourcc`).
const PROFILE_PROF: u32 = u32::from_be_bytes(*b"prof");
const PROFILE_RICC: u32 = u32::from_be_bytes(*b"rICC");
/// Sanity cap on an EXIF block or ICC profile read from an untrusted file.
const MAX_METADATA_BYTES: usize = 16 * 1024 * 1024;

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
    image_handle_get_list_of_metadata_block_ids: unsafe extern "C" fn(Ptr, *const c_char, *mut u32, c_int) -> c_int,
    image_handle_get_metadata_size: unsafe extern "C" fn(Ptr, u32) -> usize,
    image_handle_get_metadata: unsafe extern "C" fn(Ptr, u32, *mut c_void) -> HeifError,
    image_handle_get_color_profile_type: unsafe extern "C" fn(Ptr) -> u32,
    image_handle_get_raw_color_profile_size: unsafe extern "C" fn(Ptr) -> usize,
    image_handle_get_raw_color_profile: unsafe extern "C" fn(Ptr, *mut c_void) -> HeifError,
    context_add_exif_metadata: unsafe extern "C" fn(Ptr, Ptr, *const c_void, c_int) -> HeifError,
    image_set_raw_color_profile: unsafe extern "C" fn(Ptr, *const c_char, *const c_void, usize) -> HeifError,
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
    // Loads the codec plugins and builds libheif's internal tables; ref-counted, never paired with heif_deinit since the library stays loaded.
    let init: unsafe extern "C" fn(*const c_void) -> HeifError = sym!("heif_init");
    // SAFETY: heif_init(struct heif_init_params*) accepts null for the defaults; this runs once, inside the OnceLock initializer.
    check(unsafe { init(null()) }).map_err(|e| format!("libheif.dll: heif_init: {e}"))?;
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
        image_handle_get_list_of_metadata_block_ids: sym!("heif_image_handle_get_list_of_metadata_block_IDs"),
        image_handle_get_metadata_size: sym!("heif_image_handle_get_metadata_size"),
        image_handle_get_metadata: sym!("heif_image_handle_get_metadata"),
        image_handle_get_color_profile_type: sym!("heif_image_handle_get_color_profile_type"),
        image_handle_get_raw_color_profile_size: sym!("heif_image_handle_get_raw_color_profile_size"),
        image_handle_get_raw_color_profile: sym!("heif_image_handle_get_raw_color_profile"),
        context_add_exif_metadata: sym!("heif_context_add_exif_metadata"),
        image_set_raw_color_profile: sym!("heif_image_set_raw_color_profile"),
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

/// A decoded primary image and the metadata carried to save.
pub(crate) struct HeifDecoded {
    pub pixels: RgbaImage,
    pub has_alpha: bool,
    pub exif: Option<Vec<u8>>,
    pub icc: Option<Vec<u8>>,
}

/// Decodes the primary image, with libheif's default transformations (rotation, mirror, crop) applied.
pub(crate) fn decode_heif(bytes: &[u8]) -> Result<HeifDecoded, LoadError> {
    let api = api()?;
    decode_with(api, bytes).map_err(LoadError::Decode)
}

fn decode_with(api: &Api, bytes: &[u8]) -> Result<HeifDecoded, String> {
    // SAFETY (whole block): every call matches its 1.20.2 prototype; every out pointer is checked for null and
    // wrapped in a Guard before use; `bytes` is borrowed for the whole call and `ctx` never escapes it, so the buffer
    // outlives the context, as the without_copy API requires.
    unsafe {
        let ctx = Guard::new((api.context_alloc)(), api.context_free, "context")?;
        check((api.context_read_from_memory_without_copy)(ctx.ptr, bytes.as_ptr().cast(), bytes.len(), null()))?;

        let mut handle = null_mut();
        check((api.context_get_primary_image_handle)(ctx.ptr, &mut handle))?;
        let handle = Guard::new(handle, api.image_handle_release, "image handle")?;
        // libheif applies irot / imir itself, so the EXIF orientation is reset to 1 like the native loader does.
        let exif = read_exif(api, handle.ptr).and_then(super::metadata::reset_orientation);
        let icc = read_icc(api, handle.ptr).and_then(super::metadata::rgb_icc);

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
        if (w as u64).saturating_mul(h as u64).saturating_mul(4) > super::MAX_DECODE_BYTES {
            return Err("image too large".into());
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
        Ok(HeifDecoded { pixels, has_alpha: alpha, exif, icc })
    }
}

/// The TIFF structure of the first Exif block; `None` on any failure, the image still loads.
/// SAFETY: `handle` is a live image handle.
unsafe fn read_exif(api: &Api, handle: Ptr) -> Option<Vec<u8>> {
    let mut id = 0u32;
    // SAFETY: ids has room for the one id requested; the filter is a NUL-terminated C string.
    if unsafe { (api.image_handle_get_list_of_metadata_block_ids)(handle, c"Exif".as_ptr(), &mut id, 1) } != 1 {
        return None;
    }
    // SAFETY: id comes from the handle's own list.
    let size = unsafe { (api.image_handle_get_metadata_size)(handle, id) };
    if size == 0 || size > MAX_METADATA_BYTES {
        return None;
    }
    let mut block = vec![0u8; size];
    // SAFETY: libheif writes exactly get_metadata_size bytes, the buffer's length.
    check(unsafe { (api.image_handle_get_metadata)(handle, id, block.as_mut_ptr().cast()) }).ok()?;
    tiff_of_exif_block(&block).map(<[u8]>::to_vec)
}

/// The raw ICC profile when the handle carries one (`prof` or `rICC`; `nclx` has none); `None` on any failure.
/// SAFETY: `handle` is a live image handle.
unsafe fn read_icc(api: &Api, handle: Ptr) -> Option<Vec<u8>> {
    // SAFETY: plain queries on a live handle.
    let (kind, size) = unsafe { ((api.image_handle_get_color_profile_type)(handle), (api.image_handle_get_raw_color_profile_size)(handle)) };
    if !matches!(kind, PROFILE_PROF | PROFILE_RICC) || size == 0 || size > MAX_METADATA_BYTES {
        return None;
    }
    let mut icc = vec![0u8; size];
    // SAFETY: libheif writes exactly get_raw_color_profile_size bytes, the buffer's length.
    check(unsafe { (api.image_handle_get_raw_color_profile)(handle, icc.as_mut_ptr().cast()) }).ok()?;
    Some(icc)
}

/// Test aid: writes `rgba` as an AV1-coded HEIF (quality 90), with an alpha plane when any pixel is not opaque.
/// libheif's file API takes an ANSI path, so `path` must be ASCII.
#[doc(hidden)]
pub fn encode_av1_heif(rgba: &RgbaImage, path: &Path) -> Result<(), String> {
    encode_av1_heif_with_metadata(rgba, path, None, None)
}

/// Test aid: `encode_av1_heif` plus an Exif block (`exif` is the TIFF structure, libheif prepends the offset) and a `prof` ICC profile.
#[doc(hidden)]
pub fn encode_av1_heif_with_metadata(rgba: &RgbaImage, path: &Path, exif: Option<&[u8]>, icc: Option<&[u8]>) -> Result<(), String> {
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
        if let Some(icc) = icc {
            check((api.image_set_raw_color_profile)(img.ptr, c"prof".as_ptr(), icc.as_ptr().cast(), icc.len()))?;
        }
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
        let mut handle = null_mut();
        check((api.context_encode_image)(ctx.ptr, img.ptr, enc.ptr, null(), &mut handle))?;
        let handle = Guard::new(handle, api.image_handle_release, "encoded image handle")?;
        if let Some(exif) = exif {
            let len = c_int::try_from(exif.len()).map_err(|e| e.to_string())?;
            check((api.context_add_exif_metadata)(ctx.ptr, handle.ptr, exif.as_ptr().cast(), len))?;
        }
        check((api.context_write_to_file)(ctx.ptr, path.as_ptr()))
    }
}

/// The TIFF structure of a HEIF Exif item: a 4-byte big-endian offset to the TIFF header, then the data. `None` when out of bounds.
fn tiff_of_exif_block(block: &[u8]) -> Option<&[u8]> {
    let offset = usize::try_from(u32::from_be_bytes(block.get(..4)?.try_into().ok()?)).ok()?;
    block.get(4usize.checked_add(offset)?..).filter(|t| !t.is_empty())
}

#[cfg(test)]
mod tests {
    use super::tiff_of_exif_block;

    #[test]
    fn exif_block_skips_its_tiff_offset() {
        let tiff = b"II*\0\x08\0\0\0";
        let zero = [&[0, 0, 0, 0][..], tiff].concat();
        assert_eq!(tiff_of_exif_block(&zero), Some(&tiff[..]));
        let two = [&[0, 0, 0, 2, 9, 9][..], tiff].concat();
        assert_eq!(tiff_of_exif_block(&two), Some(&tiff[..]));
        assert_eq!(tiff_of_exif_block(&[0, 0, 0]), None);
        assert_eq!(tiff_of_exif_block(&[0, 0, 0, 9, 1, 2]), None);
    }
}
