using System;
using System.IO;
using System.Linq;
using System.Diagnostics;
using System.Threading.Tasks;
using System.Globalization;
using System.Reflection;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;

using Windows.Storage;
using Windows.Storage.Pickers;
using Windows.Storage.Streams;
using WinRT.Interop;
using SixLabors.ImageSharp.Processing;

using ImageViewer.Services;
using ImageViewer.Utilities;
using ImageViewer.Wrapper;

namespace ImageViewer.Helpers;

internal class Context
{
    private static Context _Instance;
    private string[] FolderFiles;
    private int CurrentIndex;

    // Window-bound services, all built lazily on first use through the same backing-field property pattern.
    private PrintService _PrintService;
    private PrintService PrintService => _PrintService ??= new PrintService(MainWindow, MainWindow.GetPrintHost());
    private SaveService _SaveService;
    private SaveService SaveService => _SaveService ??= new SaveService(MainWindow);

    public string[] LaunchArgs;
    public MainWindow MainWindow;
    public NotificationsService NotificationsService;

    // The update flow (Velopack manager, interval check, toast) lives in UpdateService.
    // Built lazily so the Velopack assemblies are not loaded on the startup path.
    private UpdateService _UpdateService;
    public UpdateService UpdateService => _UpdateService ??= new UpdateService();

    public Image CurrentImage { get; protected set; }
    public string CurrentFilePath { get; protected set; }

    // Set while the size strip is being filled, so restoring its selection is not mistaken for a user click
    private bool PopulatingIconSizes;

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

        // Let the Ctrl+O accelerator's KeyUp be processed before the modal picker grabs focus.
        await UIThread.YieldAsync();

        StorageFile selectedFile = await openFilePicker.PickSingleFileAsync();

        if (selectedFile == null || !CheckFileExtension(selectedFile.Path)) return;

        CurrentFilePath = selectedFile.Path;

