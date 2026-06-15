# Design - Test project `ImageViewer.Tests`

Date: 2026-06-15
Status: Approved

## Goal

Introduce the first automated test project for ImageViewer (none exists today).
Cover the genuinely unit-testable, UI-free logic: the image wrapper load/transform
paths, the clipboard DIB layout, the natural string comparer, the localization
key parity, and the pure utility extensions. Establish a foundation that runs from
both Visual Studio 2022 and `dotnet test`.

## Decisions (locked)

- **Attachment strategy:** direct `ProjectReference` to `ImageViewer.csproj` plus
  `InternalsVisibleTo`. No architectural refactor, low churn. The alternative
  (extract an `ImageViewer.Core` class library) was rejected as too much churn on
  an established codebase for this first pass.
- **Framework:** xUnit.
- **Scope:** "broad foundation" - utilities, DIB, Culture, and `Wrapper.Image`
  with fixtures. Excludes `Settings` (HKCU registry side effects) and all UI/`Context`.

## Architecture

New project `build/ImageViewer.Tests/`:

- TFM `net10.0-windows10.0.22621`, platform x64.
- **Not** self-contained, `IsPackable=false`, MSIX tooling off.
- Packages: `Microsoft.NET.Test.Sdk`, `xunit`, `xunit.runner.visualstudio`.
- `ProjectReference` -> `ImageViewer.csproj`.
- Added to `ImageViewer.sln` with `Debug|x64` and `Release|x64` configurations.

The app project exposes its `internal` types to the test assembly via an ItemGroup
entry in `ImageViewer.csproj`:

```xml
<ItemGroup>
  <InternalsVisibleTo Include="ImageViewer.Tests" />
</ItemGroup>
```

(The test assembly name is constant `ImageViewer.Tests` regardless of the app's
config-dependent `AssemblyName`, so a single entry covers both Debug and Release.)

### Risk and mitigation (direct-reference choice)

The test host loads the WinUI / Windows App SDK assemblies transitively. As long as
no test activates a UI type (`WriteableBitmap`, `BitmapImage`, `Context`, any XAML
type), the JIT never initializes the WinUI runtime - the methods under test
(`Load`, `GetBgra32Pixels`, transforms, utilities) have no WinUI dependency at the
call level even though `Wrapper/Image.cs` references those types in other methods.

Mitigation: a minimal smoke test is validated first. If the host refused to load,
the documented fallback is extracting an `ImageViewer.Core` library - but that is
out of scope unless the direct path proves unworkable.

## Production code changes (minimal)

- `ClipboardHelper.BuildDib`: `private` -> `internal`, so the DIB byte layout can be
  asserted directly. No other production change.

## Test coverage

| Test class | Target | Key cases |
|---|---|---|
| `ExtensionsTests` | `Extensions.RemoveAtIndex`, `UcFirst` | `[a,b,c]` remove first/middle/last; out-of-range index = no-op (returns original); `UcFirst` capitalizes first char |
| `NaturalStringComparerTests` | `NaturalStringComparer` (shlwapi `StrCmpLogicalW`) | `img2 < img10`; Explorer-style natural ordering of a list |
| `DibTests` | `ClipboardHelper.BuildDib` | header fields (`biSize=40`, `biBitCount=32`, `biHeight=+height`, `biSizeImage=stride*height`); bottom-up flip - first source row ends at the bottom of the DIB |
| `CultureTests` | `Culture`, `Strings.En`, `Strings.Fr` | en/fr key-set parity both directions (no key missing on either side); `GetString` on an unknown key returns `[KEY]` |
| `ImageTests` | `Wrapper.Image` | load per format succeeds and reports dimensions; `AutoOrient` swaps W/H on an EXIF orientation-6 fixture; `GetBgra32Pixels` returns `W*H*4` bytes with correct out dims; one transform (rotate) from `Image.Transform.cs` |

### Culture testing detail

`Culture.Init()` calls `Windows.System.UserProfile.GlobalizationPreferences`
(WinRT) and must NOT be called in tests. Parity is tested by invoking
`En.GetStrings()` / `Fr.GetStrings()` directly (accessible via `InternalsVisibleTo`)
and comparing key sets. The `[KEY]` fallback is tested through `GetString` while
`_Strings` is uninitialized (null), which returns `[KEY]` for any input.

### Async load helper

`Wrapper.Image.Load` is fire-and-forget (`async void` + `ImageLoaded` / `ImageFailed`
events). A test helper `LoadAsync(path)` wires a `TaskCompletionSource` to both
events with a timeout so image tests are `async` and deterministic.

## Fixtures

- jpg/png/gif/tiff/webp fixtures are generated at arrange time via ImageSharp
  (no committed binaries).
- The EXIF orientation-6 fixture is generated at arrange time (ImageSharp
  `ExifProfile` with `Orientation = 6`, saved to a temp file).
- Only `sample.svg` and `sample.ico` are committed under `Fixtures/` (ImageSharp
  cannot synthesize them).
- Temp files use the test temp directory and are cleaned up after each test.

## Out of scope

- `Helpers.Settings` (HKCU registry side effects).
- `Helpers.Context` and all UI / XAML.
- CI wiring (no CI exists; manual `dotnet test` / VS Test Explorer).

## Acceptance

- `dotnet test` (or `dotnet build` then VS Test Explorer) runs green on x64.
- The app build remains green with `TreatWarningsAsErrors` on (Debug and Release).
- No change to app runtime behavior beyond the `BuildDib` visibility widening.
