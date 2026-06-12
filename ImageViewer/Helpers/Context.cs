using System;
using System.IO;
using System.Linq;
using System.Collections.Generic;
using System.Diagnostics;
using System.Threading.Tasks;
using System.Globalization;
using System.Reflection;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media.Imaging;
using Microsoft.Windows.AppNotifications.Builder;

using Windows.Storage;
using Windows.Storage.Pickers;
using Windows.Storage.Streams;
using WinRT.Interop;
using SixLabors.ImageSharp.Processing;

using Velopack;
using Velopack.Sources;

using ImageViewer.Utilities;
using ImageViewer.Wrapper;

namespace ImageViewer.Helpers;

internal enum ImageInfos
{
    FileName,
    FileDate,
    ImageDimensions,
    ImageSize,
    ImageDepth,
    FolderPath
}

internal class Context
{
    private static Context _Instance;
    private string[] FolderFiles;
    private int CurrentIndex;
    private bool MemoryOnly;

    public string[] LaunchArgs;
    public MainWindow MainWindow;
    public NotificationsManger NotificationsManger;

    // Built lazily so the Velopack assemblies are not loaded on the startup path:
    // the first touch happens from the deferred CheckUpdate(), after the window is shown.
    private UpdateManager _UpdateManager;
    private UpdateManager UpdateManager => _UpdateManager ??= new UpdateManager(
        new GithubSource(
            repoUrl: "https://github.com/dragonofmercy/image-viewer",
            accessToken: null,
            prerelease: false));

    public UpdateInfo PendingUpdate { get; private set; }

    /// <summary>
    /// Query the update source and cache the result in <see cref="PendingUpdate"/>.
    /// Returns null when no update is available or when running outside a Velopack install.
    /// Network/parse errors bubble to the caller so the UI can react.
    /// </summary>
    public async Task<UpdateInfo> CheckForUpdateAsync()
    {
        if (!UpdateManager.IsInstalled)
        {
            PendingUpdate = null;
            return null;
        }

        UpdateInfo info = await UpdateManager.CheckForUpdatesAsync();
        Settings.TouchLastUpdateCheck();
        PendingUpdate = info;
        return info;
    }

    /// <summary>
    /// Download <see cref="PendingUpdate"/> and restart into the new version.
    /// No-op when there is no pending update.
    /// </summary>
    public async Task ApplyPendingUpdateAsync()
    {
        if (PendingUpdate == null) return;

        await UpdateManager.DownloadUpdatesAsync(PendingUpdate);
        UpdateManager.ApplyUpdatesAndRestart(PendingUpdate);
    }

    public Image CurrentImage { get; protected set; }
    public string CurrentFilePath { get; protected set; }

    public void ChangeTheme(ElementTheme theme)
    {
        MainWindow.UpdateTheme(theme);
    }

    /// <summary>
    /// Check if file can be open.
    /// </summary>
    public bool CheckFileExtension(string path)
    {
        return Image.SupportedFileTypes.Any(x => path.EndsWith(x, StringComparison.OrdinalIgnoreCase));
    }

    /// <summary>
    /// Load image if program is opened with open with command.
    /// </summary>
    public async void LoadDefaultImage()
    {
        if (LaunchArgs.Length <= 0) return;
        if (!CheckFileExtension(LaunchArgs[0])) return;

        LoadingDisplay(true);
        // Brief yield so the window paints and the loading indicator shows before the
        // first decode competes for the UI thread. Kept short to minimize "Open with" latency.
        await Task.Delay(50);

        if (LoadImageFromString(LaunchArgs[0]))
        {
            LoadDirectoryFiles();
        }
    }

    /// <summary>
    /// List all images in the current directory.
    /// </summary>
    public void LoadDirectoryFiles()
    {
        if (!string.IsNullOrEmpty(CurrentFilePath))
        {
            try
            {
                FolderFiles = Directory.EnumerateFiles(Path.GetDirectoryName(CurrentFilePath), "*.*", SearchOption.TopDirectoryOnly)
                    .Where(s => Image.SupportedFileTypes.Any(x => s.EndsWith(x, StringComparison.OrdinalIgnoreCase)))
                    .OrderBy(s => s, new NaturalStringComparer())
                    .ToArray();

                CurrentIndex = Array.FindIndex(FolderFiles, s => string.Equals(s, CurrentFilePath, StringComparison.OrdinalIgnoreCase));
            }
            catch (Exception)
            {
                // Folder vanished or is unreadable (network share dropped): keep the image, disable navigation
                FolderFiles = null;
                CurrentIndex = -1;
            }
        }

        UpdateButtonsAccessiblity();
    }

