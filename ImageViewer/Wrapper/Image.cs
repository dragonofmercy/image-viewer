using System;
using System.Drawing;
using System.IO;
using System.Linq;
using System.Threading;

using Windows.Storage.Streams;

using SixLabors.ImageSharp;
using SixLabors.ImageSharp.Advanced;
using SixLabors.ImageSharp.Formats;
using SixLabors.ImageSharp.Formats.Jpeg;
using SixLabors.ImageSharp.Formats.Png;
using SixLabors.ImageSharp.Formats.Tga;
using SixLabors.ImageSharp.PixelFormats;
using Svg;

using ImageSharpImage = SixLabors.ImageSharp.Image;

using ImageViewer.Utilities;

namespace ImageViewer.Wrapper;

internal partial class Image
{
    public static readonly string[] SupportedFileTypes = [".jpg", ".jpeg", ".bmp", ".png", ".gif", ".tif", ".tiff", ".tga", ".ico", ".webp", ".svg"];
    public static readonly string[] SaveFileTypes = [".jpg", ".png", ".webp", ".bmp", ".gif", ".tiff", ".tga"];

    private readonly string[] NativeExtensions = [".jpg", ".jpeg", ".bmp", ".png", ".gif", ".tif", ".tiff", ".tga", ".webp"];

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
    
    public string GetImageDimensionsAsString()
    {
        if(!WorkingImageLoaded) return "";
        return WorkingImage.Width + " x " + WorkingImage.Height;
    }

    public string GetDepthAsString()
    {
        if(!WorkingImageLoaded) return "";
        return WorkingImage.PixelType.BitsPerPixel + " bit";
    }

    public IRandomAccessStream GetBitmapImageSource()
    {
        if(WorkingImage == null) return null;

        MemoryStream memory = new();
        WorkingImage.Save(memory, Encoder);
        memory.Position = 0;

        return memory.AsRandomAccessStream();
    }

    public async void Save(string path, string type)
    {
        switch(type)
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

            if(NativeExtensions.Contains(extension))
            {
                WorkingImage = await ImageSharpImage.LoadAsync(path, CancellationToken.None);
                Encoder = WorkingImage.DetectEncoder(path);

                switch(Encoder)
                {
                    case TgaEncoder:
                        // Change TgaEncoder to PngEncoder because Image UI Component don't support TGA format
                        Encoder = new PngEncoder();
                        break;
                    case JpegEncoder:
                        Encoder = new JpegEncoder { Quality = 100 };
                        break;

                    case PngEncoder:
                        Encoder = new PngEncoder { ColorType = PngColorType.Palette };
                        break;
                }
            }
            else
            {
                // Handle SVG and ICO formats
                // NOTE: Both Svg library and ICO loading still require System.Drawing.Common
                // This is kept minimal and isolated to this fallback path only
                if(extension == ".svg")
                {
                    // Convert SVG to PNG using Svg library (requires System.Drawing)
                    SvgDocument svgDocument = SvgDocument.Open(path);
                    svgDocument.ShapeRendering = SvgShapeRendering.Auto;

                    using MemoryStream svgMemoryStream = new();
                    using (Bitmap svgBitmap = svgDocument.AdjustSize(1024, 1024).Draw())
                    {
                        svgBitmap.Save(svgMemoryStream, System.Drawing.Imaging.ImageFormat.Png);
                    }

                    svgMemoryStream.Position = 0;
                    WorkingImage = await ImageSharpImage.LoadAsync<Rgba32>(svgMemoryStream);

                    Encoder = new PngEncoder { ColorType = PngColorType.Palette };
                }
                else
                {
                    // For ICO and other legacy formats (requires System.Drawing)
                    byte[] fileBytes = await File.ReadAllBytesAsync(path);
                    using MemoryStream inputStream = new(fileBytes);
                    using MemoryStream pngStream = new();

                    using (System.Drawing.Image drawingImage = System.Drawing.Image.FromStream(inputStream))
                    {
                        drawingImage.Save(pngStream, System.Drawing.Imaging.ImageFormat.Png);
                    }

                    pngStream.Position = 0;
                    WorkingImage = await ImageSharpImage.LoadAsync<Rgba32>(pngStream);

                    Encoder = new PngEncoder();
                }
            }

            WorkingImageLoaded = true;
            ImageLoaded?.Invoke(this, EventArgs.Empty);
        }
        catch(Exception e)
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
        catch(Exception e)
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
    public string Path { get; init; }
}