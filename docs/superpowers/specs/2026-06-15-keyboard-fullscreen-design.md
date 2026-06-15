# Design - Keyboard shortcuts (Home/End/Esc) + Fullscreen (F11)

Date: 2026-06-15
Status: Approved

## Goal

Add the missing navigation/UX keyboard shortcuts and a fullscreen mode:

- `Home` -> first image of the folder
- `End` -> last image of the folder
- `Esc` -> back out of the current mode (cropper -> info pane -> fullscreen), in that priority; no-op when nothing is open. Esc never quits the app.
- `F11` -> toggle fullscreen

`Space` was considered and dropped (user finds it useless).

## Decisions (locked)

- Bare-key accelerators at the `MainLayout` Grid level, matching the existing `Ctrl+C`/`Ctrl+V` pattern (`KeyboardAcceleratorPlacementMode="Hidden"`, so no localization is needed).
- All action logic lives in `Context` (per the architecture), except the fullscreen window-presenter mechanics, which are intrinsically window-level and live in `MainWindow`.
- Esc precedence is fixed: cropper, then info pane, then fullscreen.

## Existing facts this relies on (verified)

- `MainLayout` is a 4-row Grid: row 0 (`Height=48`, `AppTitleBar`), row 1 (`Auto`, unused), row 2 (`SplitViewContainer`), row 3 (`Height=48`, the footer toolbar Grid - currently unnamed).
- Cropper open state: `MainWindow.ImageCropperContainer.Visibility == Visibility.Visible` (set Visible on open, `Collapsed` by `Context.CloseCropper`).
- Info pane open state: `MainWindow.SplitViewContainer.IsPaneOpen` (bool).
- `AppWindow` is obtained via `MainWindow.GetAppWindowForCurrentWindow()` (currently private).
- Window geometry is tracked in `App` (`Window_SizeChanged` / `Window_PositionChanged`) and only persisted while `TrackedWindowState == WindowState.Normal`; flushed once by `App.SaveWindowGeometry()` on close.
- `Context.LoadNextImage` / `LoadPrevImage` already wrap around and handle dead files (`RemoveAtIndex` + retry).

## Components and changes

### `Context` (Helpers/Context.cs)

- `LoadFirstImage()` - guard `FolderFiles is { Length: > 0 }`; set `CurrentIndex = -1` then call `LoadNextImage()` (reuses the increment + dead-file handling so index 0 loads).
- `LoadLastImage()` - guard `FolderFiles is { Length: > 0 }`; set `CurrentIndex = 0` then call `LoadPrevImage()` (decrements to -1, wraps to `Length-1`, reuses dead-file handling).
- `EscapeAction()` - precedence:
  1. if `MainWindow.ImageCropperContainer.Visibility == Visibility.Visible` -> `CloseCropper()`; return.
  2. else if `MainWindow.SplitViewContainer.IsPaneOpen` -> set `IsPaneOpen = false`, `ScrollView.Focus(FocusState.Programmatic)`; return.
  3. else if `App.IsFullScreen` -> `MainWindow.SetFullScreen(false)`.
  4. else: no-op.

### `MainWindow` (MainWindow.xaml + .cs)

- XAML: add four Grid-level accelerators next to the existing two:
  `Escape` -> `Window_Escape`, `Home` -> `Window_Home`, `End` -> `Window_End`, `F11` -> `Window_FullScreen`.
- XAML: give the row-3 footer Grid `x:Name="FooterToolbar" x:FieldModifier="public"` (needed to hide it in fullscreen; public to match the codebase's XAML-access convention if referenced from Context, though here it is only touched in code-behind).
- Code-behind handlers (thin): `Window_Escape` -> `Context.Instance().EscapeAction()`; `Window_Home` -> `LoadFirstImage()`; `Window_End` -> `LoadLastImage()`; `Window_FullScreen` -> `ToggleFullScreen()`. Each sets `e.Handled = true`.
- `ToggleFullScreen()` -> `SetFullScreen(!App.IsFullScreen)`.
- `SetFullScreen(bool enabled)`:
  - On first use, capture the original row-0 and row-3 `GridLength`s into fields (`OriginalTitleBarRowHeight`, `OriginalFooterRowHeight`) so restore is correct even on the non-customizable-titlebar path (row 0 = `Auto`).
  - enabled: `GetAppWindowForCurrentWindow().SetPresenter(AppWindowPresenterKind.FullScreen)`; collapse `AppTitleBar` and `FooterToolbar`; set rows 0 and 3 heights to `new GridLength(0)`.
  - disabled: `SetPresenter(AppWindowPresenterKind.Default)`; restore `AppTitleBar`/`FooterToolbar` visibility and the captured row heights; call `RedrawTitleBar()` to reassert custom title-bar colors.
  - set `App.IsFullScreen = enabled` last.

### `App` (App.xaml.cs)

- `public static bool IsFullScreen;`
- Guard at the top of `Window_SizeChanged` and `Window_PositionChanged`: `if (IsFullScreen) return;` so fullscreen bounds are never persisted as the normal window geometry.

## Out of scope

- Saving/persisting fullscreen as a startup state (always start windowed).
- Auto-hiding the toolbar on mouse-idle / overlay chrome.
- Any change to existing shortcuts.

## Testing

These are UI/window behaviors; the existing xUnit project covers UI-free logic only and is not extended here (the new `Context` methods depend on the UI load path and private folder state). Verification is manual:

1. Open an image in a folder of several; `Home` jumps to the first, `End` to the last (natural sort order).
2. Open the crop pane, press `Esc` -> crop closes, image restored.
3. Open the info pane, press `Esc` -> pane closes.
4. `F11` -> window goes fullscreen, title bar and footer hidden; `F11` again -> restores windowed with title bar/footer and correct size.
5. In fullscreen, `Esc` -> exits fullscreen.
6. Enter fullscreen, exit, then close the app and reopen -> window size/position are the pre-fullscreen values (geometry not corrupted).
7. `Esc` with nothing open -> nothing happens (app stays open).

## Acceptance

- App builds green Debug and Release (`TreatWarningsAsErrors` on).
- Existing 25 tests still pass.
- Manual checklist above passes.
