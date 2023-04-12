using System;
using System.IO;
using System.Linq;
using System.Collections.Generic;
using System.Drawing;
using System.Drawing.Imaging;
using System.Diagnostics;
using System.Threading.Tasks;

using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media.Imaging;

using Microsoft.Windows.AppNotifications;
using Microsoft.Windows.AppNotifications.Builder;

using Windows.Storage;
using Windows.Storage.Pickers;
using Windows.Storage.Streams;

using WinUIEx;
using WinRT.Interop;
using Svg;

namespace ImageViewer
{
    enum ImageInfos
    {
        FileName,
        FileDate,
        ImageDimensions,
        ImageSize,
        ImageDpi,
        ImageDepth,
        FolderPath
    }

    internal class Context
    {
        private static Context _Instance;

        private readonly string[] FileTypes = { ".jpg", ".jpeg", ".bmp", ".png", ".gif", ".tif", ".ico", ".webp", ".svg" };
        private readonly string[] ReadOnlyTypes = { ".gif", ".ico", ".svg" };

        private string[] FolderFiles;
        private int CurrentIndex;
        private bool MemoryOnly = false;

        public string[] LaunchArgs;
        public MainWindow MainWindow;
        public WindowManager Manager;
        public NotificationsManger NotificationsManger;

        public Bitmap CurrentImage { get; protected set; }
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
            return FileTypes.Any(x => path.EndsWith(x, true, null));
        }

        /// <summary>
        /// Load image if program is opened with open with command.
        /// </summary>
        public async void LoadDefaultImage()
        {
            if(LaunchArgs.Length > 0)
            {
                if(CheckFileExtension(LaunchArgs[0]))
                {
                    MainWindow.ImageLoadingIndicator.IsActive = true;
                    await Task.Delay(200);

                    if(LoadImageFromString(LaunchArgs[0]))
                    {
                        LoadDirectoryFiles();
                    }
                }
            }
        }

