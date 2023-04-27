using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Globalization;

using Microsoft.UI;
using Microsoft.UI.Input;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Documents;
using Microsoft.UI.Windowing;

using Windows.UI.Core;
using Windows.Foundation;
using Windows.ApplicationModel.DataTransfer;
using Windows.ApplicationModel.Resources;
using Windows.Storage;
using Windows.Storage.Streams;

using WinRT.Interop;
using SixLabors.ImageSharp.Processing;

using ImageViewer.Helpers;
using ImageViewer.Controls;
using ImageViewer.Views;

namespace ImageViewer;

public sealed partial class MainWindow : Window
{
    private Point LastMousePoint;
    private bool ScrollViewMouseDrag;

    public MainWindow(ElementTheme theme)
    {
        InitializeComponent();
        CustomizeAppBar();
        UpdateTheme(theme);
        UpdateTitle();
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
    }

    public void UpdateTheme(ElementTheme theme)
    {
        Theme.SetImmersiveDarkMode(WindowNative.GetWindowHandle(this), theme == ElementTheme.Dark);
        MainPage.RequestedTheme = theme;

        Settings.Theme = MainPage.ActualTheme;

        if(theme == ElementTheme.Dark)
        {
            ButtonSwitchThemeDark.IsEnabled = false;
            ButtonSwitchThemeDark.Visibility = Visibility.Collapsed;

            ButtonSwitchThemeLight.IsEnabled = true;
            ButtonSwitchThemeLight.Visibility = Visibility.Visible;
        }
        else
        {
            ButtonSwitchThemeDark.IsEnabled = true;
            ButtonSwitchThemeDark.Visibility = Visibility.Visible;

            ButtonSwitchThemeLight.IsEnabled = false;
            ButtonSwitchThemeLight.Visibility = Visibility.Collapsed;
        }

        RedrawTitleBar();
    }

    public void UpdateTitle(string prefix = null)
    {
        Title = string.IsNullOrEmpty(prefix) ? Context.GetProductName() : string.Concat(prefix, " - ", Context.GetProductName());
        AppTitleBarText.Text = Title;
    }

    private AppWindow GetAppWindowForCurrentWindow()
    {
        IntPtr hWnd = WindowNative.GetWindowHandle(this);
        WindowId wndId = Win32Interop.GetWindowIdFromWindow(hWnd);
        return AppWindow.GetFromWindowId(wndId);
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

    private void Window_Closed(object sender, WindowEventArgs args)
    {
        VirtualKeyboard.ControlRelease();
        Context.Instance().NotificationsManger.Clear();

        Environment.Exit(0);
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

        VirtualKeyboard.ControlRelease();
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
        if(!VirtualKeyboard.ControlPressed())
        {
            VirtualKeyboard.ControlPress();
        }
    }

    private void ScrollView_ViewChanged(object sender, ScrollViewerViewChangedEventArgs e)
    {
        TextBlockZoomFactor.Text = string.Concat(Math.Round(ScrollView.ZoomFactor * 100).ToString(CultureInfo.InvariantCulture), "%");

        if(e.IsIntermediate) return;

        VirtualKeyboard.ControlRelease();

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

    private void ButtonFileSave_Click(object sender, RoutedEventArgs e)
    {
        Context.Instance().SaveAs();
    }

    private async void ButtonAbout_Click(object sender, RoutedEventArgs e)
    {
        ContentDialog dialogAbout = new()
        {
            XamlRoot = Content.XamlRoot
        };
        dialogAbout.Content = new DialogAbout(dialogAbout);
        dialogAbout.RequestedTheme = MainPage.ActualTheme;
        await dialogAbout.ShowAsync();
    }

    private async void ButtonSettings_Click(object sender, RoutedEventArgs e)
    {
        ContentDialog dialogSettings = new()
        {
            XamlRoot = Content.XamlRoot
        };
        dialogSettings.Content = new DialogSettings(dialogSettings);
        dialogSettings.RequestedTheme = MainPage.ActualTheme;
        await dialogSettings.ShowAsync();
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

    private async void Window_Paste(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs e)
    {
        DataPackageView clipboard = Clipboard.GetContent();

        if(!clipboard.Contains(StandardDataFormats.Bitmap)) return;

        ImageView.Opacity = 0;
        ImageLoadingIndicator.IsActive = true;

        RandomAccessStreamReference clipboardImage = await clipboard.GetBitmapAsync();
        Context.Instance().LoadImageFromBuffer(clipboardImage);
    }
}