    /// <summary>
    /// Load next image.
    /// </summary>
    public void LoadNextImage()
    {
        while (true)
        {
            if (FolderFiles is { Length: > 0 })
            {
                CurrentIndex += 1;

                if (CurrentIndex >= FolderFiles.Length)
                {
                    CurrentIndex = 0;
                }

                if (!LoadImageFromString(FolderFiles[CurrentIndex]))
                {
                    // The removal shifts the next file into CurrentIndex: step back so the increment lands on it
                    FolderFiles = FolderFiles.RemoveAtIndex(CurrentIndex);
                    CurrentIndex -= 1;
                    continue;
                }
            }

            break;
        }
    }

    /// <summary>
    /// Load previous image.
    /// </summary>
    public void LoadPrevImage()
    {
        while (true)
        {
            if (FolderFiles is { Length: > 0 })
            {
                CurrentIndex -= 1;

                if (CurrentIndex < 0)
                {
                    CurrentIndex = FolderFiles.Length - 1;
                }

                if (!LoadImageFromString(FolderFiles[CurrentIndex]))
                {
                    FolderFiles = FolderFiles.RemoveAtIndex(CurrentIndex);
                    continue;
                }
            }

            break;
        }
    }

    /// <summary>
    /// Load an image from the load picker.
    /// </summary>
    public async void LoadImageFromPicker()
    {
        FileOpenPicker openFilePicker = new();

        InitializeWithWindow.Initialize(openFilePicker, WindowNative.GetWindowHandle(MainWindow));

        openFilePicker.ViewMode = PickerViewMode.Thumbnail;

        foreach (string fileType in Image.SupportedFileTypes)
        {
            openFilePicker.FileTypeFilter.Add(fileType);
        }

        StorageFile selectedFile = await openFilePicker.PickSingleFileAsync();

        if (selectedFile == null || !CheckFileExtension(selectedFile.Path)) return;

        CurrentFilePath = selectedFile.Path;

        MemoryOnly = false;

        OpenImage();
        LoadDirectoryFiles();
    }

    /// <summary>
    /// Load image from buffer (on paste)
    /// </summary>
    public async void LoadImageFromBuffer(RandomAccessStreamReference clipboard)
    {
        CurrentFilePath = null;
        MemoryOnly = true;

        MainWindow.SplitViewContainer.IsPaneOpen = false;
        OpenImage(await clipboard.OpenReadAsync());
    }

    /// <summary>
    /// Load an image from path.
    /// </summary>
    public bool LoadImageFromString(string imagePath, bool reloadDirectories = false)
    {
        if (!File.Exists(imagePath) || !CheckFileExtension(imagePath)) return false;

        // Normalize relative launch arguments so directory enumeration and index lookups match
        CurrentFilePath = Path.GetFullPath(imagePath);
        MemoryOnly = false;

        OpenImage();

        if (reloadDirectories)
        {
            LoadDirectoryFiles();
        }

        return true;
    }

    /// <summary>
    /// Update file infos.
    /// </summary>
    public void UpdateFileInfo()
    {
        Dictionary<ImageInfos, string> data = GetFileInfos();

        MainWindow.TextBlockInfoFilename.Text = data[ImageInfos.FileName];
        MainWindow.TextBlockInfoDate.Text = data[ImageInfos.FileDate];
        MainWindow.TextBlockInfoDimensions.Text = data[ImageInfos.ImageDimensions];
        MainWindow.TextBlockInfoSize.Text = data[ImageInfos.ImageSize];
        MainWindow.TextBlockInfoDepth.Text = data[ImageInfos.ImageDepth];
        MainWindow.TextBlockInfoFolder.Text = data[ImageInfos.FolderPath];
    }

