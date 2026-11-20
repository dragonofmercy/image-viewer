//! JPEG XL: read with the pure-Rust jxl-oxide, write through the bundled libjxl (0.13.0), bound at runtime.
//! Encoder signatures, struct layouts and enum values come from the libjxl 0.13.0 headers (encode.h, codestream_header.h,
//! color_encoding.h, types.h, thread_parallel_runner.h).

use std::borrow::Cow;
use std::ffi::{c_int, c_void};
use std::path::Path;
use std::ptr::null;
use std::sync::OnceLock;

use image::{DynamicImage, ImageBuffer, RgbaImage};
use jxl_oxide::{AllocTracker, EnumColourEncoding, JxlImage, RenderingIntent};
use libloading::Library;

use super::LoadError;

/// First frame as 8-bit sRGB RGBA (orientation applied by jxl-oxide), plus whether it has alpha.
pub(crate) fn decode_jxl(bytes: &[u8]) -> Result<(RgbaImage, bool), LoadError> {
    let err = |e: Box<dyn std::error::Error + Send + Sync>| LoadError::Decode(e.to_string());
    // The tracker bounds jxl-oxide's internal buffers; the header check below gives the output buffer a clear error.
    let tracker = AllocTracker::with_limit(usize::try_from(super::MAX_DECODE_BYTES).unwrap_or(usize::MAX));
    let mut image = JxlImage::builder().alloc_tracker(tracker).read(bytes).map_err(err)?;
    super::reserve(&mut 0, super::rgba_bytes(image.width(), image.height()))?;
    if image.num_loaded_keyframes() == 0 {
        return Err(LoadError::Decode("JPEG XL: no complete frame".into()));
    }
    image.request_color_encoding(EnumColourEncoding::srgb(RenderingIntent::Relative));
    let render = image.render_frame(0).map_err(err)?;
    let mut stream = render.stream();
    let (w, h, channels) = (stream.width(), stream.height(), stream.channels());
    let mut buf = vec![0u8; w as usize * h as usize * channels as usize];
    stream.write_to_buffer(&mut buf);
    let img = match channels {
        1 => ImageBuffer::from_raw(w, h, buf).map(DynamicImage::ImageLuma8),
        2 => ImageBuffer::from_raw(w, h, buf).map(DynamicImage::ImageLumaA8),
        3 => ImageBuffer::from_raw(w, h, buf).map(DynamicImage::ImageRgb8),
        4 => ImageBuffer::from_raw(w, h, buf).map(DynamicImage::ImageRgba8),
        _ => None,
    };
    let img = img.ok_or_else(|| LoadError::Decode(format!("JPEG XL: unsupported channel count {channels}")))?;
    Ok((img.into_rgba8(), channels % 2 == 0))
}

const ENC_SUCCESS: c_int = 0;
const ENC_NEED_MORE_OUTPUT: c_int = 2;
const TYPE_UINT8: c_int = 2;
const NATIVE_ENDIAN: c_int = 0;

/// `JxlBasicInfo` (JXL_BOOL and enums are int-sized).
#[repr(C)]
struct BasicInfo {
    have_container: c_int,
    xsize: u32,
    ysize: u32,
    bits_per_sample: u32,
    exponent_bits_per_sample: u32,
    intensity_target: f32,
    min_nits: f32,
    relative_to_max_display: c_int,
    linear_below: f32,
    uses_original_profile: c_int,
    have_preview: c_int,
    have_animation: c_int,
    orientation: c_int,
    num_color_channels: u32,
    num_extra_channels: u32,
    alpha_bits: u32,
    alpha_exponent_bits: u32,
    alpha_premultiplied: c_int,
    preview: [u32; 2],
    animation: [u32; 4],
    intrinsic_xsize: u32,
    intrinsic_ysize: u32,
    padding: [u8; 100],
}

/// `JxlColorEncoding`, only filled by JxlColorEncodingSetToSRGB.
#[repr(C)]
struct ColorEncoding {
    color_space: c_int,
    white_point: c_int,
    white_point_xy: [f64; 2],
    primaries: c_int,
    primaries_red_xy: [f64; 2],
    primaries_green_xy: [f64; 2],
    primaries_blue_xy: [f64; 2],
    transfer_function: c_int,
    gamma: f64,
    rendering_intent: c_int,
}

/// `JxlPixelFormat`.
#[repr(C)]
struct PixelFormat {
    num_channels: u32,
    data_type: c_int,
    endianness: c_int,
    align: usize,
}

type Ptr = *mut c_void;

