# Save options dialog + direct Save Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a quality slider dialog to "Save As" for JPEG/WebP, and a direct "Save" (Ctrl+S) that overwrites the current file only when it has unsaved transforms.

**Architecture:** Keep the native `FileSavePicker`, then show a `ContentDialog` with a quality slider for lossy formats (JPEG/WebP). Track a `Modified` flag on `Wrapper.Image` (set by transforms, cleared by save) to gate the direct Save. Reassign keyboard shortcuts: Ctrl+S becomes direct Save, Ctrl+Shift+S becomes Save As.

**Tech Stack:** C# 12, .NET 10, WinUI 3 (Windows App SDK 1.8.x), SixLabors.ImageSharp 3.x, xUnit.

**Build check:** `dotnet build ImageViewer\ImageViewer.csproj -c Debug -p:Platform=x64` (warnings are errors).
**Test check:** `dotnet test ImageViewer.Tests\ImageViewer.Tests.csproj -c Debug -p:Platform=x64`.
All commands run from `D:\Seafile\Developpements\Windows\ImageViewer\build`.

## File Structure

- `ImageViewer/Wrapper/Image.cs` - add `Modified` property; extend `Save` with optional quality; set `Modified=false` after save.
- `ImageViewer/Wrapper/Image.Transform.cs` - set `Modified=true` in `RotateFlip`/`Crop`/`Resize`.
- `ImageViewer/Helpers/Context.cs` - quality dialog, `SaveAs` integration, new `Save()`, enable the new menu item.
- `ImageViewer/MainWindow.xaml` - new menu item + accelerator reassignment.
- `ImageViewer/MainWindow.xaml.cs` - new click handler.
- `ImageViewer/Strings/<tag>/Resources.resw` (six languages) - rename one key, add four keys.
- `ImageViewer.Tests/TestSupport.cs` - add a noisy-image fixture.
- `ImageViewer.Tests/ImageTests.cs` - tests for quality + Modified.

---

## Task 1: `Modified` flag on `Wrapper.Image`

**Files:**
- Modify: `ImageViewer/Wrapper/Image.cs` (add property, clear in `Save`)
- Modify: `ImageViewer/Wrapper/Image.Transform.cs` (set in transforms)
- Modify: `ImageViewer.Tests/TestSupport.cs` (noisy fixture, used here + Task 2)
- Test: `ImageViewer.Tests/ImageTests.cs`

- [ ] **Step 1: Add a deterministic noisy fixture** (needed for the quality test in Task 2; added here so the helper exists once)

In `ImageViewer.Tests/TestSupport.cs`, inside `FixtureFactory`, add after `Save`:

```csharp
public static string SaveNoisy(TempDir dir, string fileName, int width, int height)
{
    string path = dir.File(fileName);
    using Image<Rgba32> image = new(width, height);
    image.ProcessPixelRows(accessor =>
    {
        for (int y = 0; y < accessor.Height; y++)
        {
            Span<Rgba32> row = accessor.GetRowSpan(y);
            for (int x = 0; x < row.Length; x++)
            {
                // High-frequency deterministic pattern so JPEG/WebP quality changes file size.
                byte r = (byte)((x * 37 + y * 17) & 0xFF);
                byte g = (byte)((x * 13 + y * 53) & 0xFF);
                byte b = (byte)((x * 91 + y * 7) & 0xFF);
                row[x] = new Rgba32(r, g, b, 255);
            }
        }
    });
    image.Save(path);
    return path;
}
```

Add `using SixLabors.ImageSharp.Advanced;` to the top of `TestSupport.cs` if `ProcessPixelRows` is unresolved (it lives in the base ImageSharp namespace `SixLabors.ImageSharp`, already imported - only add if the build complains).

- [ ] **Step 2: Write the failing test** in `ImageViewer.Tests/ImageTests.cs` (append inside the class):