    /// <summary>
    /// Load image infos from file.
    /// </summary>
    public Dictionary<ImageInfos, string> GetFileInfos()
    {
        Dictionary<ImageInfos, string> dict = new()
        {
            { ImageInfos.FileName, "" },
            { ImageInfos.FileDate, "" },
            { ImageInfos.ImageDimensions, "" },
            { ImageInfos.ImageSize, "" },
            { ImageInfos.ImageDepth, "" },
            { ImageInfos.FolderPath, "" }
        };

        if (!HasImageLoaded()) return dict;

        FileInfo oFileInfo = new(CurrentFilePath);

        dict[ImageInfos.FileName] = Path.GetFileName(CurrentFilePath);
        dict[ImageInfos.FolderPath] = Path.GetDirectoryName(CurrentFilePath);
        dict[ImageInfos.FileDate] = File.GetLastWriteTime(CurrentFilePath).ToString(CultureInfo.CurrentCulture);
        dict[ImageInfos.ImageSize] = HumanizeBytes(oFileInfo.Length);
        dict[ImageInfos.ImageDimensions] = CurrentImage.GetImageDimensionsAsString();
        dict[ImageInfos.ImageDepth] = CurrentImage.GetDepthAsString();

        return dict;
    }

    /// <summary>
    /// Delete current image.
    /// </summary>
    public void DeleteImage()
    {
        try
        {
            if (CurrentFilePath != null)
            {
                Microsoft.VisualBasic.FileIO.FileSystem.DeleteFile(
                    CurrentFilePath,
                    Microsoft.VisualBasic.FileIO.UIOption.OnlyErrorDialogs,
                    Microsoft.VisualBasic.FileIO.RecycleOption.SendToRecycleBin
                );

                CurrentFilePath = null;
                FolderFiles = FolderFiles.RemoveAtIndex(CurrentIndex);

                // The next file shifted into CurrentIndex: step back so LoadNextImage lands on it
                CurrentIndex -= 1;
            }

            if (FolderFiles is { Length: > 0 })
            {
                LoadNextImage();
            }
            else
            {
                CurrentImage.Dispose();
                CurrentImage = null;

                MainWindow.UpdateTitle();
                MainWindow.ImageView.Opacity = 0;
                MainWindow.SplitViewContainer.IsPaneOpen = false;
            }

            UpdateButtonsAccessiblity();
        }
        catch (Exception)
        {
            // ignored
        }
    }

    /// <summary>
    /// Display loading or not
    /// </summary>
    public void LoadingDisplay(bool status)
    {
        if (status)
        {
            MainWindow.GlobalErrorMessage.Visibility = Visibility.Collapsed;
            MainWindow.GlobalErrorMessageFileName.Text = "";
        }

        MainWindow.ImageLoadingIndicator.IsActive = status;
        MainWindow.ImageView.Opacity = status ? 0 : 1;

        UpdateButtonsAccessiblity();
    }

    /// <summary>
    /// Check if an image is open.
    /// </summary>
    public bool HasImageLoaded()
    {
        return CurrentImage is { Loaded: true };
    }

    /// <summary>
    /// Fit the image inside the image view.
    /// </summary>
    public void AdjustImage()
    {
        if (!HasImageLoaded()) return;

        float zoomFactor = GetAdjustedZoomFactor();

        MainWindow.ScrollView.ChangeView(0, 0, zoomFactor, true);
        MainWindow.ScrollView.ZoomToFactor(zoomFactor);
    }

    /// <summary>
    /// Get the zoom factor to fit image inside image view.
    /// </summary>
    public float GetAdjustedZoomFactor()
    {
        float zoomFactor = 1;

        if (!HasImageLoaded()) return zoomFactor;

        if (CurrentImage.Height > MainWindow.ImageContainer.ActualHeight || CurrentImage.Width > MainWindow.ImageContainer.ActualWidth)
        {
            double zoomFactorH = MainWindow.ImageContainer.ActualHeight / CurrentImage.Height;
            double zoomFactorW = MainWindow.ImageContainer.ActualWidth / CurrentImage.Width;

            zoomFactor = (float)(zoomFactorH < zoomFactorW ? zoomFactorH : zoomFactorW);
        }

        return zoomFactor;
    }

    /// <summary>
    /// Zoom inside the image view.
    /// </summary>
    public void Zoom(double factor)
    {
        MainWindow.ScrollView.ZoomToFactor(RoundToTen((MainWindow.ScrollView.ZoomFactor + factor) * 100) / 100);
    }

