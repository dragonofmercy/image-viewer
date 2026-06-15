using System;
using System.IO;
using System.Threading.Tasks;

using SixLabors.ImageSharp;
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

    public static string SaveIco(TempDir dir, string fileName)
    {
        // ImageSharp cannot write ICO; use System.Drawing to emit a valid icon the app's
        // System.Drawing-based ICO load path can read back.
        string path = dir.File(fileName);
        using System.Drawing.Bitmap bitmap = new(16, 16);
        using (System.Drawing.Graphics graphics = System.Drawing.Graphics.FromImage(bitmap))
        {
            graphics.Clear(System.Drawing.Color.Red);
        }

        IntPtr hIcon = bitmap.GetHicon();
        using System.Drawing.Icon icon = System.Drawing.Icon.FromHandle(hIcon);
        using FileStream stream = System.IO.File.Create(path);
        icon.Save(stream);
        return path;
    }
}

/// <summary>Bridges Wrapper.Image's event-based async load to an awaitable Task.</summary>
// internal because the return type ViewerImage (ImageViewer.Wrapper.Image) is internal,
// reachable here only through InternalsVisibleTo("ImageViewer.Tests").
internal static class ImageLoader
{
    public static async Task<ViewerImage> LoadAsync(string path, int timeoutMs = 15000)
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
        image.Load(path);

        Task completed = await Task.WhenAny(tcs.Task, Task.Delay(timeoutMs));
        if (completed != tcs.Task)
        {
            throw new TimeoutException("Image load timed out: " + path);
        }

        await tcs.Task; // surface any load exception
        return image;
    }
}
