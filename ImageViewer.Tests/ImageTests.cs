using System.Threading.Tasks;

using SixLabors.ImageSharp.Processing;

using Xunit;

using ViewerImage = ImageViewer.Wrapper.Image;

namespace ImageViewer.Tests;

public class ImageTests
{
    [Theory]
    [InlineData("sample.png", 4, 2)]
    [InlineData("sample.jpg", 4, 2)]
    [InlineData("sample.bmp", 4, 2)]
    [InlineData("sample.gif", 4, 2)]
    [InlineData("sample.tiff", 4, 2)]
    [InlineData("sample.webp", 4, 2)]
    [InlineData("sample.tga", 4, 2)]
    public async Task Load_NativeFormat_ReportsDimensions(string fileName, int width, int height)
    {
        using TempDir dir = new();
        string path = FixtureFactory.Save(dir, fileName, width, height);

        ViewerImage image = await ImageLoader.LoadAsync(path);
        try
        {
            Assert.True(image.Loaded);
            Assert.Equal(width, (int)image.Width);
            Assert.Equal(height, (int)image.Height);
        }
        finally
        {
            image.Dispose();
        }
    }

    [Fact]
    public async Task Load_Svg_Succeeds()
    {
        using TempDir dir = new();
        string path = FixtureFactory.SaveSvg(dir, "sample.svg", 32, 32);

        ViewerImage image = await ImageLoader.LoadAsync(path);
        try
        {
            Assert.True(image.Loaded);
            Assert.True(image.Width > 0 && image.Height > 0);
        }
        finally
        {
            image.Dispose();
        }
    }

    [Fact]
    public async Task Load_Ico_Succeeds()
    {
        using TempDir dir = new();
        string path = FixtureFactory.SaveIco(dir, "sample.ico");

        ViewerImage image = await ImageLoader.LoadAsync(path);
        try
        {
            Assert.True(image.Loaded);
            Assert.True(image.Width > 0 && image.Height > 0);
        }
        finally
        {
            image.Dispose();
        }
    }

    [Fact]
    public async Task Load_AppliesExifOrientation_SwappingWidthAndHeight()
    {
        using TempDir dir = new();
        // 4x2 with EXIF orientation 6 (rotate 90 CW) => AutoOrient yields 2x4.
        string path = FixtureFactory.SaveJpegOrientation6(dir, "exif.jpg", 4, 2);

        ViewerImage image = await ImageLoader.LoadAsync(path);
        try
        {
            Assert.Equal(2, (int)image.Width);
            Assert.Equal(4, (int)image.Height);
        }
        finally
        {
            image.Dispose();
        }
    }

    [Fact]
    public async Task GetBgra32Pixels_ReturnsExpectedSizeAndDimensions()
    {
        using TempDir dir = new();
        string path = FixtureFactory.Save(dir, "pixels.png", 4, 2);

        ViewerImage image = await ImageLoader.LoadAsync(path);
        try
        {
            byte[] pixels = image.GetBgra32Pixels(out int width, out int height);
            Assert.Equal(4, width);
            Assert.Equal(2, height);
            Assert.Equal(4 * 2 * 4, pixels.Length);
        }
        finally
        {
            image.Dispose();
        }
    }

    [Fact]
    public async Task RotateFlip_Rotate90_SwapsDimensions()
    {
        using TempDir dir = new();
        string path = FixtureFactory.Save(dir, "rotate.png", 4, 2);

        ViewerImage image = await ImageLoader.LoadAsync(path);
        try
        {
            image.RotateFlip(RotateMode.Rotate90, FlipMode.None);
            Assert.Equal(2, (int)image.Width);
            Assert.Equal(4, (int)image.Height);
        }
        finally
        {
            image.Dispose();
        }
    }

    [Fact]
    public async Task Modified_FalseAfterLoad_TrueAfterTransform_FalseAfterSave()
    {
        using TempDir dir = new();
        string path = FixtureFactory.Save(dir, "modflag.png", 4, 2);

        ViewerImage image = await ImageLoader.LoadAsync(path);
        try
        {
            Assert.False(image.Modified);

            image.RotateFlip(RotateMode.Rotate90, FlipMode.None);
            Assert.True(image.Modified);

            await image.Save(dir.File("modflag-out.png"), ".png");
            Assert.False(image.Modified);
        }
        finally
        {
            image.Dispose();
        }
    }
}