struct Api {
    encoder_create: unsafe extern "C" fn(*const c_void) -> Ptr,
    encoder_destroy: unsafe extern "C" fn(Ptr),
    encoder_set_parallel_runner: unsafe extern "C" fn(Ptr, *const c_void, Ptr) -> c_int,
    encoder_init_basic_info: unsafe extern "C" fn(*mut BasicInfo),
    encoder_set_basic_info: unsafe extern "C" fn(Ptr, *const BasicInfo) -> c_int,
    color_encoding_set_to_srgb: unsafe extern "C" fn(*mut ColorEncoding, c_int),
    encoder_set_color_encoding: unsafe extern "C" fn(Ptr, *const ColorEncoding) -> c_int,
    encoder_frame_settings_create: unsafe extern "C" fn(Ptr, *const c_void) -> Ptr,
    encoder_add_image_frame: unsafe extern "C" fn(Ptr, *const PixelFormat, *const c_void, usize) -> c_int,
    encoder_close_input: unsafe extern "C" fn(Ptr),
    encoder_process_output: unsafe extern "C" fn(Ptr, *mut *mut u8, *mut usize) -> c_int,
    encoder_get_error: unsafe extern "C" fn(Ptr) -> c_int,
    /// `JxlThreadParallelRunner` itself, handed to libjxl as a function pointer.
    thread_runner: *const c_void,
    thread_runner_create: unsafe extern "C" fn(*const c_void, usize) -> Ptr,
    thread_runner_destroy: unsafe extern "C" fn(Ptr),
    thread_runner_default_threads: unsafe extern "C" fn() -> usize,
    // Keep the function pointers above valid; never unloaded since Api lives in a static.
    _jxl: Library,
    _threads: Library,
}

// SAFETY: `thread_runner` is a code address in a library that is never unloaded; nothing in Api is mutable.
unsafe impl Send for Api {}
// SAFETY: as above.
unsafe impl Sync for Api {}

fn api() -> Result<&'static Api, String> {
    static API: OnceLock<Result<Api, String>> = OnceLock::new();
    API.get_or_init(load_api).as_ref().map_err(Clone::clone)
}

fn load_api() -> Result<Api, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let open = |name: &str| {
        let path = exe.with_file_name(name);
        if !path.is_file() {
            return Err(format!("{name} not found"));
        }
        // SAFETY: loading by absolute path the libjxl build shipped with the app (its brotli and cms DLLs sit beside the exe); its initializers are trusted.
        unsafe { Library::new(&path) }.map_err(|e| format!("{name}: {e}"))
    };
    let (jxl, threads) = (open("jxl.dll")?, open("jxl_threads.dll")?);
    macro_rules! sym {
        ($lib:ident, $name:literal) => {
            // SAFETY: the field type this is assigned to matches the 0.13.0 header prototype of $name.
            *unsafe { $lib.get(concat!($name, "\0").as_bytes()) }.map_err(|e| format!("libjxl: {}: {e}", $name))?
        };
    }
    Ok(Api {
        encoder_create: sym!(jxl, "JxlEncoderCreate"),
        encoder_destroy: sym!(jxl, "JxlEncoderDestroy"),
        encoder_set_parallel_runner: sym!(jxl, "JxlEncoderSetParallelRunner"),
        encoder_init_basic_info: sym!(jxl, "JxlEncoderInitBasicInfo"),
        encoder_set_basic_info: sym!(jxl, "JxlEncoderSetBasicInfo"),
        color_encoding_set_to_srgb: sym!(jxl, "JxlColorEncodingSetToSRGB"),
        encoder_set_color_encoding: sym!(jxl, "JxlEncoderSetColorEncoding"),
        encoder_frame_settings_create: sym!(jxl, "JxlEncoderFrameSettingsCreate"),
        encoder_add_image_frame: sym!(jxl, "JxlEncoderAddImageFrame"),
        encoder_close_input: sym!(jxl, "JxlEncoderCloseInput"),
        encoder_process_output: sym!(jxl, "JxlEncoderProcessOutput"),
        encoder_get_error: sym!(jxl, "JxlEncoderGetError"),
        thread_runner: sym!(threads, "JxlThreadParallelRunner"),
        thread_runner_create: sym!(threads, "JxlThreadParallelRunnerCreate"),
        thread_runner_destroy: sym!(threads, "JxlThreadParallelRunnerDestroy"),
        thread_runner_default_threads: sym!(threads, "JxlThreadParallelRunnerDefaultNumWorkerThreads"),
        _jxl: jxl,
        _threads: threads,
    })
}

