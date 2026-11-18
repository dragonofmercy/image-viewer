# Native runtime DLLs

Loaded at runtime by `viewer-core` (`src\picture\heif.rs`) for HEIC / HEIF / AVIF. `viewer-core\build.rs` copies every `*.dll` here next to the built binaries and test executables.

| File | Version | Source | SHA-256 |
|------|---------|--------|---------|
| `libheif.dll` | libheif 1.20.2 (x64) | NuGet `LibHeif.Native.Runtime` 1.20.2, `runtimes\win-x64\native\` | `D7CB4F5AB61DEC6C2619A502763608CF69EC73ED421700C4FEF03D60ADC4B621` |
| `msvcp140.dll` | 14.44.35211.0 | Microsoft Visual C++ 2015-2022 redistributable, x64 (copied from the C# app's `ImageViewer\Native\`) | `0F885B509A685D2BBFA652FED26B5FB31D88FBDAB0A978C641D1C7B8AA460AA9` |
| `vcruntime140.dll` | 14.44.35211.0 | same | `D5E4D9A3E835FA679450145D6A7D94E36573A509317111904D9B3712C30D9066` |
| `vcruntime140_1.dll` | 14.44.35211.0 | same | `1F2D41C4AA5DB0BC33EBF7B66D72943A817D7CE6CBE880502A9403823633093F` |

`libheif.dll` is built from [strukturag/libheif](https://github.com/strukturag/libheif) with the `release-noplugins` preset: HEVC (libde265) and AV1 (dav1d / libaom) codecs are compiled in, no plugin files needed. It decodes HEVC and AV1 and encodes AV1 only.

The FFI in `heif.rs` follows the libheif 1.20.2 C headers. Upgrading this DLL means re-checking those prototypes and enum values.
