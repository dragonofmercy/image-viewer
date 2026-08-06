using System;
using System.IO;
using System.Linq;
using System.Threading.Tasks;

using SixLabors.ImageSharp;
using SixLabors.ImageSharp.Advanced;
using SixLabors.ImageSharp.Metadata.Profiles.Exif;
using SixLabors.ImageSharp.PixelFormats;

using ImageViewer.Wrapper;

using ViewerImage = ImageViewer.Wrapper.Image;

namespace ImageViewer.Tests;

/// <summary>Unique temp directory deleted on Dispose.</summary>
public sealed class TempDir : IDisposable
{
    public string Path { get; }

    public TempDir()
    {
        Path = System.IO.Path.Combine(System.IO.Path.GetTempPath(), "ImageViewerTests", Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(Path);
    }

    public string File(string name) => System.IO.Path.Combine(Path, name);

    public void Dispose()
    {
        try { Directory.Delete(Path, recursive: true); }
        catch { /* best effort cleanup */ }
    }
}

/// <summary>Generates image fixtures on disk so tests own no committed binaries.</summary>
public static class FixtureFactory
{
    public static string Save(TempDir dir, string fileName, int width, int height)
    {
        string path = dir.File(fileName);
        using Image<Rgba32> image = new(width, height);
        image.Save(path); // encoder inferred from the extension
        return path;
    }

    public static string SaveJpegOrientation6(TempDir dir, string fileName, int width, int height)
    {
        string path = dir.File(fileName);
        using Image<Rgba32> image = new(width, height);
        image.Metadata.ExifProfile = new ExifProfile();
        image.Metadata.ExifProfile.SetValue(ExifTag.Orientation, (ushort)6); // rotate 90 CW => width/height swap after AutoOrient
        image.SaveAsJpeg(path);
        return path;
    }

    public static string SaveSvg(TempDir dir, string fileName, int width, int height)
    {
        string path = dir.File(fileName);
        string svg = $"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\"><rect width=\"{width}\" height=\"{height}\" fill=\"red\"/></svg>";
        System.IO.File.WriteAllText(path, svg);
        return path;
    }

    public static string SaveNoisy(TempDir dir, string fileName, int width, int height)
    {
        string path = dir.File(fileName);
        using Image<Rgba32> image = new(width, height);
        image.ProcessPixelRows(accessor =>
        {
            for (int y = 0; y < accessor.Height; y++)
            {
                Span<Rgba32> row = accessor.GetRowSpan(y);
                for (int x = 0; x < row.Length; x++)
                {
                    // High-frequency deterministic pattern so JPEG/WebP quality changes file size.
                    byte r = (byte)((x * 37 + y * 17) & 0xFF);
                    byte g = (byte)((x * 13 + y * 53) & 0xFF);
                    byte b = (byte)((x * 91 + y * 7) & 0xFF);
                    row[x] = new Rgba32(r, g, b, 255);
                }
            }
        });
        image.Save(path);
        return path;
    }

    /// <summary>
    /// Write a multi-size .ico. ImageSharp 3.x cannot encode ICO and the app no longer depends on
    /// System.Drawing, so the container is emitted by hand: ICONDIR, one ICONDIRENTRY per size,
    /// then PNG payloads (WIC decodes PNG-compressed icon entries).
    /// Sizes are written in the given order so tests can check the decoder does not just take
    /// the first frame.
    /// </summary>
    public static string SaveIco(TempDir dir, string fileName, params int[] sizes)
    {
        string path = dir.File(fileName);

        byte[][] frames = sizes.Select(size =>
        {
            using Image<Rgba32> image = new(size, size);
            using MemoryStream png = new();
            image.SaveAsPng(png);
            return png.ToArray();
        }).ToArray();

        using FileStream stream = System.IO.File.Create(path);
        using BinaryWriter writer = new(stream);

        writer.Write((ushort)0);                // reserved
        writer.Write((ushort)1);                // type: icon
        writer.Write((ushort)frames.Length);

        int offset = 6 + 16 * frames.Length;    // header + directory

        for (int i = 0; i < frames.Length; i++)
        {
            byte dimension = (byte)(sizes[i] >= 256 ? 0 : sizes[i]); // 0 encodes 256
            writer.Write(dimension);            // width
            writer.Write(dimension);            // height
            writer.Write((byte)0);              // palette size
            writer.Write((byte)0);              // reserved
            writer.Write((ushort)1);            // colour planes
            writer.Write((ushort)32);           // bits per pixel
            writer.Write(frames[i].Length);
            writer.Write(offset);

            offset += frames[i].Length;
        }

        foreach (byte[] frame in frames)
        {
            writer.Write(frame);
        }

        return path;
    }
}

/// <summary>Bridges Wrapper.Image's event-based async load to an awaitable Task.</summary>
// internal because the return type ViewerImage (ImageViewer.Wrapper.Image) is internal,
// reachable here only through InternalsVisibleTo("ImageViewer.Tests").
internal static class ImageLoader
{
    public static Task<ViewerImage> LoadAsync(string path, int timeoutMs = 15000)
        => LoadAsync(img => img.Load(path), path, timeoutMs);

    public static Task<ViewerImage> LoadAsync(Windows.Storage.Streams.IInputStream stream, int timeoutMs = 15000)
        => LoadAsync(img => img.Load(stream), "memory stream", timeoutMs);

    private static async Task<ViewerImage> LoadAsync(Action<ViewerImage> load, string what, int timeoutMs)
    {
        ViewerImage image = new();
        TaskCompletionSource<bool> tcs = new(TaskCreationOptions.RunContinuationsAsynchronously);

        void OnLoaded(object sender, EventArgs e) => tcs.TrySetResult(true);
        void OnFailed(object sender, EventArgs e)
        {
            string message = (e as ImageFailedEventArgs)?.Message ?? "unknown error";
            tcs.TrySetException(new InvalidOperationException("Image load failed: " + message));
        }

        image.ImageLoaded += OnLoaded;
        image.ImageFailed += OnFailed;
        load(image);

        Task completed = await Task.WhenAny(tcs.Task, Task.Delay(timeoutMs));
        if (completed != tcs.Task)
        {
            throw new TimeoutException("Image load timed out: " + what);
        }

        await tcs.Task; // surface any load exception
        return image;
    }
}
