# Design - Save options dialog (JPEG/WebP quality) + direct Save (Ctrl+S)

Date: 2026-06-15
Status: Approved

## Goal

Two related improvements to the save flow (ROADMAP "Save As options dialog", Horizon 2):

1. **Quality control on "Save As"** - JPEG quality is currently hard-coded to 100 and WebP/PNG have no control. After the file picker, show a small options dialog with a quality slider for lossy formats (JPEG and WebP only). Other formats save directly, no dialog.
2. **Direct "Save" (overwrite current file)** - a `Ctrl+S` that re-encodes over the current file, available only when the image has unsaved transforms (crop / rotate / flip). Silent, quality 100.

Scope is intentionally minimal: quality slider for JPEG and WebP only. PNG compression, WebP lossless toggle, and per-format defaults in Settings are explicitly out of scope (YAGNI).

## Decisions (locked)

- **Approach A** (most image editors): keep the native `FileSavePicker` for name/folder/format, then open a `ContentDialog` with the quality options. The native picker cannot host custom controls, so the options live in a follow-up dialog.
- Quality dialog appears for **JPEG and WebP only**. Default quality value = **100**. Cancel aborts the save.
- Direct Save re-encodes **silently at quality 100** (no dialog), matching today's JPEG behavior.
- Direct Save is **active only when the current image is modified** (a transform was applied since load/last save).
- `Ctrl+S` is reassigned: today it is bound to "Save as...". It moves to the new **Save** item. "Save as..." moves to **Ctrl+Shift+S** (conventional).
- Edge cases route to "Save As" instead of failing:
  - Pasted image (`CurrentFilePath == null`) + Save -> falls back to `SaveAs()`.
  - Current format not re-writable (`.svg`, `.ico`, i.e. not in `SaveFileTypes`) + Save -> falls back to `SaveAs()`.

## Existing facts this relies on (verified)

- `Context.SaveAs()` (Context.cs ~677): opens `FileSavePicker`, adds `Image.SaveFileTypes` choices, then `await CurrentImage.Save(outputFile.Path, outputFileType)`, catches and shows `SYSTEM_SAVING_ERROR` dialog, then `LoadDirectoryFiles()`.
- `Image.Save(string path, string type)` (Wrapper/Image.cs ~140): a `switch` on the extension; JPEG path is `new JpegEncoder { Quality = 100 }`; default throws `NotSupportedException`.
- `Image.SaveFileTypes = [".jpg", ".png", ".webp", ".bmp", ".gif", ".tiff", ".tga"]`. `.svg`/`.ico` are loadable but not in this list.
- Transforms mutate `WorkingImage` directly with **no modified tracking**: `Image.RotateFlip` / `Image.Crop` / `Image.Resize` (Image.Transform.cs); driven by `Context.RotateFlip` (~495) and `Context.Crop` (~505).
- The footer "hamburger" `MenuFlyout` (MainWindow.xaml ~181) contains one save entry: `ButtonFileSave` (`x:FieldModifier="public"`, `IsEnabled="False"`), bound to key `FOOTER_TOOLBAR_MENU_FILE_SAVE` (value "Save as..." / "Enregistrer sous..."), accelerator **Ctrl+S**, handler `ButtonFileSave_Click` (MainWindow.xaml.cs ~457) which calls `Context.SaveAs()`.
- `ButtonFileSave.IsEnabled` is toggled in Context's UI-state code (Context.cs lines ~531 true, ~549 / ~588 false) - the same block that enables/disables `ButtonImageTransform*`, `ButtonFileInfo`, etc.
- Localization is MRT `.resw`; six languages: `en-US`, `fr-FR`, `zh-Hans`, `de-DE`, `es-ES`, `it-IT`. `CultureTests` enforces identical key sets across all six.
- `SYSTEM_OK` key exists for dialog buttons.

## Components and changes

### `Wrapper.Image` (Image.cs + Image.Transform.cs)

- Add `public bool Modified { get; private set; }`.
- Set `Modified = true` at the end of `RotateFlip`, `Crop`, `Resize` (only when `WorkingImageLoaded`).
- Change `Save` signature to `Task Save(string path, string type, int? quality = null)`:
  - JPEG: `new JpegEncoder { Quality = quality ?? 100 }`.
  - WebP: `new WebpEncoder { Quality = quality ?? 100 }` (add `using SixLabors.ImageSharp.Formats.Webp;`). Lossy mode (the encoder default).
  - All other branches unchanged.
  - On success (end of method), set `Modified = false`.

