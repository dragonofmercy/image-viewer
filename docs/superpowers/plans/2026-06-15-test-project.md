# ImageViewer.Tests Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the first xUnit test project for ImageViewer, covering the UI-free logic (image wrapper, clipboard DIB, natural sort, localization parity, utilities).

**Architecture:** A new `build/ImageViewer.Tests/` xUnit project references `ImageViewer.csproj` directly and reads its `internal` types via `InternalsVisibleTo`. Tests never touch WinUI/XAML types, so the WinUI runtime is never initialized in the test host. Image fixtures are generated at arrange time (no committed binaries).

**Tech Stack:** .NET 10 (`net10.0-windows10.0.22621`), x64, xUnit, SixLabors.ImageSharp (transitive via the app reference), System.Drawing.Common (for ICO fixture generation).

---

## File structure

- Create: `build/ImageViewer.Tests/ImageViewer.Tests.csproj` - test project, references the app.
- Create: `build/ImageViewer.Tests/ExtensionsTests.cs` - `Extensions` pure methods.
- Create: `build/ImageViewer.Tests/NaturalStringComparerTests.cs` - shlwapi natural sort.
- Create: `build/ImageViewer.Tests/DibTests.cs` - `ClipboardHelper.BuildDib` layout.
- Create: `build/ImageViewer.Tests/CultureTests.cs` - en/fr parity + `[KEY]` fallback.
- Create: `build/ImageViewer.Tests/TestSupport.cs` - `TempDir`, `FixtureFactory`, `ImageLoader.LoadAsync`.
- Create: `build/ImageViewer.Tests/ImageTests.cs` - `Wrapper.Image` load/AutoOrient/pixels/transform.
- Modify: `build/ImageViewer/ImageViewer.csproj` - add `<InternalsVisibleTo Include="ImageViewer.Tests" />`.
- Modify: `build/ImageViewer/Utilities/ClipboardHelper.cs` - `BuildDib` `private` -> `internal`.
- Modify: `build/ImageViewer.sln` - add the test project (VS integration).

**Conventions:** PascalCase private fields, ASCII only (no curly quotes, no em/en dash), English identifiers/comments. The type `ImageViewer.Wrapper.Image` collides with `SixLabors.ImageSharp.Image` and `System.Drawing.Image`; in test files alias it: `using ViewerImage = ImageViewer.Wrapper.Image;`.

**Commands used throughout:**
- App compile check: `dotnet build ImageViewer\ImageViewer.csproj -c Debug -p:Platform=x64`
- Run tests: `dotnet test ImageViewer.Tests\ImageViewer.Tests.csproj -c Debug -p:Platform=x64`
- Run one test: append `--filter "FullyQualifiedName~ClassName.MethodName"`

All git commits use the repo-local identity (already configured: `Dragon` / `dragonofmercy@hotmail.com`) and end with the `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>` trailer. Do not push.

---

## Task 1: Scaffold the test project and prove the direct reference loads

This is the integration-risk gate: it validates that an xUnit host can reference the WinUI app project and read its `internal` types without booting WinUI.

**Files:**
- Create: `build/ImageViewer.Tests/ImageViewer.Tests.csproj`
- Create: `build/ImageViewer.Tests/SmokeTest.cs`
- Modify: `build/ImageViewer/ImageViewer.csproj`

- [ ] **Step 1: Add `InternalsVisibleTo` to the app project**

In `build/ImageViewer/ImageViewer.csproj`, add a new ItemGroup just after the existing package `ItemGroup` (the one ending with `<Manifest Include="$(ApplicationManifest)" />`):

```xml
    <ItemGroup>
        <!-- Grant the test assembly access to internal types (Wrapper.Image, Utilities, Culture, ...) -->
        <InternalsVisibleTo Include="ImageViewer.Tests" />
    </ItemGroup>
```

- [ ] **Step 2: Create the test project file**

Create `build/ImageViewer.Tests/ImageViewer.Tests.csproj`:

```xml
<Project Sdk="Microsoft.NET.Sdk">

    <PropertyGroup>
        <TargetFramework>net10.0-windows10.0.22621</TargetFramework>
        <TargetPlatformMinVersion>10.0.22621</TargetPlatformMinVersion>
        <RootNamespace>ImageViewer.Tests</RootNamespace>
        <AssemblyName>ImageViewer.Tests</AssemblyName>
        <Platforms>x64</Platforms>
        <RuntimeIdentifier>win-x64</RuntimeIdentifier>
        <LangVersion>12</LangVersion>
        <Nullable>disable</Nullable>
        <IsPackable>false</IsPackable>
        <IsTestProject>true</IsTestProject>
        <EnableMsixTooling>false</EnableMsixTooling>
        <AppendTargetFrameworkToOutputPath>false</AppendTargetFrameworkToOutputPath>
        <AppendRuntimeIdentifierToOutputPath>false</AppendRuntimeIdentifierToOutputPath>
    </PropertyGroup>

    <ItemGroup>
        <PackageReference Include="Microsoft.NET.Test.Sdk" Version="17.12.0" />
        <PackageReference Include="xunit" Version="2.9.2" />
        <PackageReference Include="xunit.runner.visualstudio" Version="2.8.2" />
        <!-- Explicit (not just transitive) so the ICO-fixture generator can use System.Drawing. -->
        <PackageReference Include="System.Drawing.Common" Version="10.0.9" />
    </ItemGroup>

    <ItemGroup>
        <ProjectReference Include="..\ImageViewer\ImageViewer.csproj" />
    </ItemGroup>

</Project>
```

- [ ] **Step 3: Write the smoke test (exercises an internal type via InternalsVisibleTo)**

Create `build/ImageViewer.Tests/SmokeTest.cs`:

```csharp
using ImageViewer.Utilities;
using Xunit;

namespace ImageViewer.Tests;

public class SmokeTest
{
    [Fact]
    public void TestHost_CanCallInternalExtension()
    {
        // Proves the host loads the app assembly and InternalsVisibleTo works,
        // without ever touching a WinUI type.
        Assert.Equal("Abc", "abc".UcFirst());
    }
}
```

- [ ] **Step 4: Run the smoke test (expected to pass once it builds)**

Run: `dotnet test ImageViewer.Tests\ImageViewer.Tests.csproj -c Debug -p:Platform=x64`
Expected: build succeeds, 1 test passes.

Troubleshooting if the run fails to build or load (integration-risk gate):
- If the WinUI ProjectReference fails to resolve targets, add `<UseWinUI>true</UseWinUI>` and `<WindowsAppSDKSelfContained>true</WindowsAppSDKSelfContained>` to the test project's PropertyGroup, then retry.
- If a test fails at runtime with a WinUI bootstrap/`Microsoft.ui.xaml` error, that means a tested path touched a UI type - it should not at this stage; re-check the smoke test only references `Extensions`.
- Last resort (out of scope, only if the host is fundamentally unusable): the documented fallback is extracting an `ImageViewer.Core` library; stop and report instead of pursuing it unprompted.

- [ ] **Step 5: Verify the app itself still builds**

Run: `dotnet build ImageViewer\ImageViewer.csproj -c Debug -p:Platform=x64`
Expected: build succeeds, 0 warnings, 0 errors (TreatWarningsAsErrors is on).

- [ ] **Step 6: Add the test project to the solution (VS integration)**