        /// <summary>
        /// List all images in the current directory.
        /// </summary>
        public void LoadDirectoryFiles()
        {
            if(!string.IsNullOrEmpty(CurrentFilePath))
            {
                FolderFiles = Directory.EnumerateFiles(Path.GetDirectoryName(CurrentFilePath), "*.*", SearchOption.TopDirectoryOnly)
                    .Where(s => FileTypes.Any(x => s.EndsWith(x, true, null)))
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
            if(FolderFiles != null && FolderFiles.Length > 0)
            {
                CurrentIndex += 1;

                if(CurrentIndex >= FolderFiles.Length)
                {
                    CurrentIndex = 0;
                }

                if(!LoadImageFromString(FolderFiles[CurrentIndex]))
                {
                    FolderFiles = FolderFiles.RemoveAtIndex(CurrentIndex);
                    LoadNextImage();
                }
            }
        }

        /// <summary>
        /// Load previous image.
        /// </summary>
        public void LoadPrevImage()
        {
            if(FolderFiles != null && FolderFiles.Length > 0)
            {
                CurrentIndex -= 1;

                if(CurrentIndex < 0)
                {
                    CurrentIndex = FolderFiles.Length - 1;
                }

                if(!LoadImageFromString(FolderFiles[CurrentIndex]))
                {
                    FolderFiles = FolderFiles.RemoveAtIndex(CurrentIndex);
                    LoadPrevImage();
                }
            }
        }

        /// <summary>
        /// Load an image from the load picker.
        /// </summary>
        public async void LoadImageFromPicker()
        {
            FileOpenPicker OpenFilePicker = new();

            foreach(string file_type in FileTypes)
            {
                OpenFilePicker.FileTypeFilter.Add(file_type);
            }

            InitializeWithWindow.Initialize(OpenFilePicker, WindowNative.GetWindowHandle(MainWindow));
            StorageFile selectedFile = await OpenFilePicker.PickSingleFileAsync();

            if(selectedFile != null && CheckFileExtension(selectedFile.Path))
            {
                CurrentFilePath = selectedFile.Path;

                MemoryOnly = false;

                LoadBitmap();
                LoadImageView();
                LoadDirectoryFiles();
            }
        }

        public async void LoadImageFromBuffer(RandomAccessStreamReference clipboard)
        {
            CurrentFilePath = null;
            MemoryOnly = true;

            MainWindow.SplitViewContainer.IsPaneOpen = false;
            LoadBitmap(await clipboard.OpenReadAsync());
            LoadImageView(false);

            MainWindow.UpdateTitle(Culture.GetString("SYSTEM_PASTED_CONTENT"));
        }

        /// <summary>
        /// Load an image from path.
        /// </summary>
        public bool LoadImageFromString(string image_path, bool reload_directories = false)
        {
            if(File.Exists(image_path) && CheckFileExtension(image_path))
            {
                CurrentFilePath = image_path;
                MemoryOnly = false;

                LoadBitmap();
                LoadImageView();

                if(reload_directories)
                {
                    LoadDirectoryFiles();
                }

                return true;
            }

            return false;
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
            MainWindow.TextBlockInfoDpi.Text = data[ImageInfos.ImageDpi];
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
                { ImageInfos.ImageDpi, "" },
                { ImageInfos.ImageDepth, "" },
                { ImageInfos.FolderPath, "" }
            };

            FileInfo fileinfo = new(CurrentFilePath);

            dict[ImageInfos.FileName] = Path.GetFileName(CurrentFilePath);
            dict[ImageInfos.FileDate] = File.GetLastWriteTime(CurrentFilePath).ToString();
            dict[ImageInfos.ImageDimensions] = string.Concat(CurrentImage.Width, " x ", CurrentImage.Height);
            dict[ImageInfos.ImageSize] = HumanizeBytes(fileinfo.Length);
            dict[ImageInfos.FolderPath] = Path.GetDirectoryName(CurrentFilePath);
            dict[ImageInfos.ImageDpi] = string.Concat(Math.Round(CurrentImage.HorizontalResolution, 2).ToString(), " dpi");
            dict[ImageInfos.ImageDepth] = string.Concat(Image.GetPixelFormatSize(CurrentImage.PixelFormat).ToString(), " bit");

            return dict;
        }

