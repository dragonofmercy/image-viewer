using System;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices.WindowsRuntime;
using System.Threading;
using System.Threading.Tasks;

using Microsoft.UI.Xaml.Media.Imaging;

using Windows.Graphics.Imaging;
using Windows.Storage.Streams;

using SixLabors.ImageSharp;
using SixLabors.ImageSharp.Advanced;
using SixLabors.ImageSharp.Formats;
using SixLabors.ImageSharp.Formats.Jpeg;
using SixLabors.ImageSharp.Formats.Png;
using SixLabors.ImageSharp.Formats.Tga;
using SixLabors.ImageSharp.Formats.Webp;
using SixLabors.ImageSharp.PixelFormats;
using SixLabors.ImageSharp.Processing;

using SkiaSharp;
using Svg.Skia;

using ImageSharpImage = SixLabors.ImageSharp.Image;

namespace ImageViewer.Wrapper;

internal partial class Image
{
    public static readonly string[] SupportedFileTypes = [".jpg", ".jpeg", ".bmp", ".png", ".gif", ".tif", ".tiff", ".tga", ".ico", ".webp", ".svg"];
    public static readonly string[] SaveFileTypes = [".jpg", ".png", ".webp", ".bmp", ".gif", ".tiff", ".tga"];

    private readonly string[] NativeExtensions = [".jpg", ".jpeg", ".bmp", ".png", ".gif", ".tif", ".tiff", ".tga", ".webp"];

    // Longest edge an SVG is rasterized to. Vector input has no natural pixel size; this caps
    // the decode cost while staying above any realistic window size.
    private const float SVG_MAX_RASTER_SIZE = 1024;

    public event EventHandler ImageLoaded;
    public event EventHandler ImageFailed;

    protected bool WorkingImageLoaded;
    protected bool Disposed;
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
    public bool IsAnimated => WorkingImage is { Frames.Count: > 1 };
    public bool Modified { get; private set; }

    public void Dispose()
    {
        Disposed = true;
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

    /// <summary>
    /// Copy the working image as top-down BGRA32 pixels, the order the clipboard DIB path expects.
    /// </summary>
    public byte[] GetBgra32Pixels(out int width, out int height)
    {
        width = WorkingImage.Width;
        height = WorkingImage.Height;

        byte[] pixels = new byte[width * height * 4];

        using(SixLabors.ImageSharp.Image<Bgra32> converted = WorkingImage.CloneAs<Bgra32>())
        {
            converted.CopyPixelDataTo(pixels);
        }

        return pixels;
    }

    public WriteableBitmap GetWriteableBitmap()
    {
        if(WorkingImage == null) return null;

        byte[] pixels = GetBgra32Pixels(out int width, out int height);

        // XAML composition expects premultiplied alpha
        for(int i = 0; i < pixels.Length; i += 4)
        {
            byte alpha = pixels[i + 3];

            if(alpha == 255) continue;

            pixels[i] = (byte)(pixels[i] * alpha / 255);
            pixels[i + 1] = (byte)(pixels[i + 1] * alpha / 255);
            pixels[i + 2] = (byte)(pixels[i + 2] * alpha / 255);
        }

        WriteableBitmap bitmap = new(width, height);

        using(Stream buffer = bitmap.PixelBuffer.AsStream())
        {
            buffer.Write(pixels, 0, pixels.Length);
        }

        bitmap.Invalidate();
        return bitmap;
    }

    public async Task Save(string path, string type, int? quality = null)
    {
        switch(type)
        {
            case ".jpg":
                await WorkingImage.SaveAsJpegAsync(path, new JpegEncoder { Quality = quality ?? 100 });
                break;

            case ".png":
                await WorkingImage.SaveAsPngAsync(path);
                break;

            case ".webp":
                await WorkingImage.SaveAsWebpAsync(path, new WebpEncoder { FileFormat = WebpFileFormatType.Lossy, Quality = quality ?? 100 });
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

            default:
                throw new NotSupportedException($"Unsupported save format: {type}");
        }

        Modified = false;
    }

    private async void LoadImageFromPath(string path)
    {
        try
        {
            string extension = Path.GetExtension(path).ToLower();

            if(NativeExtensions.Contains(extension))
            {
                WorkingImage = await ImageSharpImage.LoadAsync(path, CancellationToken.None);

                // Apply EXIF orientation so portrait photos are not displayed sideways.
                // No-op when the image carries no orientation metadata.
                WorkingImage.Mutate(x => x.AutoOrient());

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
                        // Keep the default truecolor PngEncoder: forcing PngColorType.Palette
                        // capped truecolor PNGs at 256 colors when re-encoded (animated/clipboard paths)
                        Encoder = new PngEncoder();
                        break;
                }
            }
            else
            {
                // Formats ImageSharp cannot decode. Both paths rasterize to BGRA pixels and hand
                // them to ImageSharp directly, so the rest of the class sees a normal image.
                WorkingImage = extension == ".svg" ? RasterizeSvg(path) : await DecodeWithWicAsync(path);

                Encoder = new PngEncoder();
            }

            // Load completed after Dispose (user navigated away): drop the decoded image silently
            if(Disposed)
            {
                WorkingImage?.Dispose();
                WorkingImage = null;
                return;
            }

            WorkingImageLoaded = true;
            ImageLoaded?.Invoke(this, EventArgs.Empty);
        }
        catch(Exception e)
        {
            if(Disposed) return;

            ImageFailedEventArgs args = new()
            {
                Message = e.Message,
                Path = path
            };

            ImageFailed?.Invoke(this, args);
        }
    }