Run: `dotnet sln ImageViewer.sln add ImageViewer.Tests\ImageViewer.Tests.csproj`
Then open `ImageViewer.sln` and ensure the new project has `Debug|x64` and `Release|x64` rows under `GlobalSection(ProjectConfigurationPlatforms)` (mirroring the existing project's `ActiveCfg`/`Build.0` lines; map any `Any CPU` rows `dotnet sln` may have added to `x64`). CLI test runs target the csproj directly and do not depend on these rows.

- [ ] **Step 7: Commit**

```bash
git add ImageViewer.Tests/ImageViewer.Tests.csproj ImageViewer.Tests/SmokeTest.cs ImageViewer/ImageViewer.csproj ImageViewer.sln
git commit -m "Add ImageViewer.Tests xUnit project (direct reference + InternalsVisibleTo)"
```

---

## Task 2: Extensions tests (RemoveAtIndex, UcFirst)

**Files:**
- Create: `build/ImageViewer.Tests/ExtensionsTests.cs`

- [ ] **Step 1: Write the failing tests**

Create `build/ImageViewer.Tests/ExtensionsTests.cs`:

```csharp
using ImageViewer.Utilities;
using Xunit;

namespace ImageViewer.Tests;

public class ExtensionsTests
{
    [Fact]
    public void RemoveAtIndex_RemovesMiddleElement()
    {
        string[] result = new[] { "a", "b", "c" }.RemoveAtIndex(1);
        Assert.Equal(new[] { "a", "c" }, result);
    }

    [Fact]
    public void RemoveAtIndex_RemovesFirstElement()
    {
        string[] result = new[] { "a", "b", "c" }.RemoveAtIndex(0);
        Assert.Equal(new[] { "b", "c" }, result);
    }

    [Fact]
    public void RemoveAtIndex_RemovesLastElement()
    {
        string[] result = new[] { "a", "b", "c" }.RemoveAtIndex(2);
        Assert.Equal(new[] { "a", "b" }, result);
    }

    [Theory]
    [InlineData(-1)]
    [InlineData(3)]
    public void RemoveAtIndex_OutOfRange_ReturnsOriginalUnchanged(int index)
    {
        string[] original = new[] { "a", "b", "c" };
        string[] result = original.RemoveAtIndex(index);
        Assert.Equal(new[] { "a", "b", "c" }, result);
    }

    [Fact]
    public void UcFirst_CapitalizesFirstCharacter()
    {
        Assert.Equal("Hello", "hello".UcFirst());
    }
}
```

- [ ] **Step 2: Run tests (expected pass - production code already exists)**

Run: `dotnet test ImageViewer.Tests\ImageViewer.Tests.csproj -c Debug -p:Platform=x64 --filter "FullyQualifiedName~ExtensionsTests"`
Expected: all 6 cases PASS. (If a case fails, the test encodes the intended contract - fix the test only if it misreads the existing behavior of `Extensions.RemoveAtIndex`/`UcFirst`.)

- [ ] **Step 3: Commit**

```bash
git add ImageViewer.Tests/ExtensionsTests.cs
git commit -m "Test Extensions.RemoveAtIndex and UcFirst"
```

---

## Task 3: NaturalStringComparer tests

**Files:**
- Create: `build/ImageViewer.Tests/NaturalStringComparerTests.cs`

- [ ] **Step 1: Write the failing tests**

Create `build/ImageViewer.Tests/NaturalStringComparerTests.cs`:

```csharp
using System;
using ImageViewer.Utilities;
using Xunit;

namespace ImageViewer.Tests;

public class NaturalStringComparerTests
{
    [Fact]
    public void Compare_OrdersImg2BeforeImg10()
    {
        NaturalStringComparer comparer = new();
        Assert.True(comparer.Compare("img2", "img10") < 0);
    }

    [Fact]
    public void Sort_ProducesExplorerStyleNaturalOrder()
    {
        NaturalStringComparer comparer = new();
        string[] files = { "img10.jpg", "img2.jpg", "img1.jpg" };
        Array.Sort(files, comparer);
        Assert.Equal(new[] { "img1.jpg", "img2.jpg", "img10.jpg" }, files);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `dotnet test ImageViewer.Tests\ImageViewer.Tests.csproj -c Debug -p:Platform=x64 --filter "FullyQualifiedName~NaturalStringComparerTests"`
Expected: both PASS (uses real `StrCmpLogicalW` from shlwapi).

- [ ] **Step 3: Commit**

```bash
git add ImageViewer.Tests/NaturalStringComparerTests.cs
git commit -m "Test NaturalStringComparer natural ordering"
```

---

## Task 4: ClipboardHelper DIB layout tests

**Files:**
- Modify: `build/ImageViewer/Utilities/ClipboardHelper.cs`
- Create: `build/ImageViewer.Tests/DibTests.cs`

- [ ] **Step 1: Widen BuildDib to internal**

In `build/ImageViewer/Utilities/ClipboardHelper.cs`, change the signature:

```csharp
    private static byte[] BuildDib(byte[] topDownBgra, int width, int height)
```

to:

```csharp
    internal static byte[] BuildDib(byte[] topDownBgra, int width, int height)
```

- [ ] **Step 2: Write the failing tests**

Create `build/ImageViewer.Tests/DibTests.cs`:

```csharp
using System;
using ImageViewer.Utilities;
using Xunit;

namespace ImageViewer.Tests;

public class DibTests
{
    [Fact]
    public void BuildDib_WritesBitmapInfoHeader()
    {
        // 2x2 image, top-down BGRA (4 bytes per pixel)
        const int width = 2;
        const int height = 2;
        byte[] topDown = new byte[width * height * 4];

        byte[] dib = ClipboardHelper.BuildDib(topDown, width, height);

        Assert.Equal(40, BitConverter.ToInt32(dib, 0));                 // biSize
        Assert.Equal(width, BitConverter.ToInt32(dib, 4));             // biWidth
        Assert.Equal(height, BitConverter.ToInt32(dib, 8));           // biHeight (positive => bottom-up)
        Assert.Equal(1, BitConverter.ToUInt16(dib, 12));             // biPlanes
        Assert.Equal(32, BitConverter.ToUInt16(dib, 14));          // biBitCount
        Assert.Equal(0, BitConverter.ToInt32(dib, 16));           // biCompression = BI_RGB
        Assert.Equal(width * 4 * height, BitConverter.ToInt32(dib, 20)); // biSizeImage
        Assert.Equal(40 + width * 4 * height, dib.Length);
    }

    [Fact]
    public void BuildDib_FlipsRowsBottomUp()
    {
        // 1px wide, 2px tall so each row is exactly one BGRA pixel (stride = 4).
        const int width = 1;
        const int height = 2;
        byte[] topDown =
        {
            10, 11, 12, 13, // row 0 (visual top)
            20, 21, 22, 23  // row 1 (visual bottom)
        };

        byte[] dib = ClipboardHelper.BuildDib(topDown, width, height);

        // In a bottom-up DIB the first stored row is the visual bottom (top-down row 1).
        Assert.Equal(new byte[] { 20, 21, 22, 23 }, new[] { dib[40], dib[41], dib[42], dib[43] });
        Assert.Equal(new byte[] { 10, 11, 12, 13 }, new[] { dib[44], dib[45], dib[46], dib[47] });
    }
}
```

- [ ] **Step 3: Run tests**

Run: `dotnet test ImageViewer.Tests\ImageViewer.Tests.csproj -c Debug -p:Platform=x64 --filter "FullyQualifiedName~DibTests"`
Expected: both PASS.

- [ ] **Step 4: Verify the app still builds (visibility change)**

Run: `dotnet build ImageViewer\ImageViewer.csproj -c Debug -p:Platform=x64`
Expected: 0 warnings, 0 errors.

- [ ] **Step 5: Commit**

```bash
git add ImageViewer/Utilities/ClipboardHelper.cs ImageViewer.Tests/DibTests.cs
git commit -m "Test ClipboardHelper.BuildDib header and bottom-up flip"
```

---

## Task 5: Culture / localization parity tests

**Files:**
- Create: `build/ImageViewer.Tests/CultureTests.cs`

- [ ] **Step 1: Write the failing tests**

Create `build/ImageViewer.Tests/CultureTests.cs`. Note: do NOT call `Culture.Init()` (it touches WinRT `GlobalizationPreferences`). Parity compares the dictionaries directly; the `[KEY]` fallback works for any unknown key regardless of init state.

```csharp
using System.Collections.Generic;
using System.Linq;
using ImageViewer.Helpers;
using ImageViewer.Strings;
using Xunit;

namespace ImageViewer.Tests;

public class CultureTests
{
    [Fact]
    public void En_And_Fr_HaveIdenticalKeySets()
    {
        Dictionary<string, string> en = En.GetStrings();
        Dictionary<string, string> fr = Fr.GetStrings();

        string[] missingInFr = en.Keys.Except(fr.Keys).OrderBy(k => k).ToArray();
        string[] missingInEn = fr.Keys.Except(en.Keys).OrderBy(k => k).ToArray();

        Assert.True(missingInFr.Length == 0, "Keys present in En but missing in Fr: " + string.Join(", ", missingInFr));
        Assert.True(missingInEn.Length == 0, "Keys present in Fr but missing in En: " + string.Join(", ", missingInEn));
    }

    [Fact]
    public void GetString_UnknownKey_ReturnsBracketedKey()
    {
        Assert.Equal("[__DEFINITELY_NOT_A_KEY__]", Culture.GetString("__DEFINITELY_NOT_A_KEY__"));
    }
}
```

- [ ] **Step 2: Run tests**

Run: `dotnet test ImageViewer.Tests\ImageViewer.Tests.csproj -c Debug -p:Platform=x64 --filter "FullyQualifiedName~CultureTests"`
Expected: both PASS. If `En_And_Fr_HaveIdenticalKeySets` fails, the failure message lists the divergent keys - this is a real localization bug to report (do not delete keys to make it pass without confirmation).

- [ ] **Step 3: Commit**

```bash
git add ImageViewer.Tests/CultureTests.cs
git commit -m "Test localization en/fr key parity and [KEY] fallback"
```

---

## Task 6: Test support (TempDir, FixtureFactory, async loader)

**Files:**
- Create: `build/ImageViewer.Tests/TestSupport.cs`

- [ ] **Step 1: Write the support code**

Create `build/ImageViewer.Tests/TestSupport.cs`:

```csharp
using System;
using System.IO;
using System.Threading.Tasks;

using SixLabors.ImageSharp;
using SixLabors.ImageSharp.Metadata.Profiles.Exif;
using SixLabors.ImageSharp.PixelFormats;

using ImageViewer.Wrapper;

using ViewerImage = ImageViewer.Wrapper.Image;

namespace ImageViewer.Tests;

/// <summary>Unique temp directory deleted on Dispose.</summary>
public sealed class TempDir : IDisposable
{
    public string Path { get; }

    public TempDir()
    {
        Path = System.IO.Path.Combine(System.IO.Path.GetTempPath(), "ImageViewerTests", Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(Path);
    }

    public string File(string name) => System.IO.Path.Combine(Path, name);

    public void Dispose()
    {
        try { Directory.Delete(Path, recursive: true); }
        catch { /* best effort cleanup */ }
    }
}

/// <summary>Generates image fixtures on disk so tests own no committed binaries.</summary>
public static class FixtureFactory
{
    public static string Save(TempDir dir, string fileName, int width, int height)
    {
        string path = dir.File(fileName);
        using Image<Rgba32> image = new(width, height);
        image.Save(path); // encoder inferred from the extension
        return path;
    }

    public static string SaveJpegOrientation6(TempDir dir, string fileName, int width, int height)
    {
        string path = dir.File(fileName);
        using Image<Rgba32> image = new(width, height);
        image.Metadata.ExifProfile = new ExifProfile();
        image.Metadata.ExifProfile.SetValue(ExifTag.Orientation, (ushort)6); // rotate 90 CW => width/height swap after AutoOrient
        image.SaveAsJpeg(path);
        return path;
    }

    public static string SaveSvg(TempDir dir, string fileName, int width, int height)
    {
        string path = dir.File(fileName);
        string svg = $"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\"><rect width=\"{width}\" height=\"{height}\" fill=\"red\"/></svg>";
        System.IO.File.WriteAllText(path, svg);
        return path;
    }

    public static string SaveIco(TempDir dir, string fileName)
    {
        // ImageSharp cannot write ICO; use System.Drawing to emit a valid icon the app's
        // System.Drawing-based ICO load path can read back.
        string path = dir.File(fileName);
        using System.Drawing.Bitmap bitmap = new(16, 16);
        using (System.Drawing.Graphics graphics = System.Drawing.Graphics.FromImage(bitmap))
        {
            graphics.Clear(System.Drawing.Color.Red);
        }

        IntPtr hIcon = bitmap.GetHicon();
        using System.Drawing.Icon icon = System.Drawing.Icon.FromHandle(hIcon);
        using FileStream stream = System.IO.File.Create(path);
        icon.Save(stream);
        return path;
    }
}

/// <summary>Bridges Wrapper.Image's event-based async load to an awaitable Task.</summary>
public static class ImageLoader
{
    public static async Task<ViewerImage> LoadAsync(string path, int timeoutMs = 15000)
    {
        ViewerImage image = new();
        TaskCompletionSource<bool> tcs = new(TaskCreationOptions.RunContinuationsAsynchronously);

        void OnLoaded(object sender, EventArgs e) => tcs.TrySetResult(true);
        void OnFailed(object sender, EventArgs e)
        {
            string message = (e as ImageFailedEventArgs)?.Message ?? "unknown error";
            tcs.TrySetException(new InvalidOperationException("Image load failed: " + message));
        }

        image.ImageLoaded += OnLoaded;
        image.ImageFailed += OnFailed;
        image.Load(path);

        Task completed = await Task.WhenAny(tcs.Task, Task.Delay(timeoutMs));
        if (completed != tcs.Task)
        {
            throw new TimeoutException("Image load timed out: " + path);
        }

        await tcs.Task; // surface any load exception
        return image;
    }
}
```

- [ ] **Step 2: Verify the support code compiles**

Run: `dotnet build ImageViewer.Tests\ImageViewer.Tests.csproj -c Debug -p:Platform=x64`
Expected: build succeeds. (No tests added yet; this just confirms the helpers and the `ImageSharp` / `System.Drawing` references compile.)

- [ ] **Step 3: Commit**

```bash
git add ImageViewer.Tests/TestSupport.cs
git commit -m "Add test support: TempDir, FixtureFactory, async image loader"
```

---

## Task 7: Wrapper.Image tests (load, AutoOrient, pixels, transform)

**Files:**
- Create: `build/ImageViewer.Tests/ImageTests.cs`

- [ ] **Step 1: Write the failing tests**

Create `build/ImageViewer.Tests/ImageTests.cs`:

```csharp
using System.Threading.Tasks;

using SixLabors.ImageSharp.Processing;

using Xunit;

using ViewerImage = ImageViewer.Wrapper.Image;

namespace ImageViewer.Tests;

public class ImageTests
{
    [Theory]
    [InlineData("sample.png", 4, 2)]
    [InlineData("sample.jpg", 4, 2)]
    [InlineData("sample.bmp", 4, 2)]
    [InlineData("sample.gif", 4, 2)]
    [InlineData("sample.tiff", 4, 2)]
    [InlineData("sample.webp", 4, 2)]
    [InlineData("sample.tga", 4, 2)]
    public async Task Load_NativeFormat_ReportsDimensions(string fileName, int width, int height)
    {
        using TempDir dir = new();
        string path = FixtureFactory.Save(dir, fileName, width, height);

        ViewerImage image = await ImageLoader.LoadAsync(path);
        try
        {
            Assert.True(image.Loaded);
            Assert.Equal(width, (int)image.Width);
            Assert.Equal(height, (int)image.Height);
        }
        finally
        {
            image.Dispose();
        }
    }

    [Fact]
    public async Task Load_Svg_Succeeds()
    {
        using TempDir dir = new();
        string path = FixtureFactory.SaveSvg(dir, "sample.svg", 32, 32);

        ViewerImage image = await ImageLoader.LoadAsync(path);
        try
        {
            Assert.True(image.Loaded);
            Assert.True(image.Width > 0 && image.Height > 0);
        }
        finally
        {
            image.Dispose();
        }
    }

    [Fact]
    public async Task Load_Ico_Succeeds()
    {
        using TempDir dir = new();
        string path = FixtureFactory.SaveIco(dir, "sample.ico");

        ViewerImage image = await ImageLoader.LoadAsync(path);
        try
        {
            Assert.True(image.Loaded);
            Assert.True(image.Width > 0 && image.Height > 0);
        }
        finally
        {
            image.Dispose();
        }
    }

    [Fact]
    public async Task Load_AppliesExifOrientation_SwappingWidthAndHeight()
    {
        using TempDir dir = new();
        // 4x2 with EXIF orientation 6 (rotate 90 CW) => AutoOrient yields 2x4.
        string path = FixtureFactory.SaveJpegOrientation6(dir, "exif.jpg", 4, 2);

        ViewerImage image = await ImageLoader.LoadAsync(path);
        try
        {
            Assert.Equal(2, (int)image.Width);
            Assert.Equal(4, (int)image.Height);
        }
        finally
        {
            image.Dispose();
        }
    }

    [Fact]
    public async Task GetBgra32Pixels_ReturnsExpectedSizeAndDimensions()
    {
        using TempDir dir = new();
        string path = FixtureFactory.Save(dir, "pixels.png", 4, 2);

        ViewerImage image = await ImageLoader.LoadAsync(path);
        try
        {
            byte[] pixels = image.GetBgra32Pixels(out int width, out int height);
            Assert.Equal(4, width);
            Assert.Equal(2, height);
            Assert.Equal(4 * 2 * 4, pixels.Length);
        }
        finally
        {
            image.Dispose();
        }
    }

    [Fact]
    public async Task RotateFlip_Rotate90_SwapsDimensions()
    {
        using TempDir dir = new();
        string path = FixtureFactory.Save(dir, "rotate.png", 4, 2);

        ViewerImage image = await ImageLoader.LoadAsync(path);
        try
        {
            image.RotateFlip(RotateMode.Rotate90, FlipMode.None);
            Assert.Equal(2, (int)image.Width);
            Assert.Equal(4, (int)image.Height);
        }
        finally
        {
            image.Dispose();
        }
    }
}
```

- [ ] **Step 2: Run tests**

Run: `dotnet test ImageViewer.Tests\ImageViewer.Tests.csproj -c Debug -p:Platform=x64 --filter "FullyQualifiedName~ImageTests"`
Expected: all cases PASS.

Troubleshooting:
- If `Load_Ico_Succeeds` fails because `System.Drawing.Image.FromStream` rejects the generated icon, replace the ICO body of `SaveIco` with a PNG-embedded icon, or fall back to committing a known-good `Fixtures/sample.ico` and `Content`-copying it; report the change.
- If a format's `Save` throws "no encoder", confirm the extension is in `Image.NativeExtensions`; `.tga` saves via ImageSharp's TgaEncoder.

- [ ] **Step 3: Run the full suite**

Run: `dotnet test ImageViewer.Tests\ImageViewer.Tests.csproj -c Debug -p:Platform=x64`
Expected: every test across all classes PASSES.

- [ ] **Step 4: Commit**

```bash
git add ImageViewer.Tests/ImageTests.cs
git commit -m "Test Wrapper.Image load, EXIF AutoOrient, pixels and transform"
```

---

## Final verification

- [ ] Full suite green: `dotnet test ImageViewer.Tests\ImageViewer.Tests.csproj -c Debug -p:Platform=x64`
- [ ] App still green, no new warnings: `dotnet build ImageViewer\ImageViewer.csproj -c Debug -p:Platform=x64`
- [ ] App still green in Release: `dotnet build ImageViewer\ImageViewer.csproj -c Release -p:Platform=x64`