        /// <summary>
        /// Delete current image.
        /// </summary>
        public void DeleteImage()
        {
            try
            {
                if(CurrentFilePath != null)
                {
                    Microsoft.VisualBasic.FileIO.FileSystem.DeleteFile(
                        CurrentFilePath, 
                        Microsoft.VisualBasic.FileIO.UIOption.OnlyErrorDialogs, 
                        Microsoft.VisualBasic.FileIO.RecycleOption.SendToRecycleBin
                    );

                    CurrentFilePath = null;
                    FolderFiles = FolderFiles.RemoveAtIndex(CurrentIndex);
                }

                if(FolderFiles != null && FolderFiles.Length > 0)
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
            catch(Exception)
            {
            }
        }

        /// <summary>
        /// Check if an image is open.
        /// </summary>
        public bool HasImageLoaded()
        {
            return CurrentImage != null;
        }

        /// <summary>
        /// Fit the image inside the image view.
        /// </summary>
        public void AdjustImage()
        {
            if(CurrentImage == null) return;

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

            if(CurrentImage == null) return zoomFactor;

            if(CurrentImage.Height > MainWindow.ImageContainer.ActualHeight || CurrentImage.Width > MainWindow.ImageContainer.ActualWidth)
            {
                double zoomFactorH = MainWindow.ImageContainer.ActualHeight / CurrentImage.Height;
                double zoomFactorW = MainWindow.ImageContainer.ActualWidth / CurrentImage.Width;

                zoomFactor = (float)((zoomFactorH < zoomFactorW) ? zoomFactorH : zoomFactorW);
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
        public void RotateFlip(RotateFlipType angle)
        {
            CurrentImage.RotateFlip(angle);
            LoadImageView(false);
        }

        /// <summary>
        /// Update interface buttons.
        /// </summary>
        public void UpdateButtonsAccessiblity()
        {
            if(MainWindow == null) return;

            if(CurrentImage != null)
            {
                MainWindow.ButtonImageZoomIn.IsEnabled = true;
                MainWindow.ButtonImageZoomOut.IsEnabled = true;

                MainWindow.ButtonImageAdjust.IsEnabled = true;
                MainWindow.ButtonImageZoomFull.IsEnabled = true;

                MainWindow.ButtonImageTransform.IsEnabled = MemoryOnly || (CurrentFilePath != null && !ReadOnlyTypes.Contains(Path.GetExtension(CurrentFilePath)));

                MainWindow.ButtonImageDelete.IsEnabled = true;
                MainWindow.ButtonFileSave.IsEnabled = true;
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
            }

            if(FolderFiles != null && FolderFiles.Length > 1)
            {
                MainWindow.ButtonImagePrevious.IsEnabled = true;
                MainWindow.ButtonImageNext.IsEnabled = true;
            }
            else
            {
                MainWindow.ButtonImagePrevious.IsEnabled = false;
                MainWindow.ButtonImageNext.IsEnabled = false;
            }

            MainWindow.ButtonFileInfo.IsEnabled = CurrentFilePath != null;

            MainWindow.ButtonImageTransformFlipHorizontal.IsEnabled = MainWindow.ButtonImageTransform.IsEnabled;
            MainWindow.ButtonImageTransformFlipVertical.IsEnabled = MainWindow.ButtonImageTransform.IsEnabled;
            MainWindow.ButtonImageTransformRotateLeft.IsEnabled = MainWindow.ButtonImageTransform.IsEnabled;
            MainWindow.ButtonImageTransformRotateRight.IsEnabled = MainWindow.ButtonImageTransform.IsEnabled;
        }

        public static async void CheckUpdate()
        {
            if(string.IsNullOrEmpty(Settings.UpdateInterval))
            {
                return;
            }

            if(!string.IsNullOrEmpty(Settings.LastUpdateCheck))
            {
                DateTime now = DateTime.Now;
                DateTime last_check = DateTime.Parse(Settings.LastUpdateCheck);

                switch(Settings.UpdateInterval)
                {
                    case "day":
                        last_check = last_check.AddDays(1);
                        break;
                    case "week":
                        last_check = last_check.AddDays(7);
                        break;
                    default:
                        last_check = last_check.AddMonths(1);
                        break;
                }

                if(last_check.Date > now.Date)
                {
                    return;
                }
            }
            
            if(await Update.CheckNewVersionAsync())
            {
                var builder = new AppNotificationBuilder()
                    .AddText(Culture.GetString("ABOUT_UPDATE_INFO_UPDATE_AVAILABLE"))
                    .AddButton(new AppNotificationButton(Culture.GetString("ABOUT_BTN_DOWNLOAD_UPDATE")).AddArgument("action", "doUpdate"));

                NotificationsManger.Runtime.Show(builder.BuildNotification());
            }
        }

        /// <summary>
        /// Save current file as.
        /// </summary>
        public async void SaveAs()
        {
            FileSavePicker SaveFilePicker = new()
            {
                SuggestedFileName = Path.GetFileNameWithoutExtension(CurrentFilePath)
            };
            SaveFilePicker.FileTypeChoices.Add(Culture.GetString("FILE_TYPE_IMAGE_JPG"), new List<string>() { ".jpg" });
            SaveFilePicker.FileTypeChoices.Add(Culture.GetString("FILE_TYPE_IMAGE_PNG"), new List<string>() { ".png" });
            SaveFilePicker.FileTypeChoices.Add(Culture.GetString("FILE_TYPE_IMAGE_WEBP"), new List<string>() { ".webp" });

            InitializeWithWindow.Initialize(SaveFilePicker, WindowNative.GetWindowHandle(MainWindow));
            StorageFile outputFile = await SaveFilePicker.PickSaveFileAsync();

            if(outputFile != null)
            {
                if(CurrentImage != null)
                {
                    switch(outputFile.FileType)
                    {
                        case ".jpg":
                            CurrentImage.SaveJpeg(outputFile.Path, 100);
                            break;

                        case ".png":
                            CurrentImage.Save(outputFile.Path, ImageFormat.Png);
                            break;

                        case ".webp":
                            using(Wrapper.WebP webp = new())
                            {
                                webp.Save(CurrentImage, outputFile.Path, 100);
                                webp.Dispose();
                            }
                            break;
                        default:
                            return;
                    }

                    LoadDirectoryFiles();
                }
            }
        }

        /// <summary>
        /// Load current bitmap
        /// </summary>
        private void LoadBitmap(IRandomAccessStreamWithContentType stream = null)
        {
            MainWindow.ImageView.Opacity = 0;
            MainWindow.ImageLoadingIndicator.IsActive = true;

            if(stream != null)
            {
                CurrentImage = (Bitmap)Image.FromStream(stream.AsStreamForRead());
            }
            else
            {
                switch(Path.GetExtension(CurrentFilePath).ToLower())
                {
                    case ".webp":
                        using(Wrapper.WebP webp = new())
                        {
                            CurrentImage = webp.Load(CurrentFilePath);
                            webp.Dispose();
                        };
                        break;

                    case ".svg":
                        SvgDocument svgDocument = SvgDocument.Open(CurrentFilePath);
                        svgDocument.ShapeRendering = SvgShapeRendering.Auto;
                        CurrentImage = svgDocument.AdjustSize(1024, 1024).Draw();
                        break;

                    default:
                        byte[] bytes = File.ReadAllBytes(CurrentFilePath);
                        MemoryStream ms = new(bytes);
                        CurrentImage = (Bitmap)Image.FromStream(ms);
                        ms.Dispose();
                        break;
                }
            }
        }

        /// <summary>
        /// Load bitmap (CurrentImage) inside image view
        /// </summary>
        private void LoadImageView(bool useUriSource = true)
        {
            GC.Collect();
            GC.WaitForPendingFinalizers();

            BitmapImage bitmapImage = new()
            {
                CreateOptions = BitmapCreateOptions.IgnoreImageCache,
            };
            bitmapImage.ImageOpened += CurrentImage_ImageOpened;
            bitmapImage.ImageFailed += CurrentImage_ImageFailed;

            MainWindow.ImageView.Source = bitmapImage;

            if(!useUriSource || Path.GetExtension(CurrentFilePath).ToLower() == ".svg")
            {
                using MemoryStream memory = new();
                CurrentImage.Save(memory, ImageFormat.Png);
                memory.Position = 0;
                bitmapImage.SetSource(memory.AsRandomAccessStream());
            }
            else
            {
                bitmapImage.UriSource = new(CurrentFilePath);
            }
        }

        /// <summary>
        /// Event: When image is opened.
        /// </summary>
        private void CurrentImage_ImageOpened(object sender, RoutedEventArgs e)
        {
            if(CurrentFilePath != null)
            {
                MainWindow.UpdateTitle(Path.GetFileName(CurrentFilePath));
            }

            UpdateButtonsAccessiblity();
            AdjustImage();

            MainWindow.ImageLoadingIndicator.IsActive = false;
            MainWindow.ImageView.Opacity = 1;

            if(MainWindow.SplitViewContainer.IsPaneOpen)
            {
                UpdateFileInfo();
            }
        }

        /// <summary>
        /// Event: When image loaded failed.
        /// </summary>
        private void CurrentImage_ImageFailed(object sender, ExceptionRoutedEventArgs e)
        {
            MainWindow.UpdateTitle();
            MainWindow.ImageLoadingIndicator.IsActive = false;

            CurrentImage = null;

            UpdateButtonsAccessiblity();
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

            while(bytes >= 1024 && order < sizes.Length - 1)
            {
                order++;
                bytes /= 1024;
            }

            return string.Format("{0:0.#} {1}", bytes, sizes[order]);
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
}
