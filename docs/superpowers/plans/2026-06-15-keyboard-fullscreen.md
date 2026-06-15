# Keyboard shortcuts + Fullscreen Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `Home`/`End`/`Esc` shortcuts and an `F11` fullscreen toggle to ImageViewer.

**Architecture:** Bare-key accelerators at the `MainLayout` Grid level delegate to thin code-behind handlers that call `Context` methods (action logic) and `MainWindow` methods (window-presenter mechanics). A static `App.IsFullScreen` flag protects saved window geometry from fullscreen bounds.

**Tech Stack:** WinUI 3 / .NET 10, C# 12, `Microsoft.UI.Windowing.AppWindowPresenterKind`.

---

## File structure

- Modify: `ImageViewer/App.xaml.cs` - `IsFullScreen` flag + geometry guards.
- Modify: `ImageViewer/Helpers/Context.cs` - `LoadFirstImage`, `LoadLastImage`, `EscapeAction`.
- Modify: `ImageViewer/MainWindow.xaml.cs` - fullscreen methods + 4 accelerator handlers.
- Modify: `ImageViewer/MainWindow.xaml` - name the footer Grid + 4 accelerators.

**Conventions:** ASCII only (no curly quotes / em-dashes), English identifiers/comments, PascalCase private fields. Commit with the repo-local identity (Dragon / dragonofmercy@hotmail.com - already configured; do not set it, no --global). End every commit body with `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`. Do not push, no --amend/--no-verify. `TreatWarningsAsErrors` is ON.

**Commands:**
- App build: `dotnet build ImageViewer\ImageViewer.csproj -c Debug -p:Platform=x64`
- Tests: `dotnet test ImageViewer.Tests\ImageViewer.Tests.csproj -c Debug -p:Platform=x64`

---

## Task 1: App.IsFullScreen flag and geometry guards

**Files:**
- Modify: `ImageViewer/App.xaml.cs`

- [ ] **Step 1: Add the static flag**

In `App.xaml.cs`, in the `App` class, add next to the existing geometry tracking fields (after `private static uint? PendingSizeH;`):

```csharp
    // True while the window is in fullscreen; suppresses geometry persistence so the
    // fullscreen bounds are never saved as the normal window size/position.
    public static bool IsFullScreen;
```

- [ ] **Step 2: Guard the size/position trackers**

In `Window_SizeChanged`, add a first line inside the method body (before the existing `if(TrackedWindowState != WindowState.Normal) return;`):

```csharp
        if(IsFullScreen) return;
```

In `Window_PositionChanged`, likewise add as the first line of the method body:

```csharp
        if(IsFullScreen) return;
```

- [ ] **Step 3: Build**

Run: `dotnet build ImageViewer\ImageViewer.csproj -c Debug -p:Platform=x64`
Expected: 0 warnings, 0 errors.

- [ ] **Step 4: Commit**

```bash
git add ImageViewer/App.xaml.cs
git commit -m "Add App.IsFullScreen flag and guard window geometry persistence"
```

---

## Task 2: Wire shortcuts and fullscreen

This task adds the `Context` logic, the `MainWindow` fullscreen methods + accelerator handlers, and the XAML accelerators that invoke them - together, so every member is referenced (no unused-member build break) and the project compiles as a unit.

**Files:**
- Modify: `ImageViewer/Helpers/Context.cs`
- Modify: `ImageViewer/MainWindow.xaml.cs`
- Modify: `ImageViewer/MainWindow.xaml`

- [ ] **Step 1: Add Context navigation + escape methods**

In `Context.cs`, add these three methods (place them near `LoadPrevImage`, after the `LoadPrevImage` method that ends around line 206). They reuse the existing wrap-around + dead-file handling of `LoadNextImage`/`LoadPrevImage`. `App` resolves to `ImageViewer.App` via the enclosing namespace; `Visibility` and `FocusState` are already used elsewhere in this file:

```csharp
    /// <summary>
    /// Jump to the first image of the folder.
    /// </summary>
    public void LoadFirstImage()
    {
        if (FolderFiles is not { Length: > 0 }) return;

        // Land on index 0 via LoadNextImage so dead-file handling is reused
        CurrentIndex = -1;
        LoadNextImage();
    }

    /// <summary>
    /// Jump to the last image of the folder.
    /// </summary>
    public void LoadLastImage()
    {
        if (FolderFiles is not { Length: > 0 }) return;

        // Land on the last index via LoadPrevImage (wraps from 0 to Length-1)
        CurrentIndex = 0;
        LoadPrevImage();
    }

    /// <summary>
    /// Esc: back out of the current mode, by priority - cropper, then info pane, then fullscreen.
    /// No-op (never quits) when nothing is open.
    /// </summary>
    public void EscapeAction()
    {
        if (MainWindow == null) return;

        if (MainWindow.ImageCropperContainer.Visibility == Visibility.Visible)
        {
            CloseCropper();
            return;
        }

        if (MainWindow.SplitViewContainer.IsPaneOpen)
        {
            MainWindow.SplitViewContainer.IsPaneOpen = false;
            MainWindow.ScrollView.Focus(FocusState.Programmatic);
            return;
        }

        if (App.IsFullScreen)
        {
            MainWindow.SetFullScreen(false);
        }
    }
```

- [ ] **Step 2: Add MainWindow fullscreen methods + accelerator handlers**

In `MainWindow.xaml.cs`:

