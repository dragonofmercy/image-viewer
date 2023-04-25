using System;
using System.IO;
using System.Linq;
using System.Drawing.Imaging;
using System.Threading;

using Windows.Storage.Streams;

using SixLabors.ImageSharp;
using SixLabors.ImageSharp.Processing;
using SixLabors.ImageSharp.Advanced;
using SixLabors.ImageSharp.Formats;
using SixLabors.ImageSharp.Formats.Jpeg;
using SixLabors.ImageSharp.Formats.Png;
using SixLabors.ImageSharp.Formats.Tga;
using Svg;

using ImageSharpImage = SixLabors.ImageSharp.Image;
using DrawingImage = System.Drawing.Bitmap;

using ImageViewer.Utilities;

namespace ImageViewer.Wrapper
{
    internal class Image
    {
        public static readonly string[] SupportedFileTypes = { ".jpg", ".jpeg", ".bmp", ".png", ".gif", ".tif", ".tiff", ".tga", ".ico", ".webp", ".svg" };
        public static readonly string[] SaveFileTypes = { ".jpg", ".png", ".webp", ".bmp", ".gif", ".tiff", ".tga" };

        private readonly string[] NativeExtensions = { ".jpg", ".jpeg", ".bmp", ".png", ".gif", ".tif", ".tiff", ".tga", ".webp" };

        public event EventHandler ImageLoaded;
        public event EventHandler ImageFailed;

        protected bool WorkingImageLoaded;
        protected ImageSharpImage WorkingImage;
        protected IImageEncoder Encoder = new JpegEncoder { Quality = 100 };

        public void Load(string path)
        {
            LoadImageFromPath(path);
        }

        public void Load(IInputStream stream)
        {
            LoadImageFromMemory(stream);
        }

        public bool Loaded => WorkingImageLoaded;
        public double Height => WorkingImage.Height;
        public double Width => WorkingImage.Width;

        public void Dispose()
        {
            WorkingImage?.Dispose();
            WorkingImageLoaded = false;
        }

        public void RotateFlip(RotateMode rotateMode, FlipMode flipMode)
        {
            if (!WorkingImageLoaded) return;
            WorkingImage.Mutate(x => x.RotateFlip(rotateMode, flipMode));
        }

        public string GetImageDimensionsAsString()
        {
            if (!WorkingImageLoaded) return "";
            return WorkingImage.Width + " x " + WorkingImage.Height;
        }

        public string GetDepthAsString()
        {
            if (!WorkingImageLoaded) return "";
            return WorkingImage.PixelType.BitsPerPixel + " bit";
        }

        public IRandomAccessStream GetBitmapImageSource()
        {
            if (WorkingImage == null) return null;

            MemoryStream memory = new();
            WorkingImage.Save(memory, Encoder);
            memory.Position = 0;

            return memory.AsRandomAccessStream();
        }

        public async void Save(string path, string type)
        {
            switch (type)
            {
                case ".jpg":
                    await WorkingImage.SaveAsJpegAsync(path, new JpegEncoder { Quality = 100 });
                    break;

                case ".png":
                    await WorkingImage.SaveAsPngAsync(path);
                    break;

                case ".webp":
                    await WorkingImage.SaveAsWebpAsync(path);
                    break;

                case ".bmp":
                    await WorkingImage.SaveAsBmpAsync(path);
                    break;

                case ".gif":
                    await WorkingImage.SaveAsGifAsync(path);
                    break;

                case ".tga":
                    await WorkingImage.SaveAsTgaAsync(path);
                    break;

                case ".tiff":
                    await WorkingImage.SaveAsTiffAsync(path);
                    break;
            }
        }

        private async void LoadImageFromPath(string path)
        {
            try
            {
                string extension = Path.GetExtension(path).ToLower();

                if (NativeExtensions.Contains(extension))
                {
                    WorkingImage = await ImageSharpImage.LoadAsync(path, CancellationToken.None);
                    Encoder = WorkingImage.DetectEncoder(path);

                    switch (Encoder)
                    {
                        case TgaEncoder:
                            // Change TgaEncoder to PngEncoder because Image UI Component don't support TGA format
                            Encoder = new PngEncoder();
                            break;
                        case JpegEncoder:
                            Encoder = new JpegEncoder { Quality = 100 };
                            break;
                    }
                }
                else
                {
                    DrawingImage tmp;

                    if (extension == ".svg")
                    {
                        SvgDocument svgDocument = SvgDocument.Open(path);
                        svgDocument.ShapeRendering = SvgShapeRendering.Auto;
                        tmp = svgDocument.AdjustSize(1024, 1024).Draw();

                        Encoder = new PngEncoder();
                    }
                    else
                    {
                        byte[] fileBytes = await File.ReadAllBytesAsync(path);
                        using MemoryStream defaultMemoryStream = new(fileBytes);
                        tmp = (DrawingImage)System.Drawing.Image.FromStream(defaultMemoryStream);
                        await defaultMemoryStream.DisposeAsync();
                    }

                    using MemoryStream saveMemoryStream = new();
                    tmp.Save(saveMemoryStream, ImageFormat.Png);
                    WorkingImage = ImageSharpImage.Load(saveMemoryStream.ToArray());
                    await saveMemoryStream.DisposeAsync();
                }

                WorkingImageLoaded = true;
                ImageLoaded?.Invoke(this, EventArgs.Empty);
            }
            catch (Exception e)
            {
                ImageFailedEventArgs args = new()
                {
                    Message = e.Message,
                    Path = path
                };

                ImageFailed?.Invoke(this, args);
            }
        }

        private async void LoadImageFromMemory(IInputStream stream)
        {
            try
            {
                WorkingImage = await ImageSharpImage.LoadAsync(stream.AsStreamForRead());
                Encoder = new PngEncoder();

                WorkingImageLoaded = true;
                ImageLoaded?.Invoke(this, EventArgs.Empty);
            }
            catch (Exception e)
            {
                ImageFailedEventArgs args = new()
                {
                    Message = e.Message
                };

                ImageFailed?.Invoke(this, args);
            }
        }
    }

    public class ImageFailedEventArgs : EventArgs
    {
        public string Message { get; set; }
        public string Path { get; set; }
    }
}
