using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Diagnostics;

using Microsoft.UI;
using Microsoft.UI.Input;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Windowing;

using Windows.UI.Core;
using Windows.Foundation;
using Windows.ApplicationModel.DataTransfer;
using Windows.Storage;
using Windows.Storage.Streams;

using WinRT.Interop;

namespace ImageViewer
{
    public sealed partial class MainWindow : Window
    {
        public const int KEYEVENTF_KEYDOWN = 0x0000;        // New definition
        public const int KEYEVENTF_EXTENDEDKEY = 0x0001;    // Key down flag
        public const int KEYEVENTF_KEYUP = 0x0002;          // Key up flag
        public const int VK_LCONTROL = 0xA2;                // Left Control key code

        private readonly AppWindow m_AppWindow;
        private Point LastMousePoint;
        private bool ZoomKeyDownState = false;
        private bool ScrollViewMouseDrag = false;

        [DllImport("user32.dll")]
        private static extern void keybd_event(byte bVk, byte bScan, uint dwFlags, int dwExtraInfo);

        public MainWindow(ElementTheme theme)
        {
            InitializeComponent();
            UpdateTheme(theme);

            m_AppWindow = GetAppWindowForCurrentWindow();
            m_AppWindow.SetIcon("ImageViewer.ico");
            m_AppWindow.Title = Context.GetProductName();
        }

        public void UpdateTheme(ElementTheme theme)
        {
            ThemeHelpers.SetImmersiveDarkMode(WindowNative.GetWindowHandle(this), theme == ElementTheme.Dark);
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
        }

        public void UpdateTitle(string prefix = null)
        {
            if(string.IsNullOrEmpty(prefix))
            {
                m_AppWindow.Title = Context.GetProductName();
            }
            else
            {
                m_AppWindow.Title = string.Concat(prefix, " - ", Context.GetProductName());
            }
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
            Application.Current.Exit();
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

        private void ScrollView_PointerMoved(object sender, PointerRoutedEventArgs e)
        {
            if(Context.Instance().HasImageLoaded() && ScrollViewMouseDrag)
            {
                ImageContainer.SetCursor(new CoreCursor(CoreCursorType.Hand, 0));
                PointerPoint point = e.GetCurrentPoint(ScrollView);

                double delta_x = point.Position.X - LastMousePoint.X;
                double delta_y = point.Position.Y - LastMousePoint.Y;

                ScrollView.ScrollToHorizontalOffset(ScrollView.HorizontalOffset - delta_x);
                ScrollView.ScrollToVerticalOffset(ScrollView.VerticalOffset - delta_y);

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
            if(!ZoomKeyDownState)
            {
                keybd_event(VK_LCONTROL, 0x45, KEYEVENTF_EXTENDEDKEY | KEYEVENTF_KEYDOWN, 0);
                ZoomKeyDownState = true;
            }
        }

        private void ScrollView_ViewChanged(object sender, ScrollViewerViewChangedEventArgs e)
        {
            TextBlockZoomFactor.Text = string.Concat(Math.Round(ScrollView.ZoomFactor * 100).ToString(), "%");
            
            if(!e.IsIntermediate)
            {
                keybd_event(VK_LCONTROL, 0x45, KEYEVENTF_EXTENDEDKEY | KEYEVENTF_KEYUP, 0);
                ZoomKeyDownState = false;

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
            Context.Instance().RotateFlip(System.Drawing.RotateFlipType.Rotate270FlipNone);
        }

        private void ButtonImageRotateRight_Click(object sender, RoutedEventArgs e)
        {
            Context.Instance().RotateFlip(System.Drawing.RotateFlipType.Rotate90FlipNone);
        }

        private void ButtonImageFlipHorizontal_Click(object sender, RoutedEventArgs e)
        {
            Context.Instance().RotateFlip(System.Drawing.RotateFlipType.RotateNoneFlipY);
        }

        private void ButtonImageFlipVertical_Click(object sender, RoutedEventArgs e)
        {
            Context.Instance().RotateFlip(System.Drawing.RotateFlipType.RotateNoneFlipX);
        }

        private void ButtonFileSave_Click(object sender, RoutedEventArgs e)
        {
            Context.Instance().SaveAs();
        }

        private async void ButtonAbout_Click(object sender, RoutedEventArgs e)
        {
            ContentDialog about_dialog = new()
            {
                XamlRoot = Content.XamlRoot
            };
            about_dialog.Content = new About(about_dialog);
            about_dialog.RequestedTheme = MainPage.ActualTheme;
            await about_dialog.ShowAsync();
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
                if(e.DataView.Contains(StandardDataFormats.StorageItems))
                {
                    IReadOnlyList<IStorageItem> items = await e.DataView.GetStorageItemsAsync();

                    if(items.Count > 0)
                    {
                        Context.Instance().LoadImageFromString(items[0].Path, true);
                    }
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

            if(clipboard.Contains(StandardDataFormats.Bitmap))
            {
                ImageView.Opacity = 0;
                ImageLoadingIndicator.IsActive = true;

                RandomAccessStreamReference clipboard_image = await clipboard.GetBitmapAsync();
                Context.Instance().LoadImageFromBuffer(clipboard_image);
            }
        }
    }
}