        OpenImage();
        LoadDirectoryFiles();
    }

    /// <summary>
    /// Copy the current image to the clipboard as a CF_DIB bitmap (symmetric with paste).
    /// </summary>
    public void CopyImageToClipboard()
    {
        if (!HasImageLoaded()) return;

        try
        {
            byte[] pixels = CurrentImage.GetBgra32Pixels(out int width, out int height);
            ClipboardHelper.SetImageAsDib(WindowNative.GetWindowHandle(MainWindow), pixels, width, height);
        }
        catch (Exception ex)
        {
            Debug.WriteLine($"Copy to clipboard failed: {ex.Message}");
        }
    }

    /// <summary>
    /// Load image from buffer (on paste)
    /// </summary>
    public async void LoadImageFromBuffer(RandomAccessStreamReference clipboard)
    {
        CurrentFilePath = null;

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

        OpenImage();

        if (reloadDirectories)
        {
            LoadDirectoryFiles();
        }

        return true;
    }

    /// <summary>
    /// Fill the info pane from the current file. Every field is blanked for a pasted image,
    /// which has no file on disk to describe.
    /// </summary>
    public void UpdateFileInfo()
    {
        bool hasFile = HasImageLoaded() && CurrentFilePath != null;

        MainWindow.TextBlockInfoFilename.Text = hasFile ? Path.GetFileName(CurrentFilePath) : "";
        MainWindow.TextBlockInfoFolder.Text = hasFile ? Path.GetDirectoryName(CurrentFilePath) : "";
        MainWindow.TextBlockInfoDate.Text = hasFile ? File.GetLastWriteTime(CurrentFilePath).ToString(CultureInfo.CurrentCulture) : "";
        MainWindow.TextBlockInfoSize.Text = hasFile ? Format.HumanizeBytes(new FileInfo(CurrentFilePath).Length) : "";
        MainWindow.TextBlockInfoDimensions.Text = hasFile ? CurrentImage.GetImageDimensionsAsString() : "";
        MainWindow.TextBlockInfoDepth.Text = hasFile ? CurrentImage.GetDepthAsString() : "";
    }

    /// <summary>
    /// Rebuild the icon size strip from the current image. Called once per opened file: the
    /// thumbnails are decoded pixels, not something to rebuild on every layout pass.
    /// </summary>
    public void PopulateIconSizes()
    {
        PopulatingIconSizes = true;

        try
        {
            MainWindow.IconSizeStrip.Items.Clear();

            if (!HasImageLoaded() || !CurrentImage.HasIconSizes) return;

            for (int i = 0; i < CurrentImage.IconSizeCount; i++)
            {
                (int width, int height) = CurrentImage.GetIconSize(i);

                MainWindow.IconSizeStrip.Items.Add(new IconSizeItem
                {
                    Thumbnail = CurrentImage.GetIconSizeThumbnail(i),
                    Label = width.ToString(CultureInfo.InvariantCulture),
                    Tooltip = width + " x " + height
                });
            }

            MainWindow.IconSizeStrip.SelectedIndex = CurrentImage.IconSizeIndex;
        }
        finally
        {
            PopulatingIconSizes = false;
            UpdateIconSizeButtons();
        }
    }

    /// <summary>
    /// Show another size of the current icon. The frames are already decoded, so no reload happens.
    /// </summary>
    public void SelectIconSize(int index)
    {
        if (PopulatingIconSizes || index < 0) return;
        if (!HasImageLoaded() || !CurrentImage.HasIconSizes) return;
        if (index == CurrentImage.IconSizeIndex) return;

        CurrentImage.SelectIconSize(index);
        UpdateIconSizeButtons();
        ReloadImageView();
    }

    /// <summary>
    /// Move by one entry in the size strip. Clamped, not wrapping: the arrows disable at both ends.
    /// </summary>
    public void StepIconSize(int delta)
    {
        if (!HasImageLoaded() || !CurrentImage.HasIconSizes) return;

        // Driving the selection keeps the strip highlight and the displayed frame in sync
        MainWindow.IconSizeStrip.SelectedIndex = Math.Clamp(CurrentImage.IconSizeIndex + delta, 0, CurrentImage.IconSizeCount - 1);
    }

    private void UpdateIconSizeButtons()
    {
        bool stepable = HasImageLoaded() && CurrentImage.HasIconSizes;

        MainWindow.ButtonIconSizeLarger.IsEnabled = stepable && CurrentImage.IconSizeIndex > 0;
        MainWindow.ButtonIconSizeSmaller.IsEnabled = stepable && CurrentImage.IconSizeIndex < CurrentImage.IconSizeCount - 1;
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
            zoomFactor = (float)Math.Min(MainWindow.ImageContainer.ActualHeight / CurrentImage.Height, MainWindow.ImageContainer.ActualWidth / CurrentImage.Width);
        }

        return zoomFactor;
    }

    /// <summary>
    /// Zoom inside the image view.
    /// </summary>
    public void Zoom(double factor)
    {
        MainWindow.ScrollView.ZoomToFactor(Format.RoundToTen((MainWindow.ScrollView.ZoomFactor + factor) * 100) / 100);
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
    /// Update interface buttons. The open cropper owns the whole surface: while it is up every
    /// image command is off, whatever the image state.
    /// </summary>
    public void UpdateButtonsAccessiblity()
    {
        if (MainWindow == null) return;

        bool loaded = HasImageLoaded();
        bool cropping = MainWindow.ImageCropperContainer.Visibility == Visibility.Visible;
        bool enabled = loaded && !cropping;
        bool canNavigate = !cropping && FolderFiles is { Length: > 1 };

        MainWindow.ButtonImageZoomIn.IsEnabled = enabled;
        MainWindow.ButtonImageZoomOut.IsEnabled = enabled;
        MainWindow.ButtonImageAdjust.IsEnabled = enabled;
        MainWindow.ButtonImageZoomFull.IsEnabled = enabled;
        MainWindow.ButtonImageDelete.IsEnabled = enabled;
        MainWindow.ButtonFileSave.IsEnabled = enabled;
        MainWindow.ButtonPrint.IsEnabled = enabled;
        MainWindow.ButtonFileSaveDirect.IsEnabled = enabled && CurrentImage.Modified;
        MainWindow.ButtonFileInfo.IsEnabled = enabled && CurrentFilePath != null;

        MainWindow.ButtonImagePrevious.IsEnabled = canNavigate;
        MainWindow.ButtonImageNext.IsEnabled = canNavigate;

        MainWindow.ButtonImageTransform.IsEnabled = enabled;
        MainWindow.ButtonImageTransformFlipHorizontal.IsEnabled = enabled;
        MainWindow.ButtonImageTransformFlipVertical.IsEnabled = enabled;
        MainWindow.ButtonImageTransformRotateLeft.IsEnabled = enabled;
        MainWindow.ButtonImageTransformRotateRight.IsEnabled = enabled;
        MainWindow.ButtonImageTransformCrop.IsEnabled = enabled;

        MainWindow.TextBlockDimensions.Text = loaded ? CurrentImage.Width + "x" + CurrentImage.Height : "";

        // Cheap visibility toggle only: the strip contents are built by PopulateIconSizes on load.
        MainWindow.IconSizeBar.Visibility = enabled && !App.IsFullScreen && CurrentImage.HasIconSizes ? Visibility.Visible : Visibility.Collapsed;

        if (MainWindow.ImageLoadingIndicator.IsActive)
        {
            MainWindow.UpdateTitle(Culture.GetString("SYSTEM_LOADING"));
        }
        else if (loaded)
        {
            string title = Path.GetFileName(CurrentFilePath ?? Culture.GetString("SYSTEM_PASTED_CONTENT"));
            // Unsaved indicator: real files turn dirty on edit, pasted/memory images start dirty (see Image load).
            MainWindow.UpdateTitle(CurrentImage.Modified ? "● " + title : title);
        }
        else
        {
            MainWindow.UpdateTitle();
        }
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
    /// Overwrite the current file in place. No-op unless the image was modified.
    /// Falls back to Save As for pasted images and non-writable source formats (svg/ico).
    /// </summary>
    public async Task<bool> Save()
    {
        if (!HasImageLoaded() || !CurrentImage.Modified) return false;

        // Pasted content has no source file: route to Save As.
        if (CurrentFilePath == null) return await SaveAs();

        string type = SaveService.NormalizeExtension(Path.GetExtension(CurrentFilePath));

        // Source format we cannot re-encode (svg/ico): route to Save As.
        if (!Image.SaveFileTypes.Contains(type)) return await SaveAs();

        if (!await SaveService.WriteAsync(CurrentImage, CurrentFilePath, type)) return false;

        UpdateButtonsAccessiblity();
        return true;
    }

    /// <summary>
    /// Save current file as.
    /// </summary>
    public async Task<bool> SaveAs()
    {
        if (!HasImageLoaded()) return false;

        string suggestedName = CurrentFilePath != null ? Path.GetFileNameWithoutExtension(CurrentFilePath) : DateTime.Now.ToString("yyyy-MM-dd-hh-mm-ss");

        (string Path, string Type)? target = await SaveService.PickSaveTargetAsync(suggestedName);

        if (target == null) return false;

        if (!await SaveService.WriteAsync(CurrentImage, target.Value.Path, target.Value.Type)) return false;

        // Adopt the saved file as the current document so the title, folder listing and navigation
        // follow it - uniform for pasted/memory-only content and real files, same or different folder.
        CurrentFilePath = target.Value.Path;

        LoadDirectoryFiles();

        return true;
    }

    /// <summary>
    /// Print the current image fit-to-page through the native Windows print dialog.
    /// </summary>
    public async Task Print()
    {
        if(!HasImageLoaded()) return;

        try
        {
            WriteableBitmap bitmap = CurrentImage.GetWriteableBitmap();
            string jobName = CurrentFilePath != null
                ? Path.GetFileName(CurrentFilePath)
                : Culture.GetString("SYSTEM_PASTED_CONTENT");

            await PrintService.PrintAsync(bitmap, jobName);
        }
        catch(Exception)
        {
            await MainWindow.ShowErrorAsync("SYSTEM_PRINTING_ERROR");
        }
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

        // Fill the strip before the view reloads: it changes the layout height, and AdjustImage
        // (fired at the end of ReloadImageView) must fit the image to the space that remains.
        PopulateIconSizes();
        ReloadImageView();
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

/// <summary>
/// One entry of the icon size strip. Public because the DataTemplate in MainWindow.xaml binds to it.
/// </summary>
public class IconSizeItem
{
    public ImageSource Thumbnail { get; init; }
    public string Label { get; init; }
    public string Tooltip { get; init; }
}