(a) Add fields near the other private fields (after `private bool ScrollViewMouseDrag;`):

```csharp
    private GridLength? OriginalTitleBarRowHeight;
    private GridLength? OriginalFooterRowHeight;
    private Visibility? OriginalTitleBarVisibility;
```

(b) Add the fullscreen methods (place them after `GetAppWindowForCurrentWindow`, around line 134):

```csharp
    public void ToggleFullScreen()
    {
        SetFullScreen(!App.IsFullScreen);
    }

    public void SetFullScreen(bool enabled)
    {
        AppWindow appWindow = GetAppWindowForCurrentWindow();

        // Capture the windowed layout once so it restores correctly (row 0 may be Auto when
        // the custom title bar is unsupported)
        OriginalTitleBarRowHeight ??= MainLayout.RowDefinitions[0].Height;
        OriginalFooterRowHeight ??= MainLayout.RowDefinitions[3].Height;
        OriginalTitleBarVisibility ??= AppTitleBar.Visibility;

        if(enabled)
        {
            appWindow.SetPresenter(AppWindowPresenterKind.FullScreen);

            AppTitleBar.Visibility = Visibility.Collapsed;
            FooterToolbar.Visibility = Visibility.Collapsed;
            MainLayout.RowDefinitions[0].Height = new GridLength(0);
            MainLayout.RowDefinitions[3].Height = new GridLength(0);
        }
        else
        {
            appWindow.SetPresenter(AppWindowPresenterKind.Default);

            AppTitleBar.Visibility = OriginalTitleBarVisibility.Value;
            FooterToolbar.Visibility = Visibility.Visible;
            MainLayout.RowDefinitions[0].Height = OriginalTitleBarRowHeight.Value;
            MainLayout.RowDefinitions[3].Height = OriginalFooterRowHeight.Value;

            RedrawTitleBar();
        }

        App.IsFullScreen = enabled;
    }
```

(c) Add the four accelerator handlers (place them next to `Window_Copy` / `Window_Paste` at the end of the class):

```csharp
    private void Window_Escape(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs e)
    {
        Context.Instance().EscapeAction();
        e.Handled = true;
    }

    private void Window_Home(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs e)
    {
        Context.Instance().LoadFirstImage();
        e.Handled = true;
    }

    private void Window_End(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs e)
    {
        Context.Instance().LoadLastImage();
        e.Handled = true;
    }

    private void Window_FullScreen(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs e)
    {
        ToggleFullScreen();
        e.Handled = true;
    }
```

Note: `AppWindowPresenterKind` and `GridLength` are in already-imported namespaces (`Microsoft.UI.Windowing`, `Microsoft.UI.Xaml`); no new `using` is needed.

- [ ] **Step 3: Add the XAML accelerators and name the footer**

In `MainWindow.xaml`, extend the existing `Grid.KeyboardAccelerators` block (lines 13-16) to:

```xml
            <Grid.KeyboardAccelerators>
                <KeyboardAccelerator Key="V" Modifiers="Control" Invoked="Window_Paste" />
                <KeyboardAccelerator Key="C" Modifiers="Control" Invoked="Window_Copy" />
                <KeyboardAccelerator Key="Escape" Invoked="Window_Escape" />
                <KeyboardAccelerator Key="Home" Invoked="Window_Home" />
                <KeyboardAccelerator Key="End" Invoked="Window_End" />
                <KeyboardAccelerator Key="F11" Invoked="Window_FullScreen" />
            </Grid.KeyboardAccelerators>
```

And give the row-3 footer Grid a name. Change the line:

```xml
            <Grid Grid.Row="3" BorderThickness="0,1,0,0" Background="{ThemeResource AppBarBackgroundBrush}" BorderBrush="{ThemeResource AppBarBorderBrush}">
```

to:

```xml
            <Grid Grid.Row="3" x:Name="FooterToolbar" x:FieldModifier="public" BorderThickness="0,1,0,0" Background="{ThemeResource AppBarBackgroundBrush}" BorderBrush="{ThemeResource AppBarBorderBrush}">
```

- [ ] **Step 4: Build**

Run: `dotnet build ImageViewer\ImageViewer.csproj -c Debug -p:Platform=x64`
Expected: 0 warnings, 0 errors.

- [ ] **Step 5: Existing tests still pass**

Run: `dotnet test ImageViewer.Tests\ImageViewer.Tests.csproj -c Debug -p:Platform=x64`
Expected: 25 passed, 0 failed.

- [ ] **Step 6: Commit**

```bash
git add ImageViewer/Helpers/Context.cs ImageViewer/MainWindow.xaml.cs ImageViewer/MainWindow.xaml
git commit -m "Add Home/End/Esc shortcuts and F11 fullscreen toggle"
```

---

## Final verification

- [ ] Debug build green: `dotnet build ImageViewer\ImageViewer.csproj -c Debug -p:Platform=x64`
- [ ] Release build green: `dotnet build ImageViewer\ImageViewer.csproj -c Release -p:Platform=x64`
- [ ] Tests green: `dotnet test ImageViewer.Tests\ImageViewer.Tests.csproj -c Debug -p:Platform=x64`
- [ ] Manual (cannot be automated - report as "needs user verification"): Home/End jump, Esc closes cropper then info pane then exits fullscreen, F11 toggles fullscreen hiding title bar + footer, and saved window geometry is intact after a fullscreen round-trip.
