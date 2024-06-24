using System;
using System.IO;
using System.Linq;
using System.Collections.Generic;
using System.Diagnostics;
using System.Threading.Tasks;
using System.Globalization;

using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media.Imaging;
using Microsoft.Windows.AppNotifications.Builder;

using Windows.Storage;
using Windows.Storage.Pickers;
using Windows.Storage.Streams;

using WinUIEx;
using WinRT.Interop;
using SixLabors.ImageSharp.Processing;

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
        return Image.SupportedFileTypes.Any(x => path.EndsWith(x, true, null));
    }

    /// <summary>
    /// Load image if program is opened with open with command.
    /// </summary>
    public async void LoadDefaultImage()
    {
        if (LaunchArgs.Length <= 0) return;
        if (!CheckFileExtension(LaunchArgs[0])) return;

        LoadingDisplay(true);
        await Task.Delay(200);

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
            FolderFiles = Directory.EnumerateFiles(Path.GetDirectoryName(CurrentFilePath), "*.*", SearchOption.TopDirectoryOnly)
                .Where(s => Image.SupportedFileTypes.Any(x => s.EndsWith(x, true, null)))
                .OrderBy(s => s, new NaturalStringComparer())
                .ToArray();

            CurrentIndex = Array.IndexOf(FolderFiles, CurrentFilePath);
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
                    FolderFiles = FolderFiles.RemoveAtIndex(CurrentIndex);
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

        foreach (string fileType in Image.SupportedFileTypes)
        {
            openFilePicker.FileTypeFilter.Add(fileType);
        }

        InitializeWithWindow.Initialize(openFilePicker, WindowNative.GetWindowHandle(MainWindow));
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

        MainWindow.UpdateTitle(Culture.GetString("SYSTEM_PASTED_CONTENT"));
    }

    /// <summary>
    /// Load an image from path.
    /// </summary>
    public bool LoadImageFromString(string imagePath, bool reloadDirectories = false)
    {
        if (!File.Exists(imagePath) || !CheckFileExtension(imagePath)) return false;

        CurrentFilePath = imagePath;
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
    /// Zoom inside image view.
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

        MainWindow.ButtonImageTransformFlipHorizontal.IsEnabled = MainWindow.ButtonImageTransform.IsEnabled;
        MainWindow.ButtonImageTransformFlipVertical.IsEnabled = MainWindow.ButtonImageTransform.IsEnabled;
        MainWindow.ButtonImageTransformRotateLeft.IsEnabled = MainWindow.ButtonImageTransform.IsEnabled;
        MainWindow.ButtonImageTransformRotateRight.IsEnabled = MainWindow.ButtonImageTransform.IsEnabled;
    }

    /// <summary>
    /// Check update using UpdateInterval setting
    /// </summary>
    public async void CheckUpdate()
    {
        if (string.IsNullOrEmpty(Settings.UpdateInterval))
        {
            return;
        }

        if (!string.IsNullOrEmpty(Settings.LastUpdateCheck))
        {
            DateTime now = DateTime.Now;
            DateTime lastCheck = DateTime.Parse(Settings.LastUpdateCheck);

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

            if (lastCheck.Date > now.Date)
            {
                return;
            }
        }

        if (!await Update.CheckNewVersionAsync()) return;

        var builder = new AppNotificationBuilder()
            .AddText(Culture.GetString("ABOUT_UPDATE_INFO_UPDATE_AVAILABLE"))
            .AddButton(new AppNotificationButton(Culture.GetString("ABOUT_BTN_DOWNLOAD_UPDATE")).AddArgument("action", "doUpdate"));

        NotificationsManger.Runtime.Show(builder.BuildNotification());
    }

    /// <summary>
    /// Save current file as.
    /// </summary>
    public async void SaveAs()
    {
        if (!HasImageLoaded()) return;

        FileSavePicker saveFilePicker = new()
        {
            SuggestedFileName = CurrentFilePath != null ? Path.GetFileNameWithoutExtension(CurrentFilePath) : DateTime.Now.ToString("yyyy-MM-dd-hh-mm-ss")
        };

        foreach (string fileType in Image.SaveFileTypes)
        {
            saveFilePicker.FileTypeChoices.Add(Culture.GetString("FOOTER_TOOLBAR_MENU_FILE_SAVE_FORMAT").Replace("{0}", fileType.Remove(0, 1).ToUpper()), new List<string>() { fileType });
        }

        InitializeWithWindow.Initialize(saveFilePicker, WindowNative.GetWindowHandle(MainWindow));
        StorageFile outputFile = await saveFilePicker.PickSaveFileAsync();

        if (outputFile == null) return;
        if (!Image.SaveFileTypes.Contains(outputFile.FileType)) return;

        CurrentImage.Save(outputFile.Path, outputFile.FileType);

        LoadDirectoryFiles();
    }

    /// <summary>
    /// Load bitmap (CurrentImage) inside image view
    /// </summary>
    public void ReloadImageView()
    {
        if (!HasImageLoaded()) return;

        BitmapImage bitmapImage = new()
        {
            CreateOptions = BitmapCreateOptions.IgnoreImageCache,
        };

        bitmapImage.ImageOpened += ImageView_ImageOpened;
        bitmapImage.ImageFailed += ImageView_ImageFailed;

        MainWindow.ImageView.Source = bitmapImage;

        bitmapImage.SetSource(CurrentImage.GetBitmapImageSource());
    }

    /// <summary>
    /// Event: When image view is loaded and ready.
    /// </summary>
    private void ImageView_ImageOpened(object sender, RoutedEventArgs e)
    {
        if (!HasImageLoaded()) return;

        if (CurrentFilePath != null)
        {
            MainWindow.UpdateTitle(Path.GetFileName(CurrentFilePath));
        }

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
        MainWindow.UpdateTitle();
        MainWindow.ImageLoadingIndicator.IsActive = false;

        CurrentImage = null;

        UpdateButtonsAccessiblity();
    }

    /// <summary>
    /// Load current image
    /// </summary>
    private void OpenImage(IInputStream stream = null)
    {
        CurrentImage?.Dispose();

        LoadingDisplay(true);

        CurrentImage = new();
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