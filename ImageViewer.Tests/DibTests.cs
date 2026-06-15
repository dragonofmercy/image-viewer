using System;
using ImageViewer.Utilities;
using Xunit;

namespace ImageViewer.Tests;

public class DibTests
{
    [Fact]
    public void BuildDib_WritesBitmapInfoHeader()
    {
        // 2x2 image, top-down BGRA (4 bytes per pixel)
        const int width = 2;
        const int height = 2;
        byte[] topDown = new byte[width * height * 4];

        byte[] dib = ClipboardHelper.BuildDib(topDown, width, height);

        Assert.Equal(40, BitConverter.ToInt32(dib, 0));                 // biSize
        Assert.Equal(width, BitConverter.ToInt32(dib, 4));             // biWidth
        Assert.Equal(height, BitConverter.ToInt32(dib, 8));           // biHeight (positive => bottom-up)
        Assert.Equal(1, BitConverter.ToUInt16(dib, 12));             // biPlanes
        Assert.Equal(32, BitConverter.ToUInt16(dib, 14));          // biBitCount
        Assert.Equal(0, BitConverter.ToInt32(dib, 16));           // biCompression = BI_RGB
        Assert.Equal(width * 4 * height, BitConverter.ToInt32(dib, 20)); // biSizeImage
        Assert.Equal(40 + width * 4 * height, dib.Length);
    }

    [Fact]
    public void BuildDib_FlipsRowsBottomUp()
    {
        // 1px wide, 2px tall so each row is exactly one BGRA pixel (stride = 4).
        const int width = 1;
        const int height = 2;
        byte[] topDown =
        {
            10, 11, 12, 13, // row 0 (visual top)
            20, 21, 22, 23  // row 1 (visual bottom)
        };

        byte[] dib = ClipboardHelper.BuildDib(topDown, width, height);

        // In a bottom-up DIB the first stored row is the visual bottom (top-down row 1).
        Assert.Equal(new byte[] { 20, 21, 22, 23 }, new[] { dib[40], dib[41], dib[42], dib[43] });
        Assert.Equal(new byte[] { 10, 11, 12, 13 }, new[] { dib[44], dib[45], dib[46], dib[47] });
    }
}
