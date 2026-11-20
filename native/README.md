# Native runtime DLLs

Loaded at runtime by `viewer-core`: `src\picture\heif.rs` for HEIC / HEIF / AVIF, `src\picture\jxl.rs` to write JPEG XL. `viewer-core\build.rs` copies every `*.dll` here next to the built binaries and test executables.

| File | Version | Source | SHA-256 |
|------|---------|--------|---------|
| `libheif.dll` | libheif 1.20.2 (x64) | NuGet `LibHeif.Native.Runtime` 1.20.2, `runtimes\win-x64\native\` | `D7CB4F5AB61DEC6C2619A502763608CF69EC73ED421700C4FEF03D60ADC4B621` |
| `jxl.dll` | libjxl 0.13.0 (x64) | GitHub release `libjxl/libjxl` v0.13.0, `jxl-x64-windows.zip`, `x64-windows\bin\` | `ABCC2145CB3838E753EFD9748174868773E6102AE010AE8A408C9DB101BD7C6A` |
| `jxl_cms.dll` | libjxl 0.13.0 (x64) | same | `11320FB72D524A40D26A9612012175F0B846759CB071ED1B0EC5C44A48D328A1` |
| `jxl_threads.dll` | libjxl 0.13.0 (x64) | same | `9D82803B23EC66415677E08EE86E915F4557777ED20DFFD11C348376F8D94E3C` |
| `brotlicommon.dll` | brotli 1.2.1 (x64) | same | `2F302085531B2B35695CE265D2809E1E5C3864C629F0D88FE7A01F1CB35EBE09` |
| `brotlidec.dll` | brotli 1.2.1 (x64) | same | `E690802F3F9B25626B0460AB0629702E7FE26920413D00399D290D4A13DD6919` |
| `brotlienc.dll` | brotli 1.2.1 (x64) | same | `D393E6756488E0D1A71DE8E66E6885C08515C7FF1E92B2014B847AE260AB14BF` |
| `msvcp140.dll` | 14.44.35211.0 | Microsoft Visual C++ 2015-2022 redistributable, x64 (copied from the C# app's `ImageViewer\Native\`) | `0F885B509A685D2BBFA652FED26B5FB31D88FBDAB0A978C641D1C7B8AA460AA9` |
| `vcruntime140.dll` | 14.44.35211.0 | same | `D5E4D9A3E835FA679450145D6A7D94E36573A509317111904D9B3712C30D9066` |
| `vcruntime140_1.dll` | 14.44.35211.0 | same | `1F2D41C4AA5DB0BC33EBF7B66D72943A817D7CE6CBE880502A9403823633093F` |

`libheif.dll` is built from [strukturag/libheif](https://github.com/strukturag/libheif) with the `release-noplugins` preset: HEVC (libde265) and AV1 (dav1d / libaom) codecs are compiled in, no plugin files needed. It decodes HEVC and AV1 and encodes AV1 only.

The FFI in `heif.rs` follows the libheif 1.20.2 C headers. Upgrading this DLL means re-checking those prototypes and enum values.

`jxl.rs` loads `jxl.dll` and `jxl_threads.dll`; `jxl.dll` itself loads `jxl_cms.dll` and the brotli DLLs from the exe folder; JPEG XL decoding stays in pure Rust (jxl-oxide). The FFI in `jxl.rs` follows the libjxl 0.13.0 headers, including the `JxlBasicInfo`, `JxlColorEncoding` and `JxlPixelFormat` layouts: re-check them on an upgrade.