    /// <summary>
    /// Rasterize an SVG through Skia. Scaled down to fit <see cref="SVG_MAX_RASTER_SIZE"/> on both
    /// edges, never up: a vector file smaller than the cap keeps its authored pixel size.
    /// </summary>
    private static ImageSharpImage RasterizeSvg(string path)
    {
        using SKSvg svg = new();

        if(svg.Load(path) == null) throw new InvalidOperationException($"Cannot parse SVG: {path}");

        SKRect bounds = svg.Picture.CullRect;

        if(bounds.Width <= 0 || bounds.Height <= 0) throw new InvalidOperationException($"SVG has no drawable area: {path}");

        float scale = Math.Min(1f, Math.Min(SVG_MAX_RASTER_SIZE / bounds.Width, SVG_MAX_RASTER_SIZE / bounds.Height));
        int width = Math.Max(1, (int)(bounds.Width * scale));
        int height = Math.Max(1, (int)(bounds.Height * scale));

        // Unpremultiplied BGRA is exactly ImageSharp's Bgra32 layout, so the pixels transfer as-is.
        using SKBitmap bitmap = new(new SKImageInfo(width, height, SKColorType.Bgra8888, SKAlphaType.Unpremul));

        using(SKCanvas canvas = new(bitmap))
        {
            canvas.Clear(SKColors.Transparent);
            canvas.Scale(scale);
            // CullRect is not always anchored at the origin: shift it there before drawing.
            canvas.Translate(-bounds.Left, -bounds.Top);
            canvas.DrawPicture(svg.Picture);
        }

        return ImageSharpImage.LoadPixelData<Bgra32>(bitmap.GetPixelSpan(), width, height);
    }

    /// <summary>
    /// Decode through WIC, the codec set Explorer itself uses. This covers ICO, which ImageSharp
    /// 3.x has no decoder for, without any third-party imaging dependency.
    /// </summary>
    private static async Task<ImageSharpImage> DecodeWithWicAsync(string path)
    {
        using MemoryStream source = new(await File.ReadAllBytesAsync(path));

        BitmapDecoder decoder = await BitmapDecoder.CreateAsync(source.AsRandomAccessStream());

        // An .ico packs several sizes in one file. Show the biggest, not whichever the container
        // happens to list first, otherwise a 16x16 entry can win over a 256x256 one.
        BitmapFrame frame = await decoder.GetFrameAsync(0);

        for(uint i = 1; i < decoder.FrameCount; i++)
        {
            BitmapFrame candidate = await decoder.GetFrameAsync(i);
            if(candidate.PixelWidth > frame.PixelWidth) frame = candidate;
        }

        PixelDataProvider pixels = await frame.GetPixelDataAsync(
            BitmapPixelFormat.Bgra8,
            BitmapAlphaMode.Straight,
            new BitmapTransform(),
            ExifOrientationMode.IgnoreExifOrientation,
            ColorManagementMode.DoNotColorManage);

        return ImageSharpImage.LoadPixelData<Bgra32>(pixels.DetachPixelData(), (int)frame.PixelWidth, (int)frame.PixelHeight);
    }

    private async void LoadImageFromMemory(IInputStream stream)
    {
        try
        {
            WorkingImage = await ImageSharpImage.LoadAsync(stream.AsStreamForRead());
            Encoder = new PngEncoder();

            // Load completed after Dispose (user navigated away): drop the decoded image silently
            if(Disposed)
            {
                WorkingImage?.Dispose();
                WorkingImage = null;
                return;
            }

            WorkingImageLoaded = true;
            // Memory/clipboard source has no backing file: start dirty so the title shows the
            // unsaved indicator and Save routes to Save As (cleared by the first Save).
            Modified = true;
            ImageLoaded?.Invoke(this, EventArgs.Empty);
        }
        catch(Exception e)
        {
            if(Disposed) return;

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