```csharp
[Fact]
public async Task Modified_FalseAfterLoad_TrueAfterTransform_FalseAfterSave()
{
    using TempDir dir = new();
    string path = FixtureFactory.Save(dir, "modflag.png", 4, 2);

    ViewerImage image = await ImageLoader.LoadAsync(path);
    try
    {
        Assert.False(image.Modified);

        image.RotateFlip(RotateMode.Rotate90, FlipMode.None);
        Assert.True(image.Modified);

        await image.Save(dir.File("modflag-out.png"), ".png");
        Assert.False(image.Modified);
    }
    finally
    {
        image.Dispose();
    }
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `dotnet test ImageViewer.Tests\ImageViewer.Tests.csproj -c Debug -p:Platform=x64 --filter "FullyQualifiedName~Modified_FalseAfterLoad"`
Expected: FAIL - `Image` has no `Modified` member (compile error).

- [ ] **Step 4: Add the `Modified` property and clear it in `Save`**

In `ImageViewer/Wrapper/Image.cs`, add the property near the other public state (after `IsAnimated`):

```csharp
public bool Modified { get; private set; }
```

At the very end of the `Save` method body (after the `switch`, before the method returns), add:

```csharp
Modified = false;
```

- [ ] **Step 5: Set `Modified` in the transforms**

In `ImageViewer/Wrapper/Image.Transform.cs`, set the flag at the end of each method:

```csharp
public void RotateFlip(RotateMode rotateMode, FlipMode flipMode)
{
    if(!WorkingImageLoaded) return;
    WorkingImage.Mutate(ctx => ctx.RotateFlip(rotateMode, flipMode));
    Modified = true;
}

public void Resize(int width, int height, IResampler mode)
{
    if(!WorkingImageLoaded) return;
    WorkingImage.Mutate(ctx => ctx.Resize(width, height, mode));
    Modified = true;
}

public void Crop(int x, int y, int cropWidth, int cropHeight)
{
    if(!WorkingImageLoaded) return;
    WorkingImage.Mutate(ctx => ctx.Crop(new Rectangle(x, y, cropWidth, cropHeight)));
    Modified = true;
}
```

- [ ] **Step 6: Run the test to verify it passes**

Run: `dotnet test ImageViewer.Tests\ImageViewer.Tests.csproj -c Debug -p:Platform=x64 --filter "FullyQualifiedName~Modified_FalseAfterLoad"`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add ImageViewer/Wrapper/Image.cs ImageViewer/Wrapper/Image.Transform.cs ImageViewer.Tests/TestSupport.cs ImageViewer.Tests/ImageTests.cs
git commit -m "Track Modified state on Wrapper.Image (set by transforms, cleared by save)"
```

---

## Task 2: Quality parameter on `Image.Save` (JPEG + WebP)

**Files:**
- Modify: `ImageViewer/Wrapper/Image.cs:140-175` (the `Save` method)
- Test: `ImageViewer.Tests/ImageTests.cs`

- [ ] **Step 1: Write the failing test** in `ImageViewer.Tests/ImageTests.cs` (append inside the class):

