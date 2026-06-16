using System;
using System.Diagnostics;

using WinRT.Interop;

using ImageViewer.Utilities;
using ImageViewer.Wrapper;

namespace ImageViewer.Services;

/// <summary>
/// Copies the working image to the Windows clipboard as a CF_DIB bitmap (symmetric with paste).
/// Holds only the owning window for the clipboard owner handle.
/// </summary>
internal sealed class ClipboardService
{
    private readonly MainWindow Window;

    internal ClipboardService(MainWindow window)
    {
        Window = window;
    }

    /// <summary>
    /// Copy the image to the clipboard as a CF_DIB bitmap. Failures are swallowed (logged).
    /// </summary>
    internal void CopyImage(Image image)
    {
        try
        {
            byte[] pixels = image.GetBgra32Pixels(out int width, out int height);
            ClipboardHelper.SetImageAsDib(WindowNative.GetWindowHandle(Window), pixels, width, height);
        }
        catch (Exception ex)
        {
            Debug.WriteLine($"Copy to clipboard failed: {ex.Message}");
        }
    }
}
