using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media.Imaging;

using System;
using System.IO;
using System.Linq;
using System.Threading;
using System.Collections.Generic;
using System.Drawing;
using System.Drawing.Imaging;
using System.Diagnostics;

using Windows.Storage;
using Windows.Storage.Pickers;
using Windows.Storage.Streams;

using WinUIEx;
using WinRT.Interop;

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

        private readonly string[] FileTypes = { ".jpg", ".jpeg", ".bmp", ".png", ".gif", ".tif", ".ico", ".webp" };
        private readonly string[] ReadOnlyTypes = { ".gif", ".ico" };

        private string[] FolderFiles;
        private int CurrentIndex;

        public string[] LaunchArgs;
        public MainWindow MainWindow;
        public WindowManager Manager;

        public BitmapImage CurrentImage { get; protected set; }
        public string CurrentFilePath { get; protected set; }

        /// <summary>
        /// Constructor.
        /// </summary>
        public Context()
        {
            _Instance = this;
        }

        /// <summary>
        /// Load image if program is opened with open with command.
        /// </summary>
        public void LoadDefaultImage()
        {
            if(LaunchArgs.Length > 0)
            {
                if(FileTypes.Any(x => LaunchArgs[0].EndsWith(x, true, null)))
                {
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
                FolderFiles = Directory.EnumerateFiles(Path.GetDirectoryName(CurrentFilePath), "*.*", SearchOption.TopDirectoryOnly).Where(s => FileTypes.Any(x => s.EndsWith(x, true, null))).OrderBy(s => s, new NaturalStringComparer()).ToArray();
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

            InitializeWithWindow.Initialize(OpenFilePicker, WinRT.Interop.WindowNative.GetWindowHandle(MainWindow));
            StorageFile file = await OpenFilePicker.PickSingleFileAsync();

            if(file != null)
            {
                using IRandomAccessStream fileStream = await file.OpenAsync(Windows.Storage.FileAccessMode.Read);

                CreateImage();
                CurrentFilePath = file.Path;

                await CurrentImage.SetSourceAsync(fileStream);
                MainWindow.ImageView.Source = CurrentImage;

                LoadDirectoryFiles();
                file = null;
            }
        }

        /// <summary>
        /// Load an image from path.
        /// </summary>
        public bool LoadImageFromString(string image_path)
        {
            if(File.Exists(image_path))
            {
                CreateImage();

                CurrentImage.UriSource = new(image_path);
                CurrentFilePath = CurrentImage.UriSource.LocalPath;
                MainWindow.ImageView.Source = CurrentImage;

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

            Image tmp_image;
            FileInfo fileinfo = new(CurrentFilePath);

            dict[ImageInfos.FileName] = Path.GetFileName(CurrentFilePath);
            dict[ImageInfos.FileDate] = File.GetLastWriteTime(CurrentFilePath).ToString();
            dict[ImageInfos.ImageDimensions] = String.Concat(CurrentImage.PixelWidth, " x ", CurrentImage.PixelHeight);
            dict[ImageInfos.ImageSize] = HumanizeBytes(fileinfo.Length);
            dict[ImageInfos.FolderPath] = Path.GetDirectoryName(CurrentFilePath);

            try
            {
                if(IsWebp())
                {
                    using WebP webp = new();
                    tmp_image = webp.Load(CurrentFilePath);
                }
                else
                {
                    tmp_image = Image.FromFile(CurrentFilePath);
                }

                dict[ImageInfos.ImageDpi] = string.Concat(Math.Round(tmp_image.HorizontalResolution, 2).ToString(), " dpi");
                dict[ImageInfos.ImageDepth] = string.Concat(Image.GetPixelFormatSize(tmp_image.PixelFormat).ToString(), " bit");
                tmp_image.Dispose();
            }
            catch(Exception)
            {
            }

            return dict;
        }

        /// <summary>
        /// Delete current image.
        /// </summary>
        public void DeleteImage()
        {
            try
            {
                File.Delete(CurrentFilePath);
                FolderFiles = FolderFiles.RemoveAtIndex(CurrentIndex);

                if(FolderFiles != null && FolderFiles.Length > 0)
                {
                    LoadNextImage();
                }
                else
                {
                    CurrentImage = null;
                    MainWindow.TextBlockAppTitle.Text = GetProductName();
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
        /// Check if current file is webp.
        /// </summary>
        public bool IsWebp()
        {
            return Path.GetExtension(CurrentFilePath).ToLower() == ".webp";
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

            float zoom_factor = GetAdjustedZoomFactor();

            Thread.Sleep(50);
            MainWindow.ScrollView.ChangeView(0, 0, zoom_factor, true);
            MainWindow.ScrollView.ZoomToFactor(zoom_factor);
        }

        /// <summary>
        /// Get the zoom factor to fit image inside image view.
        /// </summary>
        public float GetAdjustedZoomFactor()
        {
            float zoom_factor = 1;

            if(CurrentImage == null) return zoom_factor;

            if(CurrentImage.PixelHeight > MainWindow.ImageContainer.ActualHeight || CurrentImage.PixelWidth > MainWindow.ImageContainer.ActualWidth)
            {
                double zoom_factory_h = MainWindow.ImageContainer.ActualHeight / CurrentImage.PixelHeight;
                double zoom_factory_w = MainWindow.ImageContainer.ActualWidth / CurrentImage.PixelWidth;

                zoom_factor = (float)((zoom_factory_h < zoom_factory_w) ? zoom_factory_h : zoom_factory_w);
            }

            return zoom_factor;
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
            Bitmap bmp;

            if(IsWebp())
            {
                using WebP webp = new();
                bmp = webp.Load(CurrentFilePath);
            }
            else
            {
                bmp = (Bitmap)Image.FromFile(CurrentFilePath);
            }

            bmp.RotateFlip(angle);

            if(IsWebp())
            {
                using WebP webp = new();
                webp.Save(bmp, CurrentFilePath, 100);
            }
            else
            {
                bmp.Save(CurrentFilePath);
            }

            bmp.Dispose();

            LoadImageFromString(CurrentFilePath);
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
                MainWindow.ButtonImageTransform.IsEnabled = !ReadOnlyTypes.Contains(Path.GetExtension(CurrentFilePath));
                
                MainWindow.ButtonImageDelete.IsEnabled = true;
                MainWindow.ButtonFileInfo.IsEnabled = true;
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
                MainWindow.ButtonFileInfo.IsEnabled = false;
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

            InitializeWithWindow.Initialize(SaveFilePicker, WinRT.Interop.WindowNative.GetWindowHandle(MainWindow));
            StorageFile output_file = await SaveFilePicker.PickSaveFileAsync();

            if(output_file != null)
            {
                Bitmap bmp;

                if(IsWebp())
                {
                    using WebP webp = new();
                    bmp = webp.Load(CurrentFilePath);
                }
                else
                {
                    bmp = (Bitmap)Image.FromFile(CurrentFilePath);
                }

                switch(output_file.ContentType)
                {
                    case "image/jpeg":
                        bmp.SaveJpeg(output_file.Path, 100);
                        break;

                    case "image/png":
                        bmp.Save(output_file.Path, ImageFormat.Png);
                        break;
                }

                bmp.Dispose();
                LoadDirectoryFiles();
            }
        }

        /// <summary>
        /// Event: When image is loaded.
        /// </summary>
        private void CurrentImage_ImageOpened(object sender, RoutedEventArgs e)
        {
            MainWindow.TextBlockAppTitle.Text = string.Concat(Path.GetFileName(CurrentFilePath), " - ", GetProductName());
            MainWindow.ImageLoadingIndicator.IsActive = false;

            UpdateButtonsAccessiblity();
            AdjustImage();

            Thread.Sleep(50);
            MainWindow.ImageView.Opacity = 1;

            if(MainWindow.SplitViewContainer.IsPaneOpen)
            {
                UpdateFileInfo();
            }
        }

        /// <summary>
        /// Create new image object.
        /// </summary>
        private void CreateImage()
        {
            MainWindow.ImageView.Opacity = 0;
            MainWindow.ImageLoadingIndicator.IsActive = true;

            CurrentImage = null;
            CurrentImage = new()
            {
                CreateOptions = BitmapCreateOptions.IgnoreImageCache
            };
            CurrentImage.ImageOpened += CurrentImage_ImageOpened;
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
            return _Instance;
        }
    }
}