```csharp
[Theory]
[InlineData(".jpg")]
[InlineData(".webp")]
public async Task Save_LowerQuality_ProducesSmallerFile(string type)
{
    using TempDir dir = new();
    string path = FixtureFactory.SaveNoisy(dir, "src.png", 64, 64);

    ViewerImage image = await ImageLoader.LoadAsync(path);
    try
    {
        string high = dir.File("high" + type);
        string low = dir.File("low" + type);

        await image.Save(high, type, 95);
        await image.Save(low, type, 20);

        long highSize = new System.IO.FileInfo(high).Length;
        long lowSize = new System.IO.FileInfo(low).Length;

        Assert.True(lowSize < highSize, $"expected q20 ({lowSize}) < q95 ({highSize})");
    }
    finally
    {
        image.Dispose();
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `dotnet test ImageViewer.Tests\ImageViewer.Tests.csproj -c Debug -p:Platform=x64 --filter "FullyQualifiedName~Save_LowerQuality"`
Expected: FAIL - `Save` takes only two arguments (compile error).

- [ ] **Step 3: Add the quality parameter to `Save`**

In `ImageViewer/Wrapper/Image.cs`, add the WebP encoder using-directive at the top with the other format usings:

```csharp
using SixLabors.ImageSharp.Formats.Webp;
```

Change the `Save` signature and the JPEG/WebP branches:

```csharp
public async Task Save(string path, string type, int? quality = null)
{
    switch(type)
    {
        case ".jpg":
            await WorkingImage.SaveAsJpegAsync(path, new JpegEncoder { Quality = quality ?? 100 });
            break;

        case ".png":
            await WorkingImage.SaveAsPngAsync(path);
            break;

        case ".webp":
            await WorkingImage.SaveAsWebpAsync(path, new WebpEncoder { Quality = quality ?? 100 });
            break;

        case ".bmp":
            await WorkingImage.SaveAsBmpAsync(path);
            break;

        case ".gif":
            await WorkingImage.SaveAsGifAsync(path);
            break;

        case ".tga":
            await WorkingImage.SaveAsTgaAsync(path);
            break;

        case ".tiff":
            await WorkingImage.SaveAsTiffAsync(path);
            break;

        default:
            throw new NotSupportedException($"Unsupported save format: {type}");
    }

    Modified = false;
}
```

(The `Modified = false;` line was added in Task 1; keep it as the last statement.)

- [ ] **Step 4: Run the test to verify it passes**

Run: `dotnet test ImageViewer.Tests\ImageViewer.Tests.csproj -c Debug -p:Platform=x64 --filter "FullyQualifiedName~Save_LowerQuality"`
Expected: PASS (both JPEG and WebP cases).

- [ ] **Step 5: Run the full test suite** (no regressions)

Run: `dotnet test ImageViewer.Tests\ImageViewer.Tests.csproj -c Debug -p:Platform=x64`
Expected: PASS, all tests.

- [ ] **Step 6: Commit**

```bash
git add ImageViewer/Wrapper/Image.cs ImageViewer.Tests/ImageTests.cs
git commit -m "Honor an optional quality on Image.Save for JPEG and WebP"
```

---

## Task 3: Localization keys (six `.resw` files)

**Files (all under `ImageViewer/Strings/<tag>/Resources.resw`):** `en-US`, `fr-FR`, `zh-Hans`, `de-DE`, `es-ES`, `it-IT`.

For each language, do the same three edits. Per project convention: ASCII punctuation only (straight quotes, hyphen), but accented letters are kept.

- [ ] **Step 1: Rename the existing "Save as..." key**

In every `Resources.resw`, find:

```xml
<data name="FOOTER_TOOLBAR_MENU_FILE_SAVE" xml:space="preserve">
```

Rename the attribute to `FOOTER_TOOLBAR_MENU_FILE_SAVE_AS` (keep each file's existing `<value>` unchanged - "Save as..." / "Enregistrer sous..." / "另存为..." / "Speichern unter..." / "Guardar como..." / "Salva con nome...").

- [ ] **Step 2: Add the new "Save" key**

Add a new `<data>` element in each file, using that language's value:

| tag | value |
|-----|-------|
| en-US | `Save` |
| fr-FR | `Enregistrer` |
| zh-Hans | `保存` |
| de-DE | `Speichern` |
| es-ES | `Guardar` |
| it-IT | `Salva` |

```xml
<data name="FOOTER_TOOLBAR_MENU_FILE_SAVE" xml:space="preserve">
  <value>Save</value>
</data>
```

- [ ] **Step 3: Add the dialog keys** (`SAVE_OPTIONS_TITLE`, `SAVE_OPTIONS_QUALITY`, `SYSTEM_CANCEL`)

| key | en-US | fr-FR | zh-Hans | de-DE | es-ES | it-IT |
|-----|-------|-------|---------|-------|-------|-------|
| SAVE_OPTIONS_TITLE | Save options | Options d'enregistrement | 保存选项 | Speicheroptionen | Opciones de guardado | Opzioni di salvataggio |
| SAVE_OPTIONS_QUALITY | Quality | Qualité | 质量 | Qualität | Calidad | Qualità |
| SYSTEM_CANCEL | Cancel | Annuler | 取消 | Abbrechen | Cancelar | Annulla |

Each as:

```xml
<data name="SAVE_OPTIONS_TITLE" xml:space="preserve">
  <value>Save options</value>
