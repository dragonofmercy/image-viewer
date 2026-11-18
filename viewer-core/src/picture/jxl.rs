//! JPEG XL: read with the pure-Rust jxl-oxide, write through the WIC encoder of Microsoft's Store
//! "JPEG XL Image Extension" when it is installed and actually instantiable.

use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::sync::OnceLock;

use image::{DynamicImage, ImageBuffer, RgbaImage};
use jxl_oxide::{EnumColourEncoding, JxlImage, RenderingIntent};
use windows::Win32::Foundation::{E_POINTER, GENERIC_WRITE, RPC_E_CHANGED_MODE, WINCODEC_ERR_COMPONENTNOTFOUND};
use windows::Win32::Graphics::Imaging::{CLSID_WICImagingFactory, GUID_WICPixelFormat32bppBGRA, IWICBitmapCodecInfo, IWICBitmapEncoder, IWICImagingFactory, WICBitmapEncoderNoCache, WICComponentEnumerateDefault, WICEncoder};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize};
use windows::core::{GUID, Interface, PCWSTR, PWSTR};

use super::LoadError;

/// First frame as 8-bit sRGB RGBA (orientation applied by jxl-oxide), plus whether it has alpha.
pub(crate) fn decode_jxl(bytes: &[u8]) -> Result<(RgbaImage, bool), LoadError> {
    let err = |e: Box<dyn std::error::Error + Send + Sync>| LoadError::Decode(e.to_string());
    let mut image = JxlImage::builder().read(bytes).map_err(err)?;
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

/// Probed once per process with a real encoder creation: an uninstalled Store extension stays in
/// the WIC codec list until the next reboot while failing to instantiate.
pub fn can_save_jxl() -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| with_wic(|factory| create_jxl_encoder(factory).map(|_| ())).is_ok())
}

/// Writes `rgba` to `path` as JPEG XL through the WIC encoder.
pub fn encode_jxl_wic(rgba: &RgbaImage, path: &Path) -> Result<(), String> {
    let mut bgra = rgba.as_raw().clone();
    bgra.as_chunks_mut::<4>().0.iter_mut().for_each(|p| p.swap(0, 2));
    let (w, h) = rgba.dimensions();
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
    with_wic(|factory| {
        let encoder = create_jxl_encoder(factory)?;
        // SAFETY: plain WIC calls on live interfaces; `wide` is NUL-terminated and outlives the call, `bgra` is w*h*4 bytes.
        unsafe {
            let bitmap = factory.CreateBitmapFromMemory(w, h, &GUID_WICPixelFormat32bppBGRA, w * 4, &bgra)?;
            let stream = factory.CreateStream()?;
            stream.InitializeFromFilename(PCWSTR(wide.as_ptr()), GENERIC_WRITE.0)?;
            encoder.Initialize(&stream, WICBitmapEncoderNoCache)?;
            let mut frame = None;
            encoder.CreateNewFrame(&mut frame, std::ptr::null_mut())?;
            let frame = frame.ok_or_else(|| windows::core::Error::from(E_POINTER))?;
            frame.Initialize(None)?;
            frame.SetSize(w, h)?;
            let mut format = GUID_WICPixelFormat32bppBGRA;
            frame.SetPixelFormat(&mut format)?;
            // WriteSource converts to whatever format the encoder negotiated in `format`.
            frame.WriteSource(&bitmap, std::ptr::null())?;
            frame.Commit()?;
            encoder.Commit()?;
        }
        Ok(())
    })
    .map_err(|e| format!("JPEG XL encode: {e}"))
}

fn create_jxl_encoder(factory: &IWICImagingFactory) -> windows::core::Result<IWICBitmapEncoder> {
    let container = find_jxl_container(factory)?.ok_or_else(|| windows::core::Error::from(WINCODEC_ERR_COMPONENTNOTFOUND))?;
    // SAFETY: plain WIC call; `container` lives across it.
    unsafe { factory.CreateEncoder(&container, std::ptr::null()) }
}

/// Container format of the first WIC encoder that lists `.jxl` among its extensions.
fn find_jxl_container(factory: &IWICImagingFactory) -> windows::core::Result<Option<GUID>> {
    // SAFETY: plain WIC/COM calls; every buffer passed is sized by its slice.
    unsafe {
        let components = factory.CreateComponentEnumerator(WICEncoder.0 as u32, WICComponentEnumerateDefault.0 as u32)?;
        loop {
            let mut item = [None];
            let mut fetched = 0;
            components.Next(&mut item, Some(&mut fetched)).ok()?;
            let Some(unknown) = item[0].take().filter(|_| fetched == 1) else { return Ok(None) };
            // A broken third-party codec is skipped rather than hiding the ones after it.
            let Ok(info) = unknown.cast::<IWICBitmapCodecInfo>() else { continue };
            let mut len = 0;
            // Size query: the wrapper would pass a dangling non-null pointer for an empty slice, which WIC rejects (E_INVALIDARG).
            if (Interface::vtable(&info).GetFileExtensions)(info.as_raw(), 0, PWSTR::null(), &mut len).is_err() {
                continue;
            }
            let mut exts = vec![0u16; len as usize];
            if info.GetFileExtensions(&mut exts, &mut len).is_err() {
                continue;
            }
            let exts = String::from_utf16_lossy(&exts);
            if exts.trim_end_matches('\0').split(',').any(|e| e.trim().eq_ignore_ascii_case(".jxl")) {
                return Ok(Some(info.GetContainerFormat()?));
            }
        }
    }
}

/// Runs `f` with a WIC factory on a COM-initialized (MTA) thread. A thread already in another
/// apartment (RPC_E_CHANGED_MODE) is used as is; only our own successful init is balanced.
fn with_wic<T>(f: impl FnOnce(&IWICImagingFactory) -> windows::core::Result<T>) -> windows::core::Result<T> {
    struct ComGuard(bool);
    impl Drop for ComGuard {
        fn drop(&mut self) {
            if self.0 {
                // SAFETY: balances the successful CoInitializeEx made by this guard's owner on this thread.
                unsafe { CoUninitialize() };
            }
        }
    }
    // SAFETY: COM init on the current thread; the reserved pointer is null as required.
    let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    if hr.is_err() && hr != RPC_E_CHANGED_MODE {
        return Err(hr.into());
    }
    // Declared before the factory so it drops last: every COM object is released before CoUninitialize.
    let _guard = ComGuard(hr.is_ok());
    // SAFETY: standard WIC factory creation on an initialized thread.
    let factory: IWICImagingFactory = unsafe { CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER) }?;
    f(&factory)
}