/// True when the bundled libjxl loads; false only on a broken install (DLL missing or unloadable).
pub fn can_save_jxl() -> bool {
    api().is_ok()
}

/// Owns one libjxl object and releases it on drop.
struct Guard {
    ptr: Ptr,
    release: unsafe extern "C" fn(Ptr),
}

impl Guard {
    fn new(ptr: Ptr, release: unsafe extern "C" fn(Ptr), what: &str) -> Result<Self, String> {
        if ptr.is_null() {
            return Err(format!("libjxl returned no {what}"));
        }
        Ok(Guard { ptr, release })
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        // SAFETY: ptr is a live, non-null object from libjxl, released exactly once with its matching function.
        unsafe { (self.release)(self.ptr) }
    }
}

/// Writes `rgba` to `path` as JPEG XL with libjxl's defaults (lossy, distance 1.0: visually lossless), with an alpha
/// channel only when some pixel is not opaque.
pub fn encode_jxl(rgba: &RgbaImage, path: &Path) -> Result<(), String> {
    let bytes = encode_with(api()?, rgba).map_err(|e| format!("JPEG XL encode: {e}"))?;
    std::fs::write(path, bytes).map_err(|e| e.to_string())
}

fn encode_with(api: &Api, rgba: &RgbaImage) -> Result<Vec<u8>, String> {
    let alpha = rgba.pixels().any(|p| p[3] != 255);
    let pixels: Cow<[u8]> = if alpha {
        Cow::Borrowed(rgba.as_raw())
    } else {
        Cow::Owned(rgba.as_raw().as_chunks::<4>().0.iter().flat_map(|p| [p[0], p[1], p[2]]).collect())
    };
    let (w, h) = rgba.dimensions();
    let format = PixelFormat {
        num_channels: if alpha { 4 } else { 3 },
        data_type: TYPE_UINT8,
        endianness: NATIVE_ENDIAN,
        align: 0,
    };

    // SAFETY (whole block): every call matches its 0.13.0 prototype; created objects are null-checked and guarded, the
    // runner guard is declared before the encoder so it outlives it; the frame settings belong to the encoder; `pixels`
    // holds w*h*num_channels bytes; each ProcessOutput call gets the unused tail of `out`.
    unsafe {
        let runner = Guard::new((api.thread_runner_create)(null(), (api.thread_runner_default_threads)()), api.thread_runner_destroy, "thread runner")?;
        let enc = Guard::new((api.encoder_create)(null()), api.encoder_destroy, "encoder")?;
        let check = |status: c_int, what: &str| if status == ENC_SUCCESS { Ok(()) } else { Err(format!("{what} failed (libjxl error {})", (api.encoder_get_error)(enc.ptr))) };
        check((api.encoder_set_parallel_runner)(enc.ptr, api.thread_runner, runner.ptr), "JxlEncoderSetParallelRunner")?;

        let mut info = std::mem::zeroed::<BasicInfo>();
        (api.encoder_init_basic_info)(&mut info);
        (info.xsize, info.ysize, info.bits_per_sample) = (w, h, 8);
        if alpha {
            (info.num_extra_channels, info.alpha_bits) = (1, 8);
        }
        check((api.encoder_set_basic_info)(enc.ptr, &info), "JxlEncoderSetBasicInfo")?;
        let mut color = std::mem::zeroed::<ColorEncoding>();
        (api.color_encoding_set_to_srgb)(&mut color, 0);
        check((api.encoder_set_color_encoding)(enc.ptr, &color), "JxlEncoderSetColorEncoding")?;

        let settings = (api.encoder_frame_settings_create)(enc.ptr, null());
        if settings.is_null() {
            return Err("libjxl returned no frame settings".into());
        }
        check((api.encoder_add_image_frame)(settings, &format, pixels.as_ptr().cast(), pixels.len()), "JxlEncoderAddImageFrame")?;
        (api.encoder_close_input)(enc.ptr);

        let mut out = vec![0u8; 1 << 16];
        let mut used = 0;
        loop {
            let mut next = out.as_mut_ptr().add(used);
            let mut avail = out.len() - used;
            let status = (api.encoder_process_output)(enc.ptr, &mut next, &mut avail);
            used = out.len() - avail;
            if status != ENC_NEED_MORE_OUTPUT {
                check(status, "JxlEncoderProcessOutput")?;
                break;
            }
            out.resize(out.len() * 2, 0);
        }
        out.truncate(used);
        Ok(out)
    }
}