    /// <summary>
    /// Rotate or flip image.
    /// </summary>
    public void RotateFlip(RotateMode rotateMode, FlipMode flipMode)
    {
        if (!HasImageLoaded()) return;
        CurrentImage.RotateFlip(rotateMode, flipMode);
        ReloadImageView();
    }

    /// <summary>
    /// Crop image
    /// </summary>
    public void Crop(int x, int y, int width, int height)
    {
        if (!HasImageLoaded()) return;
        CurrentImage.Crop(x, y, width, height);
        CloseCropper();
        ReloadImageView();
    }

    /// <summary>
    /// Update interface buttons.
    /// </summary>
    public void UpdateButtonsAccessiblity()
    {
        if (MainWindow == null) return;

        if (HasImageLoaded())
        {
            MainWindow.ButtonImageZoomIn.IsEnabled = true;
            MainWindow.ButtonImageZoomOut.IsEnabled = true;

            MainWindow.ButtonImageAdjust.IsEnabled = true;
            MainWindow.ButtonImageZoomFull.IsEnabled = true;

            MainWindow.ButtonImageTransform.IsEnabled = MemoryOnly || CurrentFilePath != null;

            MainWindow.ButtonImageDelete.IsEnabled = true;
            MainWindow.ButtonFileSave.IsEnabled = true;

            MainWindow.ButtonFileInfo.IsEnabled = CurrentFilePath != null;

            MainWindow.TextBlockDimensions.Text = CurrentImage.Width + "x" + CurrentImage.Height;

            MainWindow.UpdateTitle(Path.GetFileName(CurrentFilePath ?? Culture.GetString("SYSTEM_PASTED_CONTENT")));
        }
        else
        {
            MainWindow.ButtonImageZoomIn.IsEnabled = false;
            MainWindow.ButtonImageZoomOut.IsEnabled = false;

            MainWindow.ButtonImageAdjust.IsEnabled = false;
            MainWindow.ButtonImageZoomFull.IsEnabled = false;
            MainWindow.ButtonImageTransform.IsEnabled = false;

            MainWindow.ButtonImageDelete.IsEnabled = false;
            MainWindow.ButtonFileSave.IsEnabled = false;

            MainWindow.ButtonFileInfo.IsEnabled = false;

            MainWindow.TextBlockDimensions.Text = "";

            MainWindow.UpdateTitle();
        }

        if(MainWindow.ImageLoadingIndicator.IsActive)
        {
            MainWindow.UpdateTitle(Culture.GetString("SYSTEM_LOADING"));
        }

        if (FolderFiles is { Length: > 1 })
        {
            MainWindow.ButtonImagePrevious.IsEnabled = true;
            MainWindow.ButtonImageNext.IsEnabled = true;
        }
        else
        {
            MainWindow.ButtonImagePrevious.IsEnabled = false;
            MainWindow.ButtonImageNext.IsEnabled = false;
        }

        if(MainWindow.ImageCropperContainer.Visibility == Visibility.Visible)
        {
            MainWindow.ButtonImageTransform.IsEnabled = false;
            MainWindow.ButtonImagePrevious.IsEnabled = false;
            MainWindow.ButtonImageNext.IsEnabled = false;

            MainWindow.ButtonImageZoomIn.IsEnabled = false;
            MainWindow.ButtonImageZoomOut.IsEnabled = false;

            MainWindow.ButtonImageAdjust.IsEnabled = false;
            MainWindow.ButtonImageZoomFull.IsEnabled = false;
            MainWindow.ButtonImageTransform.IsEnabled = false;

            MainWindow.ButtonImageDelete.IsEnabled = false;
            MainWindow.ButtonFileSave.IsEnabled = false;
            MainWindow.ButtonFileInfo.IsEnabled = false;
        }

        MainWindow.ButtonImageTransformFlipHorizontal.IsEnabled = MainWindow.ButtonImageTransform.IsEnabled;
        MainWindow.ButtonImageTransformFlipVertical.IsEnabled = MainWindow.ButtonImageTransform.IsEnabled;
        MainWindow.ButtonImageTransformRotateLeft.IsEnabled = MainWindow.ButtonImageTransform.IsEnabled;
        MainWindow.ButtonImageTransformRotateRight.IsEnabled = MainWindow.ButtonImageTransform.IsEnabled;
        MainWindow.ButtonImageTransformCrop.IsEnabled = MainWindow.ButtonImageTransform.IsEnabled;
    }

