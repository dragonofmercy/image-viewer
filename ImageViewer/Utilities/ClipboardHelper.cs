using System;
using System.IO;
using System.Runtime.InteropServices;

namespace ImageViewer.Utilities;

/// <summary>
/// Win32 clipboard interop. WinRT's DataPackage.SetBitmap publishes a stream format that classic
/// Win32 apps (Paint, Office) do not paste; they expect CF_DIB. This writes a real device-independent
/// bitmap so the image pastes everywhere.
/// </summary>
internal static class ClipboardHelper
{
    private const uint CF_DIB = 8;
    private const uint GMEM_MOVEABLE = 0x0002;

    [DllImport("user32.dll", SetLastError = true)] private static extern bool OpenClipboard(IntPtr hWndNewOwner);
    [DllImport("user32.dll", SetLastError = true)] private static extern bool EmptyClipboard();
    [DllImport("user32.dll", SetLastError = true)] private static extern IntPtr SetClipboardData(uint uFormat, IntPtr hMem);
    [DllImport("user32.dll", SetLastError = true)] private static extern bool CloseClipboard();

    [DllImport("kernel32.dll")] private static extern IntPtr GlobalAlloc(uint uFlags, UIntPtr dwBytes);
    [DllImport("kernel32.dll")] private static extern IntPtr GlobalLock(IntPtr hMem);
    [DllImport("kernel32.dll")] private static extern bool GlobalUnlock(IntPtr hMem);
    [DllImport("kernel32.dll")] private static extern IntPtr GlobalFree(IntPtr hMem);

    /// <summary>
    /// Put a 32bpp image on the clipboard as CF_DIB. <paramref name="topDownBgra"/> is top-down BGRA
    /// (the order ImageSharp's Bgra32 produces); it is flipped to the bottom-up layout a DIB requires.
    /// </summary>
    public static bool SetImageAsDib(IntPtr hwnd, byte[] topDownBgra, int width, int height)
    {
        byte[] dib = BuildDib(topDownBgra, width, height);

        if (!OpenClipboard(hwnd)) return false;

        try
        {
            EmptyClipboard();

            IntPtr hMem = GlobalAlloc(GMEM_MOVEABLE, (UIntPtr)dib.Length);
            if (hMem == IntPtr.Zero) return false;

            IntPtr target = GlobalLock(hMem);
            if (target == IntPtr.Zero) { GlobalFree(hMem); return false; }

            try { Marshal.Copy(dib, 0, target, dib.Length); }
            finally { GlobalUnlock(hMem); }

            // On success the clipboard owns hMem and must not be freed; on failure we reclaim it.
            if (SetClipboardData(CF_DIB, hMem) == IntPtr.Zero)
            {
                GlobalFree(hMem);
                return false;
            }

            return true;
        }
        finally
        {
            CloseClipboard();
        }
    }

    private static byte[] BuildDib(byte[] topDownBgra, int width, int height)
    {
        int stride = width * 4;
        int imageSize = stride * height;

        using MemoryStream ms = new(40 + imageSize);
        using BinaryWriter w = new(ms);

        // BITMAPINFOHEADER
        w.Write(40);              // biSize
        w.Write(width);           // biWidth
        w.Write(height);          // biHeight (positive => bottom-up)
        w.Write((ushort)1);       // biPlanes
        w.Write((ushort)32);      // biBitCount
        w.Write(0);               // biCompression = BI_RGB
        w.Write(imageSize);       // biSizeImage
        w.Write(0);               // biXPelsPerMeter
        w.Write(0);               // biYPelsPerMeter
        w.Write(0);               // biClrUsed
        w.Write(0);               // biClrImportant

        // Pixel data, bottom-up: write source rows from last to first
        for (int y = height - 1; y >= 0; y--)
        {
            w.Write(topDownBgra, y * stride, stride);
        }

        w.Flush();
        return ms.ToArray();
    }
}