</data>
```

- [ ] **Step 4: Verify key parity** (the xUnit `CultureTests` enforces identical key sets)

Run: `dotnet test ImageViewer.Tests\ImageViewer.Tests.csproj -c Debug -p:Platform=x64 --filter "FullyQualifiedName~Culture"`
Expected: PASS. A FAIL here means a key is missing/typoed in one language - fix it.

- [ ] **Step 5: Commit**

```bash
git add ImageViewer/Strings
git commit -m "Add localization keys for Save item, Save options dialog, Cancel"
```

---

## Task 4: Quality dialog + `SaveAs` integration (`Context`)

**Files:**
- Modify: `ImageViewer/Helpers/Context.cs` (`SaveAs` ~677, new dialog helper)

- [ ] **Step 1: Add the quality dialog helper** to `ImageViewer/Helpers/Context.cs` (place it just after `SaveAs`):

```csharp
/// <summary>
/// Asks the user for an encoding quality (1-100) for lossy formats.
/// Returns the chosen value, or null if the user cancelled.
/// </summary>
private async Task<int?> ShowSaveQualityDialog()
{
    Slider qualitySlider = new()
    {
        Minimum = 1,
        Maximum = 100,
        Value = 100,
        StepFrequency = 1,
        TickFrequency = 10,
        Header = Culture.GetString("SAVE_OPTIONS_QUALITY")
    };

    Microsoft.UI.Xaml.Controls.ContentDialog dialog = new()
    {
        XamlRoot = MainWindow.Content.XamlRoot,
        RequestedTheme = ((FrameworkElement)MainWindow.Content).ActualTheme,
        Title = Culture.GetString("SAVE_OPTIONS_TITLE"),
        Content = qualitySlider,
        PrimaryButtonText = Culture.GetString("SYSTEM_OK"),
        CloseButtonText = Culture.GetString("SYSTEM_CANCEL"),
        DefaultButton = Microsoft.UI.Xaml.Controls.ContentDialogButton.Primary
    };

    Microsoft.UI.Xaml.Controls.ContentDialogResult result = await dialog.ShowAsync();
    if (result != Microsoft.UI.Xaml.Controls.ContentDialogResult.Primary) return null;

    return (int)qualitySlider.Value;
}
```

If `Slider` is unresolved, add `using Microsoft.UI.Xaml.Controls;` only if it is not already imported; otherwise reference `Microsoft.UI.Xaml.Controls.Slider` inline to avoid an ambiguity with any existing `Slider` import. (Check the existing using block first.)

- [ ] **Step 2: Wire the dialog into `SaveAs`**

In `SaveAs`, replace the save call section:

```csharp
        if (!Image.SaveFileTypes.Contains(outputFileType)) return false;

        try
        {
            await CurrentImage.Save(outputFile.Path, outputFileType);
        }
```

with quality handling for lossy formats:

```csharp
        if (!Image.SaveFileTypes.Contains(outputFileType)) return false;

        int? quality = null;
        if (outputFileType is ".jpg" or ".webp")
        {
            quality = await ShowSaveQualityDialog();
            if (quality == null) return false; // user cancelled the options dialog
        }

        try
        {
            await CurrentImage.Save(outputFile.Path, outputFileType, quality);
        }