    /// <summary>
    /// Update Cropper layout.
    /// </summary>
    public void UpdateCropperLayout()
    {
        if(MainWindow.ImageCropper.Source == null) return;
        MainWindow.ImageCropper.GetType().GetTypeInfo().GetDeclaredMethod("UpdateMaskArea").Invoke(MainWindow.ImageCropper, [false]);
    }

    /// <summary>
    /// Close Cropper.
    /// </summary>
    public void CloseCropper()
    {
        MainWindow.ImageCropper.IsEnabled = false;
        MainWindow.ImageContainer.Visibility = Visibility.Visible;
        MainWindow.ImageCropperContainer.Visibility = Visibility.Collapsed;

        MainWindow.ImageCropper.Source = null;

        UpdateButtonsAccessiblity();
        MainWindow.ScrollView.Focus(FocusState.Programmatic);
    }

    /// <summary>
    /// Background update check honoring the UpdateInterval setting. Errors are swallowed.
    /// </summary>
    public async void CheckUpdate()
    {
        if (!UpdateManager.IsInstalled) return;

        if (string.IsNullOrEmpty(Settings.UpdateInterval))
        {
            return;
        }

        // A corrupted registry value must not prevent startup: an unparseable date simply triggers a new check
        if (!string.IsNullOrEmpty(Settings.LastUpdateCheck) && DateTime.TryParseExact(Settings.LastUpdateCheck, Settings.UPDATE_DATE_FORMAT, CultureInfo.InvariantCulture, DateTimeStyles.None, out DateTime lastCheck))
        {
            switch (Settings.UpdateInterval)
            {
                case "day":
                    lastCheck = lastCheck.AddDays(1);
                    break;
                case "week":
                    lastCheck = lastCheck.AddDays(7);
                    break;
                default:
                    lastCheck = lastCheck.AddMonths(1);
                    break;
            }

            if (lastCheck.Date > DateTime.Now.Date)
            {
                return;
            }
        }

        try
        {
            if (await CheckForUpdateAsync() == null) return;
        }
        catch (Exception ex)
        {
            Debug.WriteLine($"Update check failed: {ex.Message}");
            return;
        }

        AppNotificationBuilder builder = new AppNotificationBuilder()
            .AddText(Culture.GetString("ABOUT_UPDATE_INFO_UPDATE_AVAILABLE"))
            .AddButton(new AppNotificationButton(Culture.GetString("ABOUT_BTN_DOWNLOAD_UPDATE")).AddArgument("action", "doUpdate"));

        NotificationsManger.Runtime.Show(builder.BuildNotification());
    }

