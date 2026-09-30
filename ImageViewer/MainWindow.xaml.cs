using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Globalization;
using System.Linq;
using System.Runtime.InteropServices;
using System.Threading.Tasks;
using Microsoft.UI;
using Microsoft.UI.Input;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media.Imaging;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Documents;
using Microsoft.UI.Windowing;

using Windows.UI.Core;
using Windows.Foundation;
using Windows.ApplicationModel.DataTransfer;
using Windows.Storage;
using Windows.Storage.Streams;
using WinRT.Interop;
using SixLabors.ImageSharp.Processing;

using ImageViewer.Helpers;
using ImageViewer.Utilities;
using ImageViewer.Views;

namespace ImageViewer;

public sealed partial class MainWindow : Window
{
    private const int SW_RESTORE = 9;

    [DllImport("user32.dll")]
    private static extern bool IsIconic(IntPtr hWnd);

    [DllImport("user32.dll")]
    private static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);

    private bool SavingProcess = false;
    private ContentDialog OpenDialog;
    private Point LastMousePoint;
    private bool ScrollViewMouseDrag;
    private GridLength? OriginalTitleBarRowHeight;
    private GridLength? OriginalFooterRowHeight;
    private Visibility? OriginalTitleBarVisibility;

    // Sentinel ratios: the two entries that cannot be a fixed number.
    private const double ASPECT_FREE = 0;
    private const double ASPECT_SAME = -1;

    // Fullscreen collapses the footer row by index: keep it here so inserting a row above it is a one-line change.
    private const int FOOTER_ROW = 3;

    public readonly Dictionary<string, double>CropperAspectRatios = new()
    {
        {Culture.GetString("TRANSFORM_CROP_FREE"), ASPECT_FREE},
        {Culture.GetString("TRANSFORM_CROP_SAME"), ASPECT_SAME},
        {"1:1", 1},
        {"16:9", 16d / 9d},
        {"16:10", 16d / 10d},
        {"4:3", 4d / 3d}
    };

    public MainWindow(ElementTheme theme)
    {
        InitializeComponent();
        CustomizeAppBar();

        // Follow OS theme switches at runtime while the preference is ElementTheme.Default
        MainPage.ActualThemeChanged += (_, _) => ApplyResolvedTheme();

        UpdateTheme(theme);
        UpdateBackdrop(Settings.Backdrop);
        UpdateTitle();

        CboCropAspectRatios.ItemsSource = CropperAspectRatios.Select(kv => new { Key = kv.Key, Value = kv.Value }).ToList();
    }

    private void CustomizeAppBar()
    {
        AppWindow mAppWindow = GetAppWindowForCurrentWindow();
        mAppWindow.SetIcon("ImageViewer.ico");

        if(AppWindowTitleBar.IsCustomizationSupported())
        {
            AppWindowTitleBar windowTitleBar = mAppWindow.TitleBar;
            windowTitleBar.ExtendsContentIntoTitleBar = true;
            windowTitleBar.PreferredHeightOption = TitleBarHeightOption.Tall;

            RedrawTitleBar();
        }
        else
        {
            AppTitleBar.Visibility = Visibility.Collapsed;
            MainLayout.RowDefinitions[0].Height = GridLength.Auto;
        }
    }

    public void RedrawTitleBar()
    {
        if(!AppWindowTitleBar.IsCustomizationSupported()) return;
        
        ResourceDictionary resourceTheme = Theme.GetThemeResourceDictionary(MainPage.ActualTheme == ElementTheme.Dark ? "Dark" : "Light");
        AppWindowTitleBar windowTitleBar = GetAppWindowForCurrentWindow().TitleBar;
        
        windowTitleBar.ButtonBackgroundColor = Colors.Transparent;
        windowTitleBar.ButtonForegroundColor = windowTitleBar.ButtonInactiveForegroundColor = (resourceTheme["TitleBarButtonForeground"] as SolidColorBrush).Color;

        windowTitleBar.ButtonHoverBackgroundColor = (resourceTheme["TitleBarButtonHoverBackground"] as SolidColorBrush).Color;
        windowTitleBar.ButtonHoverForegroundColor = (resourceTheme["TitleBarButtonHoverForeground"] as SolidColorBrush).Color;

        windowTitleBar.ButtonPressedBackgroundColor = (resourceTheme["TitleBarButtonPressedBackground"] as SolidColorBrush).Color;
        windowTitleBar.ButtonPressedForegroundColor = (resourceTheme["TitleBarButtonPressedForeground"] as SolidColorBrush).Color;

        windowTitleBar.ButtonInactiveBackgroundColor = Colors.Transparent;
    }

    public void UpdateTheme(ElementTheme theme)
    {
        MainPage.RequestedTheme = theme;

        // Persist the user intent: Default must survive so the app keeps following the system theme
        Settings.Theme = theme;

        ApplyResolvedTheme();
    }

    private void ApplyResolvedTheme()
    {
        bool isDark = MainPage.ActualTheme == ElementTheme.Dark;

        Theme.SetImmersiveDarkMode(WindowNative.GetWindowHandle(this), isDark);

        ButtonSwitchThemeDark.IsEnabled = !isDark;
        ButtonSwitchThemeDark.Visibility = isDark ? Visibility.Collapsed : Visibility.Visible;

        ButtonSwitchThemeLight.IsEnabled = isDark;
        ButtonSwitchThemeLight.Visibility = isDark ? Visibility.Visible : Visibility.Collapsed;

        RedrawTitleBar();
        PaintSurfaces();
    }

    internal void UpdateBackdrop(Backdrop backdrop)
    {
        SystemBackdrop = backdrop switch
        {
            Backdrop.Mica => new MicaBackdrop(),
            Backdrop.Acrylic => new DesktopAcrylicBackdrop(),
            _ => null
        };

        Settings.Backdrop = backdrop;

        PaintSurfaces();
    }

    /// <summary>
    /// Opaque theme colors in Basic mode, transparent over a backdrop so the material shows through.
    /// Painted from code rather than {ThemeResource} because the brush depends on the backdrop too;
    /// ApplyResolvedTheme calls this on every theme switch. Transparent, never null: the image area
    /// must stay hit-testable for drag and drop and the pointer handlers.
    /// </summary>
    private void PaintSurfaces()
    {
        ResourceDictionary colors = Theme.GetThemeResourceDictionary(MainPage.ActualTheme == ElementTheme.Dark ? "Dark" : "Light");
        Brush Pick(string key) => SystemBackdrop != null ? new SolidColorBrush(Colors.Transparent) : (Brush)colors[key];

        AppTitleBar.Background = FooterToolbar.Background = Pick("AppBarBackgroundBrush");
        ImageContainer.Background = ImageCropper.Background = Pick("ImageViewContainerBackground");
    }

    public void UpdateTitle(string prefix = null)
    {
        Title = string.IsNullOrEmpty(prefix) ? AppInfo.ProductName : string.Concat(prefix, " - ", AppInfo.ProductName);
        AppTitleBarText.Text = Title;
    }

    private AppWindow GetAppWindowForCurrentWindow()
    {
        IntPtr hWnd = WindowNative.GetWindowHandle(this);
        WindowId wndId = Win32Interop.GetWindowIdFromWindow(hWnd);
        return AppWindow.GetFromWindowId(wndId);
    }

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
        OriginalFooterRowHeight ??= MainLayout.RowDefinitions[FOOTER_ROW].Height;
        OriginalTitleBarVisibility ??= AppTitleBar.Visibility;

        if(enabled)
        {
            // Raise the guard BEFORE switching presenter: SetPresenter triggers a synchronous
            // size/position change, and the fullscreen bounds must never be persisted as the
            // windowed geometry
            App.IsFullScreen = true;

            appWindow.SetPresenter(AppWindowPresenterKind.FullScreen);

            AppTitleBar.Visibility = Visibility.Collapsed;
            FooterToolbar.Visibility = Visibility.Collapsed;
            IconSizeBar.Visibility = Visibility.Collapsed;
            MainLayout.RowDefinitions[0].Height = new GridLength(0);
            MainLayout.RowDefinitions[FOOTER_ROW].Height = new GridLength(0);
        }
        else
        {
            appWindow.SetPresenter(AppWindowPresenterKind.Default);

            AppTitleBar.Visibility = OriginalTitleBarVisibility.Value;
            FooterToolbar.Visibility = Visibility.Visible;
            MainLayout.RowDefinitions[0].Height = OriginalTitleBarRowHeight.Value;
            MainLayout.RowDefinitions[FOOTER_ROW].Height = OriginalFooterRowHeight.Value;

            RedrawTitleBar();

            // Lower the guard only after the presenter is restored, so the transition's
            // size/position change back to the windowed bounds is not recorded mid-flight
            App.IsFullScreen = false;

            // The size strip is only shown for icons: let the context decide whether it comes back
            Context.Instance().UpdateButtonsAccessiblity();
        }
    }

    private void ButtonOpenFile_Click(object sender, RoutedEventArgs e)
    {
        Context.Instance().LoadImageFromPicker();
    }

    private void ButtonNextFile_Click(object sender, RoutedEventArgs e)
    {
        Context.Instance().LoadNextImage();
    }

    private void ButtonPrevFile_Click(object sender, RoutedEventArgs e)
    {
        Context.Instance().LoadPrevImage();
    }

    private void ButtonFullsize_Click(object sender, RoutedEventArgs e)
    {
        ScrollView.ChangeView(0, 0, 1);
    }

    private void ButtonAdjust_Click(object sender, RoutedEventArgs e)
    {
        Context.Instance().AdjustImage();
    }

    private void ButtonZoomIn_Click(object sender, RoutedEventArgs e)
    {
        Context.Instance().Zoom(0.1);
    }

    private void ButtonZoomOut_Click(object sender, RoutedEventArgs e)
    {
        Context.Instance().Zoom(-0.1);
    }

    private void ButtonDelete_Click(object sender, RoutedEventArgs e)
    {
        Context.Instance().DeleteImage();
    }

    private void ButtonQuit_Click(object sender, RoutedEventArgs e)
    {
        Close();
    }

    private async void Window_Closed(object sender, WindowEventArgs args)
    {
        await Context.Instance().PrepareExitAsync();

        Environment.Exit(0);
    }

    private void CboCropAspectRatios_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        double ratio = (double)CboCropAspectRatios.SelectedValue;

        ImageCropper.AspectRatio = ratio switch
        {
            ASPECT_FREE => null,
            ASPECT_SAME => Context.Instance().CurrentImage.Width / Context.Instance().CurrentImage.Height,
            _ => ratio
        };
    }

    private void ButtonFileInfo_Click(object sender, RoutedEventArgs e)
    {
        SplitViewContainer.IsPaneOpen = true;
        ScrollView.Focus(FocusState.Programmatic);
    }

    private void ButtonFileInfoClose_Click(object sender, RoutedEventArgs e)
    {
        SplitViewContainer.IsPaneOpen = false;
        ScrollView.Focus(FocusState.Programmatic);
    }

    private void ButtonImageCrop_Click(object sender, RoutedEventArgs e)
    {
        if(ImageCropper.Source != null) return;

        ImageCropper.Source = Context.Instance().CurrentImage.GetWriteableBitmap();

        CboCropAspectRatios.SelectedValue = ASPECT_FREE;

        ImageContainer.Visibility = Visibility.Collapsed;
        ImageCropperContainer.Visibility = Visibility.Visible;
        ImageCropperContainer.IsPaneOpen = true;

        ImageCropper.IsEnabled = true;
        Context.Instance().UpdateButtonsAccessiblity();
    }

    private void ImageCropperSizeChanged(object sender, SizeChangedEventArgs e)
    {
        Context.Instance().UpdateCropperLayout();
    }

    private void ImageCropperEnabledChanged(object sender, DependencyPropertyChangedEventArgs e)
    {
        Context.Instance().UpdateCropperLayout();
    }

    private void ButtonCropValidate_Click(object sender, RoutedEventArgs e)
    {
        Rect rect = ImageCropper.CroppedRegion;
        Context.Instance().Crop((int)rect.X, (int)rect.Y, (int)rect.Width, (int)rect.Height);
    }

    private void ButtonCropCancel_Click(object sender, RoutedEventArgs e)
    {
        Context.Instance().CloseCropper();
    }

    private void ButtonCropReset_Click(object sender, RoutedEventArgs e)
    {
        ImageCropper.Reset();
    }

    private void TextBlockInfoFolder_Click(Hyperlink hyperlinkControl, RoutedEventArgs args)
    {
        Run firstItem = (Run)hyperlinkControl.Inlines[0];
        Process.Start("explorer.exe", firstItem.Text);
    }

    private void ScrollView_PointerMoved(object sender, PointerRoutedEventArgs e)
    {
        if(Context.Instance().HasImageLoaded() && ScrollViewMouseDrag)
        {
            ImageContainer.SetCursor(new CoreCursor(CoreCursorType.Hand, 0));
            PointerPoint point = e.GetCurrentPoint(ScrollView);

            double deltaX = point.Position.X - LastMousePoint.X;
            double deltaY = point.Position.Y - LastMousePoint.Y;

            ScrollView.ScrollToHorizontalOffset(ScrollView.HorizontalOffset - deltaX);
            ScrollView.ScrollToVerticalOffset(ScrollView.VerticalOffset - deltaY);

            LastMousePoint = point.Position;
        }
        else
        {
            ImageContainer.SetCursor(new CoreCursor(CoreCursorType.Arrow, 0));
        }
    }

    private void ScrollView_PointerExited(object sender, PointerRoutedEventArgs e)
    {
        ImageContainer.SetCursor(new CoreCursor(CoreCursorType.Arrow, 0));
        ScrollViewMouseDrag = false;
    }

    private void ScrollView_PointerPressed(object sender, PointerRoutedEventArgs e)
    {
        PointerPoint point = e.GetCurrentPoint(ScrollView);

        if(point.Properties.IsLeftButtonPressed)
        {
            ScrollViewMouseDrag = true;
            LastMousePoint = point.Position;
        }
        else if(point.Properties.IsXButton1Pressed)
        {
            Context.Instance().LoadPrevImage();
        }
        else if(point.Properties.IsXButton2Pressed)
        {
            Context.Instance().LoadNextImage();
        }
    }

    private void ScrollView_PointerReleased(object sender, PointerRoutedEventArgs e)
    {
        ScrollViewMouseDrag = false;
    }

    private void ScrollView_PointerWheelChanged(object sender, PointerRoutedEventArgs e)
    {
        if(!Context.Instance().HasImageLoaded()) return;

        PointerPoint point = e.GetCurrentPoint(ScrollView);
        int delta = point.Properties.MouseWheelDelta;

        if(delta == 0) return;

        float newZoom = Math.Clamp((float)(ScrollView.ZoomFactor * Math.Pow(1.1, delta / 120.0)), ScrollView.MinZoomFactor, ScrollView.MaxZoomFactor);

        // Keep the pixel under the cursor stable while zooming
        double scale = newZoom / ScrollView.ZoomFactor;
        double offsetX = (ScrollView.HorizontalOffset + point.Position.X) * scale - point.Position.X;
        double offsetY = (ScrollView.VerticalOffset + point.Position.Y) * scale - point.Position.Y;

        ScrollView.ChangeView(offsetX, offsetY, newZoom);
        e.Handled = true;
    }

    private void ScrollView_ViewChanged(object sender, ScrollViewerViewChangedEventArgs e)
    {
        TextBlockZoomFactor.Text = string.Concat(Math.Round(ScrollView.ZoomFactor * 100).ToString(CultureInfo.InvariantCulture), "%");

        if(e.IsIntermediate) return;

        if(ScrollView.ZoomFactor == Context.Instance().GetAdjustedZoomFactor())
        {
            ButtonImageAdjust.Visibility = Visibility.Collapsed;
            ButtonImageAdjust.IsEnabled = false;
            ButtonImageZoomFull.Visibility = Visibility.Visible;
            ButtonImageZoomFull.IsEnabled = true;
        }
        else
        {
            ButtonImageAdjust.Visibility = Visibility.Visible;
            ButtonImageAdjust.IsEnabled = true;
            ButtonImageZoomFull.Visibility = Visibility.Collapsed;
            ButtonImageZoomFull.IsEnabled = false;
        }
    }

    private void SplitViewContainer_PaneOpening(SplitView sender, object args)
    {
        Context.Instance().UpdateFileInfo();
        ScrollView.Focus(FocusState.Programmatic);
    }

    private void Grid_SizeChanged(object sender, SizeChangedEventArgs e)
    {
        Context.Instance().UpdateButtonsAccessiblity();
    }

    private void IconSizeStrip_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        Context.Instance().SelectIconSize(IconSizeStrip.SelectedIndex);
    }

    private void ButtonIconSizeLarger_Click(object sender, RoutedEventArgs e)
    {
        // Sizes are listed largest first, so stepping left grows the icon
        Context.Instance().StepIconSize(-1);
    }

    private void ButtonIconSizeSmaller_Click(object sender, RoutedEventArgs e)
    {
        Context.Instance().StepIconSize(1);
    }

    private void ButtonImageRotateLeft_Click(object sender, RoutedEventArgs e)
    {
        Context.Instance().RotateFlip(RotateMode.Rotate270, FlipMode.None);
    }

    private void ButtonImageRotateRight_Click(object sender, RoutedEventArgs e)
    {
        Context.Instance().RotateFlip(RotateMode.Rotate90, FlipMode.None);
    }

    private void ButtonImageFlipHorizontal_Click(object sender, RoutedEventArgs e)
    {
        Context.Instance().RotateFlip(RotateMode.None, FlipMode.Vertical);
    }

    private void ButtonImageFlipVertical_Click(object sender, RoutedEventArgs e)
    {
        Context.Instance().RotateFlip(RotateMode.None, FlipMode.Horizontal);
    }

    private async void ButtonFileSaveDirect_Click(object sender, RoutedEventArgs e)
    {
        await RunSave(Context.Instance().Save);
    }

    private async void ButtonFileSave_Click(object sender, RoutedEventArgs e)
    {
        await RunSave(Context.Instance().SaveAs);
    }

    /// <summary>
    /// Run a save through the re-entrancy guard: the accelerator can fire again before the
    /// modal picker takes focus, which would open a second one.
    /// </summary>
    private async Task RunSave(Func<Task<bool>> save)
    {
        if(SavingProcess) return;

        try
        {
            SavingProcess = true;
            await save();
        }
        finally
        {
            SavingProcess = false;
        }
    }

    private async void ButtonPrint_Click(object sender, RoutedEventArgs e)
    {
        await Context.Instance().Print();
    }

    public Panel GetPrintHost()
    {
        return PrintHost;
    }

    /// <summary>
    /// Modal error dialog with a single OK button, themed like the window.
    /// </summary>
    public async Task ShowErrorAsync(string messageKey)
    {
        await ShowDialogAsync(new ContentDialog
        {
            Content = Culture.GetString(messageKey),
            CloseButtonText = Culture.GetString("SYSTEM_OK")
        });
    }

    /// <summary>
    /// Show a dialog themed like the window. WinUI throws when a second ContentDialog opens while
    /// one is showing, so every dialog goes through here and is dropped if another is already up.
    /// </summary>
    private async Task ShowDialogAsync(ContentDialog dialog)
    {
        if(OpenDialog != null) return;

        OpenDialog = dialog;
        dialog.XamlRoot = Content.XamlRoot;
        dialog.RequestedTheme = MainPage.ActualTheme;

        try
        {
            await dialog.ShowAsync();
        }
        finally
        {
            OpenDialog = null;
        }
    }

    private void ButtonAbout_Click(object sender, RoutedEventArgs e)
    {
        Context.Instance().ShowAbout();
    }

    /// <summary>
    /// Show the about dialog, bringing the window forward first: the update toast can be clicked
    /// while the window is minimized or behind another app, and a dialog nobody can see reads as
    /// "the button did nothing".
    /// </summary>
    public async Task ShowAbout(bool startUpdate = false)
    {
        // Win32 rather than the presenter: a fullscreen window minimized by Win+D has no OverlappedPresenter
        IntPtr hwnd = WindowNative.GetWindowHandle(this);
        if(IsIconic(hwnd))
        {
            ShowWindow(hwnd, SW_RESTORE);
        }

        Activate();

        // Toast clicked with the about dialog already up: run the update there instead of stacking a second one
        if(OpenDialog?.Content is DialogAbout openAbout)
        {
            if(startUpdate) openAbout.StartUpdate();
            return;
        }

        ContentDialog dialogAbout = new();
        dialogAbout.Content = new DialogAbout(dialogAbout, startUpdate);
        await ShowDialogAsync(dialogAbout);
    }

    private async void ButtonSettings_Click(object sender, RoutedEventArgs e)
    {
        ContentDialog dialogSettings = new();
        dialogSettings.Content = new DialogSettings(dialogSettings);
        await ShowDialogAsync(dialogSettings);
    }

    private void ButtonSwitchThemeDark_Click(object sender, RoutedEventArgs e)
    {
        Context.Instance().ChangeTheme(ElementTheme.Dark);
    }

    private void ButtonSwitchThemeLight_Click(object sender, RoutedEventArgs e)
    {
        Context.Instance().ChangeTheme(ElementTheme.Light);
    }

    private async void ImageContainer_Drop(object sender, DragEventArgs e)
    {
        try
        {
            if(!e.DataView.Contains(StandardDataFormats.StorageItems)) return;

            IReadOnlyList<IStorageItem> items = await e.DataView.GetStorageItemsAsync();

            if(items.Count > 0)
            {
                Context.Instance().LoadImageFromString(items[0].Path, true);
            }
        }
        catch(Exception ex)
        {
            Debug.WriteLine(ex.Message);
        }
    }

    private void ImageContainer_DragOver(object sender, DragEventArgs e)
    {
        e.AcceptedOperation = DataPackageOperation.Move;
        e.DragUIOverride.IsCaptionVisible = false;
        e.DragUIOverride.IsGlyphVisible = false;
    }

    private void Window_Copy(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs e)
    {
        Context.Instance().CopyImageToClipboard();
        e.Handled = true;
    }

    private async void Window_Paste(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs e)
    {
        DataPackageView clipboard = Clipboard.GetContent();

        if(!clipboard.Contains(StandardDataFormats.Bitmap)) return;

        ImageView.Opacity = 0;
        ImageLoadingIndicator.IsActive = true;

        RandomAccessStreamReference clipboardImage = await clipboard.GetBitmapAsync();
        Context.Instance().LoadImageFromBuffer(clipboardImage);
    }

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

    private void Window_Prev(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs e)
    {
        Context.Instance().LoadPrevImage();
        e.Handled = true;
    }

    private void Window_Next(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs e)
    {
        Context.Instance().LoadNextImage();
        e.Handled = true;
    }
}