```

(The rest of `SaveAs` - catch block, `LoadDirectoryFiles()`, `return true` - is unchanged.)

- [ ] **Step 3: Build to verify it compiles**

Run: `dotnet build ImageViewer\ImageViewer.csproj -c Debug -p:Platform=x64`
Expected: Build succeeded, 0 warnings (warnings are errors).

- [ ] **Step 4: Commit**

```bash
git add ImageViewer/Helpers/Context.cs
git commit -m "Show a quality options dialog before Save As for JPEG and WebP"
```

---

## Task 5: Direct `Save()` (overwrite current file)

**Files:**
- Modify: `ImageViewer/Helpers/Context.cs` (new `Save` method; enable line in `UpdateButtonsAccessiblity`)

- [ ] **Step 1: Add the `Save` method** to `ImageViewer/Helpers/Context.cs` (place it just before `SaveAs`):

```csharp
/// <summary>
/// Overwrite the current file in place. No-op unless the image was modified.
/// Falls back to Save As for pasted images and non-writable source formats (svg/ico).
/// </summary>
public async Task<bool> Save()
{
    if (!HasImageLoaded() || !CurrentImage.Modified) return false;

    // Pasted content has no source file: route to Save As.
    if (CurrentFilePath == null) return await SaveAs();

    string type = Path.GetExtension(CurrentFilePath).ToLowerInvariant();
    if (type == ".jpeg") type = ".jpg";
    if (type == ".tif") type = ".tiff";

    // Source format we cannot re-encode (svg/ico): route to Save As.
    if (!Image.SaveFileTypes.Contains(type)) return await SaveAs();

    try
    {
        await CurrentImage.Save(CurrentFilePath, type);
    }
    catch (Exception ex)
    {
        Debug.WriteLine($"Save failed: {ex.Message}");

        Microsoft.UI.Xaml.Controls.ContentDialog errorDialog = new()
        {
            XamlRoot = MainWindow.Content.XamlRoot,
            RequestedTheme = ((FrameworkElement)MainWindow.Content).ActualTheme,
            Content = Culture.GetString("SYSTEM_SAVING_ERROR"),
            CloseButtonText = Culture.GetString("SYSTEM_OK")
        };

        await errorDialog.ShowAsync();
        return false;
    }

    UpdateButtonsAccessiblity();
    return true;
}
```

- [ ] **Step 2: Enable the new menu item in `UpdateButtonsAccessiblity`**

In `ImageViewer/Helpers/Context.cs`, inside `UpdateButtonsAccessiblity`:

In the `HasImageLoaded()` true branch, after `MainWindow.ButtonFileSave.IsEnabled = true;` add:

```csharp
            MainWindow.ButtonFileSaveDirect.IsEnabled = CurrentImage.Modified;
```

In the `else` (no image) branch, after `MainWindow.ButtonFileSave.IsEnabled = false;` add:

```csharp
            MainWindow.ButtonFileSaveDirect.IsEnabled = false;
```

In the cropper-open block, after `MainWindow.ButtonFileSave.IsEnabled = false;` add:

```csharp
            MainWindow.ButtonFileSaveDirect.IsEnabled = false;
```

(`ButtonFileSaveDirect` is added to the XAML in Task 6. This task will not build until Task 6 is done; that is expected - build verification happens at the end of Task 6.)

- [ ] **Step 3: Commit**

```bash
git add ImageViewer/Helpers/Context.cs
git commit -m "Add Context.Save() direct overwrite, gated on Modified state"
```

---

## Task 6: Menu item, handler, and shortcut reassignment (`MainWindow`)

**Files:**
- Modify: `ImageViewer/MainWindow.xaml` (menu items ~199-206)
- Modify: `ImageViewer/MainWindow.xaml.cs` (handler ~457)

- [ ] **Step 1: Add the new "Save" menu item and reassign accelerators** in `ImageViewer/MainWindow.xaml`

Find the existing item (around line 199):

```xml
<MenuFlyoutItem x:Name="ButtonFileSave" x:FieldModifier="public" Text="{x:Bind helpers:Culture.GetString('FOOTER_TOOLBAR_MENU_FILE_SAVE')}" IsEnabled="False" Click="ButtonFileSave_Click">
    <MenuFlyoutItem.Icon>
        <FontIcon Glyph="&#xEA35;" />
    </MenuFlyoutItem.Icon>
    <MenuFlyoutItem.KeyboardAccelerators>
        <KeyboardAccelerator Key="S" Modifiers="Control" />
    </MenuFlyoutItem.KeyboardAccelerators>
</MenuFlyoutItem>
```

Replace that whole block with the new "Save" item (Ctrl+S) followed by the relabeled "Save as..." item (Ctrl+Shift+S):

```xml
<MenuFlyoutItem x:Name="ButtonFileSaveDirect" x:FieldModifier="public" Text="{x:Bind helpers:Culture.GetString('FOOTER_TOOLBAR_MENU_FILE_SAVE')}" IsEnabled="False" Click="ButtonFileSaveDirect_Click">
    <MenuFlyoutItem.Icon>
        <FontIcon Glyph="&#xE74E;" />
    </MenuFlyoutItem.Icon>
    <MenuFlyoutItem.KeyboardAccelerators>
        <KeyboardAccelerator Key="S" Modifiers="Control" />
    </MenuFlyoutItem.KeyboardAccelerators>