    /// <summary>
    /// Save current file as.
    /// </summary>
    public async Task<bool> SaveAs()
    {
        if (!HasImageLoaded()) return false;

        FileSavePicker saveFilePicker = new()
        {
            SuggestedFileName = CurrentFilePath != null ? Path.GetFileNameWithoutExtension(CurrentFilePath) : DateTime.Now.ToString("yyyy-MM-dd-hh-mm-ss")
        };

        foreach (string fileType in Image.SaveFileTypes)
        {
            saveFilePicker.FileTypeChoices.Add(Culture.GetString("FOOTER_TOOLBAR_MENU_FILE_SAVE_FORMAT").Replace("{0}", fileType.Remove(0, 1).ToUpper()), new List<string>{ fileType });
        }

        InitializeWithWindow.Initialize(saveFilePicker, WindowNative.GetWindowHandle(MainWindow));
        StorageFile outputFile = await saveFilePicker.PickSaveFileAsync();

        if (outputFile == null) return false;

        string outputFileType = outputFile.FileType.ToLowerInvariant();

        if (!Image.SaveFileTypes.Contains(outputFileType)) return false;

        try
        {
            await CurrentImage.Save(outputFile.Path, outputFileType);
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

        LoadDirectoryFiles();

        return true;
    }

    /// <summary>
    /// Load bitmap (CurrentImage) inside image view
    /// </summary>
    public void ReloadImageView()
    {
        if (!HasImageLoaded()) return;

        // Animated images must go through BitmapImage (WriteableBitmap only renders a single frame)
        if (CurrentImage.IsAnimated)
        {
            BitmapImage bitmapImage = new()
            {
                CreateOptions = BitmapCreateOptions.IgnoreImageCache
            };

            bitmapImage.ImageOpened += ImageView_ImageOpened;
            bitmapImage.ImageFailed += ImageView_ImageFailed;
            bitmapImage.SetSource(CurrentImage.GetBitmapImageSource());

            MainWindow.ImageView.Source = bitmapImage;
            return;
        }

        MainWindow.ImageView.Source = CurrentImage.GetWriteableBitmap();
        ImageView_ImageOpened(MainWindow.ImageView, null);
    }

    /// <summary>
    /// Event: When image view is loaded and ready.
    /// </summary>
    private void ImageView_ImageOpened(object sender, RoutedEventArgs e)
    {
        if (!HasImageLoaded()) return;

        UpdateButtonsAccessiblity();
        AdjustImage();

        LoadingDisplay(false);

        if (MainWindow.SplitViewContainer.IsPaneOpen)
        {
            UpdateFileInfo();
        }
    }

    /// <summary>
    /// Event: When image view loaded failed.
    /// </summary>
    private void ImageView_ImageFailed(object sender, ExceptionRoutedEventArgs e)
    {
        MainWindow.ImageLoadingIndicator.IsActive = false;

        CurrentImage = null;

        UpdateButtonsAccessiblity();
    }

    /// <summary>
    /// Load current image
    /// </summary>
    public void OpenImage(IInputStream stream = null)
    {
        CloseCropper();

        if (CurrentImage != null)
        {
            CurrentImage.ImageLoaded -= WorkingImage_ImageLoaded;
            CurrentImage.ImageFailed -= WorkingImage_ImageFailed;
            CurrentImage.Dispose();
        }

        LoadingDisplay(true);

        CurrentImage = new Image();
        CurrentImage.ImageLoaded += WorkingImage_ImageLoaded;
        CurrentImage.ImageFailed += WorkingImage_ImageFailed;

        if (stream != null)
        {
            CurrentImage.Load(stream);
        }
        else
        {
            CurrentImage.Load(CurrentFilePath);
        }
    }

    /// <summary>
    /// Event: When image load failed.
    /// </summary>
    private void WorkingImage_ImageFailed(object sender, EventArgs e)
    {
        // A stale load can complete after the user navigated to another image: never touch the new one
        if (!ReferenceEquals(sender, CurrentImage)) return;

        MainWindow.UpdateTitle();

        MainWindow.ImageLoadingIndicator.IsActive = false;
        MainWindow.GlobalErrorMessage.Visibility = Visibility.Visible;

        MainWindow.GlobalErrorMessageFileName.Text = ((ImageFailedEventArgs)e).Path;

        CurrentImage.Dispose();
        UpdateButtonsAccessiblity();
    }

    /// <summary>
    /// Event: When image is loaded.
    /// </summary>
    private void WorkingImage_ImageLoaded(object sender, EventArgs e)
    {
        if (!ReferenceEquals(sender, CurrentImage)) return;

        ReloadImageView();
    }

    /// <summary>
    /// Round value to unit 10.
    /// </summary>
    public static float RoundToTen(double input)
    {
        return (int)(Math.Round(input) / 10.0) * 10;
    }

    /// <summary>
    /// Get product name
    /// </summary>
    public static string GetProductName()
    {
        return FileVersionInfo.GetVersionInfo(System.Reflection.Assembly.GetExecutingAssembly().Location).ProductName;
    }

    /// <summary>
    /// Get product version
    /// </summary>
    public static string GetProductVersion()
    {
        return FileVersionInfo.GetVersionInfo(System.Reflection.Assembly.GetExecutingAssembly().Location).ProductVersion;
    }

    /// <summary>
    /// Get a human readable byte value.
    /// </summary>
    public static string HumanizeBytes(double bytes)
    {
        int order = 0;
        string[] sizes = { "B", "KB", "MB", "GB", "TB" };

        while (bytes >= 1024 && order < sizes.Length - 1)
        {
            order++;
            bytes /= 1024;
        }

        return $"{bytes:0.#} {sizes[order]}";
    }

    /// <summary>
    /// Get current Context instance.
    /// </summary>
    public static Context Instance()
    {
        _Instance ??= new Context();

        return _Instance;
    }
}