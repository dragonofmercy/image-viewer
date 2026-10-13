using System;
using System.Collections.Generic;
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
using SixLabors.ImageSharp.Formats.Ico;
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
    public static readonly string[] SupportedFileTypes = [".jpg", ".jpeg", ".bmp", ".png", ".gif", ".tif", ".tiff", ".tga", ".ico", ".webp", ".svg", ".jxl"];
    public static readonly string[] SaveFileTypes = [".jpg", ".png", ".webp", ".bmp", ".gif", ".tiff", ".tga", ".jxl"];

    // JPEG XL is encoded by the WIC codec of Microsoft's Store "JPEG XL Image Extension", when installed.
    private static readonly Lazy<Task<Guid?>> JxlEncoderId = new(FindJxlEncoderAsync);

    // WIC errors meaning "no usable codec": none registered, or registered but its package is gone
    // (an uninstalled Store extension stays listed until the next reboot).
    private const int WINCODEC_ERR_COMPONENTNOTFOUND = unchecked((int)0x88982F50);
    private const int WINCODEC_ERR_COMPONENTINITIALIZEFAILURE = unchecked((int)0x88982F8B);

    public static async Task<bool> CanSaveJxlAsync() => await JxlEncoderId.Value != null;

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

    // Every size packed in the current .ico, largest first. Null for any other format.
    // An icon frame tops out at 256x256 (256 KB of pixels), so holding them all costs
    // well under 2 MB even for a fully populated icon - cheap enough to make switching
    // sizes instant instead of re-reading the file.
    protected ImageSharpImage[] IconFrames;

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

    /// <summary>An .ico holding more than one size: the only case where the size strip is shown.</summary>
    public bool HasIconSizes => IconFrames is { Length: > 1 };
    public int IconSizeCount => IconFrames?.Length ?? 0;
    public int IconSizeIndex { get; private set; }

    public (int Width, int Height) GetIconSize(int index) => (IconFrames[index].Width, IconFrames[index].Height);

    /// <summary>
    /// Display another size of the current icon. The frames are already decoded, so this is a
    /// reference swap: any transform applied to a frame stays on that frame.
    /// </summary>
    public void SelectIconSize(int index)
    {
        if(IconFrames == null || index < 0 || index >= IconFrames.Length) return;

        IconSizeIndex = index;
        WorkingImage = IconFrames[index];
    }

    public WriteableBitmap GetIconSizeThumbnail(int index) => ToWriteableBitmap(IconFrames[index]);

    public void Dispose()
    {
        Disposed = true;
        DisposeWorkingImages();
        WorkingImageLoaded = false;
    }

    /// <summary>
    /// Release the decoded pixels. WorkingImage is one of <see cref="IconFrames"/> when an icon is
    /// open, so the frames own the disposal in that case.
    /// </summary>
    private void DisposeWorkingImages()
    {
        if(IconFrames != null)
        {
            foreach(ImageSharpImage frame in IconFrames)
            {
                frame.Dispose();
            }

            IconFrames = null;
        }
        else
        {
            WorkingImage?.Dispose();
        }

        WorkingImage = null;
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
    public byte[] GetBgra32Pixels(out int width, out int height) => GetBgra32Pixels(WorkingImage, out width, out height);

    private static byte[] GetBgra32Pixels(ImageSharpImage source, out int width, out int height)
    {
        width = source.Width;
        height = source.Height;

        byte[] pixels = new byte[width * height * 4];

        using(SixLabors.ImageSharp.Image<Bgra32> converted = source.CloneAs<Bgra32>())
        {
            converted.CopyPixelDataTo(pixels);
        }

        return pixels;
    }

    public WriteableBitmap GetWriteableBitmap() => ToWriteableBitmap(WorkingImage);

    private static WriteableBitmap ToWriteableBitmap(ImageSharpImage source)
    {
        if(source == null) return null;

        byte[] pixels = GetBgra32Pixels(source, out int width, out int height);

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

            case ".jxl":
                await EncodeWicAsync(path, await JxlEncoderId.Value ?? throw new NotSupportedException("JPEG XL encoder not installed"));
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
            string extension = Path.GetExtension(path).ToLowerInvariant();

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
            else if(extension == ".svg")
            {
                // ImageSharp cannot decode SVG: Skia rasterizes it to BGRA pixels handed to ImageSharp
                // directly, so the rest of the class sees a normal image.
                WorkingImage = RasterizeSvg(path);
                Encoder = new PngEncoder();
            }
            else if(extension == ".ico")
            {
                IconFrames = await DecodeIconFramesAsync(path);
                IconSizeIndex = 0;
                WorkingImage = IconFrames[0];
                Encoder = new PngEncoder();
            }
            else if(extension == ".jxl")
            {
                WorkingImage = await DecodeWicAsync(path);
                Encoder = new PngEncoder();
            }
            else
            {
                // Routed through ImageFailed so the caller shows the "cannot open this file" message
                throw new NotSupportedException($"Unsupported file type: {extension}");
            }

            // Load completed after Dispose (user navigated away): drop the decoded image silently
            if(Disposed)
            {
                DisposeWorkingImages();
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
                Path = path,
                CodecMissing = e.HResult is WINCODEC_ERR_COMPONENTNOTFOUND or WINCODEC_ERR_COMPONENTINITIALIZEFAILURE
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
    /// An .ico packs several sizes in one file, in no guaranteed order. ImageSharp decodes each entry
    /// as a frame padded top-left onto a canvas the size of the largest one, so every frame is cropped
    /// back to its own size (directory value, 0 meaning the full canvas). The list is sorted largest
    /// first, so the biggest opens by default (a container listing 16x16 first must not win over a
    /// 256x256 entry) and the size strip reads top-down.
    /// </summary>
    private static async Task<ImageSharpImage[]> DecodeIconFramesAsync(string path)
    {
        using ImageSharpImage icon = await ImageSharpImage.LoadAsync(path, CancellationToken.None);

        List<ImageSharpImage> frames = new(icon.Frames.Count);

        for(int i = 0; i < icon.Frames.Count; i++)
        {
            IcoFrameMetadata metadata = icon.Frames[i].Metadata.GetIcoMetadata();
            int width = metadata.EncodingWidth == 0 ? icon.Width : Math.Min((int)metadata.EncodingWidth, icon.Width);
            int height = metadata.EncodingHeight == 0 ? icon.Height : Math.Min((int)metadata.EncodingHeight, icon.Height);

            ImageSharpImage frame = icon.Frames.CloneFrame(i);
            frame.Mutate(x => x.Crop(width, height));
            frames.Add(frame);
        }

        // OrderByDescending is stable, so two entries of the same size (an icon can hold 32x32 in
        // both 8-bit and 32-bit) keep their file order instead of being shuffled.
        return [.. frames.OrderByDescending(frame => (long)frame.Width * frame.Height)];
    }

    /// <summary>
    /// Decode the first frame through WIC. Used for JPEG XL, which neither ImageSharp nor Skia read:
    /// the codec is Microsoft's "JPEG XL Image Extension" (Store, Windows 11 24H2+). Without it,
    /// BitmapDecoder throws and the user gets the usual "cannot open this file" message.
    /// </summary>
    private static async Task<ImageSharpImage> DecodeWicAsync(string path)
    {
        using MemoryStream source = new(await File.ReadAllBytesAsync(path));

        BitmapDecoder decoder = await BitmapDecoder.CreateAsync(source.AsRandomAccessStream());
        BitmapFrame frame = await decoder.GetFrameAsync(0);

        PixelDataProvider pixels = await frame.GetPixelDataAsync(
            BitmapPixelFormat.Bgra8,
            BitmapAlphaMode.Straight,
            new BitmapTransform(),
            ExifOrientationMode.RespectExifOrientation,
            ColorManagementMode.ColorManageToSRgb);

        return ImageSharpImage.LoadPixelData<Bgra32>(pixels.DetachPixelData(), (int)frame.OrientedPixelWidth, (int)frame.OrientedPixelHeight);
    }

    /// <summary>
    /// The codec list is not enough: an uninstalled Store extension stays registered until the next
    /// reboot, so only a real instantiation tells whether the encoder works.
    /// </summary>
    private static async Task<Guid?> FindJxlEncoderAsync()
    {
        BitmapCodecInformation codec = BitmapEncoder.GetEncoderInformationEnumerator()
            .FirstOrDefault(c => c.FileExtensions.Contains(".jxl", StringComparer.OrdinalIgnoreCase));

        if(codec == null) return null;

        try
        {
            using InMemoryRandomAccessStream probe = new();
            await BitmapEncoder.CreateAsync(codec.CodecId, probe);
            return codec.CodecId;
        }
        catch(Exception)
        {
            return null;
        }
    }

    private async Task EncodeWicAsync(string path, Guid encoderId)
    {
        byte[] pixels = GetBgra32Pixels(out int width, out int height);

        using FileStream file = File.Create(path);
        BitmapEncoder encoder = await BitmapEncoder.CreateAsync(encoderId, file.AsRandomAccessStream());
        encoder.SetPixelData(BitmapPixelFormat.Bgra8, BitmapAlphaMode.Straight, (uint)width, (uint)height, 96, 96, pixels);
        await encoder.FlushAsync();
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
                DisposeWorkingImages();
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
    public bool CodecMissing { get; init; }
}