### `Context` (Helpers/Context.cs)

- `SaveAs()`:
  - After `outputFileType` is validated, if it is `.jpg` or `.webp`, call the new quality dialog (default 100). If the dialog is cancelled, return false (abort, do not encode). Pass the chosen quality to `Save`.
  - Other formats: call `Save(path, type)` as today (quality null).
- New `Task<bool> Save()` (direct overwrite):
  - `if (!HasImageLoaded() || CurrentImage is { Modified: false })` -> return false (no-op).
  - `if (CurrentFilePath == null)` -> `return await SaveAs();` (pasted image).
  - Compute `type` from `Path.GetExtension(CurrentFilePath).ToLowerInvariant()`, normalize `.jpeg` -> `.jpg`, `.tif` -> `.tiff`.
  - `if (!Image.SaveFileTypes.Contains(type))` -> `return await SaveAs();` (svg/ico).
  - Else: `await CurrentImage.Save(CurrentFilePath, type)` (quality 100), inside the same try/catch + `SYSTEM_SAVING_ERROR` dialog pattern as `SaveAs`. On success refresh UI state so the Save item disables again (Modified now false). No `LoadDirectoryFiles()` needed (file set unchanged) but harmless; keep consistent with SaveAs - call it.
- New `Task<int?> ShowSaveQualityDialog()` helper (or inline in `SaveAs`): builds a `ContentDialog` (XamlRoot + RequestedTheme like the existing error dialog) containing a `Slider` (Minimum 1, Maximum 100, Value 100, `StepFrequency=1`, value shown), PrimaryButton = save (`SYSTEM_OK`), CloseButton = cancel. Returns the slider value on primary, `null` on cancel.
- UI-state block: add `MainWindow.ButtonFileSaveDirect.IsEnabled = HasImageLoaded() && CurrentImage.Modified;` alongside the existing toggles, and set it false in the disabled branches. Call this UI-state refresh at the end of `RotateFlip` and `Crop` so the Save item lights up after a transform.

### `MainWindow` (MainWindow.xaml + .cs)

- Existing `ButtonFileSave` item: keep handler/`SaveAs`, change accelerator to `Ctrl+Shift+S`, rebind its label to the new "Save as..." key (see Localization).
- New `MenuFlyoutItem x:Name="ButtonFileSaveDirect" x:FieldModifier="public" IsEnabled="False"` placed just above `ButtonFileSave`, label = new "Save" key, icon = a save glyph, accelerator **Ctrl+S**, handler `ButtonFileSaveDirect_Click` -> `await Context.Instance().Save()`.
- Order in menu: Save (direct), Save as..., then existing Delete etc.

### Localization (`Strings/<tag>/Resources.resw`, all six)

- Rename key `FOOTER_TOOLBAR_MENU_FILE_SAVE` -> `FOOTER_TOOLBAR_MENU_FILE_SAVE_AS` (value unchanged: "Save as..." / "Enregistrer sous..." / etc.); update the XAML `x:Bind` on the existing item.
- Add `FOOTER_TOOLBAR_MENU_FILE_SAVE` = "Save" / "Enregistrer" / (zh, de, es, it).
- Add `SAVE_OPTIONS_TITLE` = "Save options" / "Options d'enregistrement" / (zh, de, es, it).
- Add `SAVE_OPTIONS_QUALITY` = "Quality" / "Qualite" / (zh, de, es, it). French value keeps its accent ("Qualite" with the accent); only Unicode punctuation is banned, accented letters stay.
- Keep all six language key sets identical (CultureTests).

## Testing

- Extend `ImageViewer.Tests` (UI-free only):
  - `Image.Save` round-trip for JPEG at a low quality vs 100 produces a smaller file at low quality (quality is honored); WebP likewise.
  - `Image.Save` to JPEG/WebP/PNG round-trips and reloads with expected dimensions.
  - `Modified` is false after load, true after `RotateFlip`/`Crop`, false again after `Save`.
- The dialog, menu wiring, and accelerators are UI and are verified manually (the test host never boots WinUI).

## Out of scope (YAGNI)

- PNG compression-level control, WebP lossless toggle.
- Per-format default quality stored in Settings.
- A custom non-native save window (Approach B).
- Slideshow, metadata panel, and other ROADMAP items.