</MenuFlyoutItem>
<MenuFlyoutItem x:Name="ButtonFileSave" x:FieldModifier="public" Text="{x:Bind helpers:Culture.GetString('FOOTER_TOOLBAR_MENU_FILE_SAVE_AS')}" IsEnabled="False" Click="ButtonFileSave_Click">
    <MenuFlyoutItem.Icon>
        <FontIcon Glyph="&#xEA35;" />
    </MenuFlyoutItem.Icon>
    <MenuFlyoutItem.KeyboardAccelerators>
        <KeyboardAccelerator Key="S" Modifiers="Control,Shift" />
    </MenuFlyoutItem.KeyboardAccelerators>
</MenuFlyoutItem>
```

(Glyph `E74E` is the Segoe "Save" icon for the direct Save; `EA35` stays on Save As.)

- [ ] **Step 2: Add the click handler** in `ImageViewer/MainWindow.xaml.cs` (next to `ButtonFileSave_Click` ~457):

```csharp
private async void ButtonFileSaveDirect_Click(object sender, RoutedEventArgs e)
{
    await Context.Instance().Save();
}
```

Match the exact shape of the existing `ButtonFileSave_Click` (same `try`/`catch` or guard pattern it uses - mirror it). If `ButtonFileSave_Click` only awaits `SaveAs()` inside a guard, mirror that guard.

- [ ] **Step 3: Build the app**

Run: `dotnet build ImageViewer\ImageViewer.csproj -c Debug -p:Platform=x64`
Expected: Build succeeded, 0 warnings. This is the first build that includes Task 5's `ButtonFileSaveDirect` reference - it must resolve now.

- [ ] **Step 4: Run the full test suite**

Run: `dotnet test ImageViewer.Tests\ImageViewer.Tests.csproj -c Debug -p:Platform=x64`
Expected: PASS, all tests (including `CultureTests` parity).

- [ ] **Step 5: Commit**

```bash
git add ImageViewer/MainWindow.xaml ImageViewer/MainWindow.xaml.cs
git commit -m "Add Save menu item (Ctrl+S); move Save As to Ctrl+Shift+S"
```

---

## Task 7: Manual verification (UI)

These cannot be unit-tested (the test host never boots WinUI). Run the Debug app and confirm:

- [ ] Open a JPEG, crop or rotate it, press **Ctrl+S** -> file overwrites silently, no dialog; the "Save" menu item greys out again afterward.
- [ ] Open an unmodified JPEG -> "Save" menu item is greyed; Ctrl+S does nothing.
- [ ] **Save As** (Ctrl+Shift+S) a JPEG -> after picking the file, the quality dialog appears; choosing a low quality yields a visibly smaller/lossier file; Cancel aborts (no file written).
- [ ] Save As a PNG/BMP/GIF/TIFF/TGA -> no quality dialog, saves directly.
- [ ] Paste an image (Ctrl+V), modify it, Ctrl+S -> falls back to the Save As picker.
- [ ] Open an `.svg` or `.ico`, modify it, Ctrl+S -> falls back to the Save As picker.
- [ ] Switch language to French/German/etc. in Settings -> the new menu item and dialog are localized.

---

## Self-review notes

- Spec coverage: Modified flag (T1), quality on Save (T2), localization incl. key rename (T3), quality dialog + SaveAs (T4), direct Save + edge fallbacks + enable logic (T5), menu/handler/shortcut reassignment (T6), manual UI checks (T7). All spec sections covered.
- Type consistency: `Save(string, string, int?)`, `ShowSaveQualityDialog() -> Task<int?>`, `Save() -> Task<bool>`, `Modified` (get; private set;), menu name `ButtonFileSaveDirect`, handler `ButtonFileSaveDirect_Click`, keys `FOOTER_TOOLBAR_MENU_FILE_SAVE` (Save) / `FOOTER_TOOLBAR_MENU_FILE_SAVE_AS` (Save as...) / `SAVE_OPTIONS_TITLE` / `SAVE_OPTIONS_QUALITY` / `SYSTEM_CANCEL` - consistent across tasks.
- Build ordering: Tasks 5 references `ButtonFileSaveDirect` defined in Task 6; both commit independently but the build is green only after Task 6 Step 3. Noted in-task.
