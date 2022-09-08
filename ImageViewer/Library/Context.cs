using System;
using System.IO;
using System.Linq;
using System.Collections.Generic;
using System.Drawing;
using System.Drawing.Imaging;
using System.Diagnostics;

using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media.Imaging;

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
        private bool MemoryOnly = false;

        public string[] LaunchArgs;
        public MainWindow MainWindow;
        public WindowManager Manager;

        public Bitmap CurrentImage { get; protected set; }
        public string CurrentFilePath { get; protected set; }

        /// <summary>
        /// Constructor.
        /// </summary>
        public Context()
        {
            _Instance = this;
        }

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
        public void LoadDefaultImage()
        {
            if(LaunchArgs.Length > 0)
            {
                if(CheckFileExtension(LaunchArgs[0]))
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

            InitializeWithWindow.Initialize(OpenFilePicker, WindowNative.GetWindowHandle(MainWindow));
            StorageFile selected_file = await OpenFilePicker.PickSingleFileAsync();

            if(selected_file != null && CheckFileExtension(selected_file.Path))
            {
                CurrentFilePath = selected_file.Path;
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
            dict[ImageInfos.ImageDimensions] = String.Concat(CurrentImage.Width, " x ", CurrentImage.Height);
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
                    File.Delete(CurrentFilePath);
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
        /// Check if current file is webp.
        /// </summary>
        public bool IsWebp()
        {
            return CurrentFilePath != null && Path.GetExtension(CurrentFilePath).ToLower() == ".webp";
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

            if(CurrentImage.Height > MainWindow.ImageContainer.ActualHeight || CurrentImage.Width > MainWindow.ImageContainer.ActualWidth)
            {
                double zoom_factory_h = MainWindow.ImageContainer.ActualHeight / CurrentImage.Height;
                double zoom_factory_w = MainWindow.ImageContainer.ActualWidth / CurrentImage.Width;

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
            StorageFile output_file = await SaveFilePicker.PickSaveFileAsync();

            if(output_file != null)
            {
                if(CurrentImage != null)
                {
                    switch(output_file.FileType)
                    {
                        case ".jpg":
                            CurrentImage.SaveJpeg(output_file.Path, 100);
                            break;

                        case ".png":
                            CurrentImage.Save(output_file.Path, ImageFormat.Png);
                            break;

                        case ".webp":
                            using(Wrapper.WebP webp = new())
                            {
                                webp.Save(CurrentImage, output_file.Path, 100);
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
                if(IsWebp())
                {
                    using Wrapper.WebP webp = new();
                    CurrentImage = webp.Load(CurrentFilePath);
                    webp.Dispose();
                }
                else
                {
                    byte[] bytes = File.ReadAllBytes(CurrentFilePath);
                    MemoryStream ms = new(bytes);
                    CurrentImage = (Bitmap)Image.FromStream(ms);
                }
            }
        }

        /// <summary>
        /// Load bitmap (CurrentImage) inside image view
        /// </summary>
        private void LoadImageView(bool UseUriSource = true)
        {
            BitmapImage bitmap_image = new()
            {
                CreateOptions = BitmapCreateOptions.IgnoreImageCache
            };
            bitmap_image.ImageOpened += CurrentImage_ImageOpened;
            bitmap_image.ImageFailed += CurrentImage_ImageFailed;

            if(UseUriSource)
            {
                bitmap_image.UriSource = new(CurrentFilePath);
            }
            else
            {
                using MemoryStream memory = new();
                CurrentImage.Save(memory, ImageFormat.Png);
                memory.Position = 0;
                bitmap_image.SetSource(memory.AsRandomAccessStream());
            }
            
            MainWindow.ImageView.Source = bitmap_image;
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

            MainWindow.ImageLoadingIndicator.IsActive = false;

            UpdateButtonsAccessiblity();
            AdjustImage();

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
            return _Instance;
        }
    }
}
