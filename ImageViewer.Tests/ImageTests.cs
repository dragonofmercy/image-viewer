using System.IO;
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

    [Theory]
    // Below the 1024 raster cap: authored size is kept, never upscaled.
    [InlineData(32, 32, 32, 32)]
    [InlineData(800, 600, 800, 600)]
    // Above the cap: scaled down on the long edge, aspect ratio preserved.
    [InlineData(4096, 2048, 1024, 512)]
    [InlineData(1000, 4000, 256, 1024)]
    public async Task Load_Svg_RastersWithinTheSizeCap(int svgWidth, int svgHeight, int expectedWidth, int expectedHeight)
    {
        using TempDir dir = new();
        string path = FixtureFactory.SaveSvg(dir, "sample.svg", svgWidth, svgHeight);

        ViewerImage image = await ImageLoader.LoadAsync(path);
        try
        {
            Assert.True(image.Loaded);
            Assert.Equal(expectedWidth, (int)image.Width);
            Assert.Equal(expectedHeight, (int)image.Height);
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
        string path = FixtureFactory.SaveIco(dir, "sample.ico", 32);

        ViewerImage image = await ImageLoader.LoadAsync(path);
        try
        {
            Assert.True(image.Loaded);
            Assert.Equal(32, (int)image.Width);
            Assert.Equal(32, (int)image.Height);
        }
        finally
        {
            image.Dispose();
        }
    }

    [Fact]
    public async Task Load_Ico_PicksLargestFrame_NotTheFirstOne()
    {
        using TempDir dir = new();
        // Smallest entry written first: taking frame 0 would show a 16x16 icon instead of the 256x256 one.
        string path = FixtureFactory.SaveIco(dir, "multi.ico", 16, 256, 48);

        ViewerImage image = await ImageLoader.LoadAsync(path);
        try
        {
            Assert.Equal(256, (int)image.Width);
            Assert.Equal(256, (int)image.Height);
        }
        finally
        {
            image.Dispose();
        }
    }

    [Fact]
    public async Task Load_Ico_ExposesEverySizeLargestFirst()
    {
        using TempDir dir = new();
        string path = FixtureFactory.SaveIco(dir, "sizes.ico", 16, 256, 48);

        ViewerImage image = await ImageLoader.LoadAsync(path);
        try
        {
            Assert.True(image.HasIconSizes);
            Assert.Equal(3, image.IconSizeCount);
            Assert.Equal(0, image.IconSizeIndex);

            // File order is 16/256/48: the strip must list them 256/48/16, whatever the container says.
            Assert.Equal((256, 256), image.GetIconSize(0));
            Assert.Equal((48, 48), image.GetIconSize(1));
            Assert.Equal((16, 16), image.GetIconSize(2));
        }
        finally
        {
            image.Dispose();
        }
    }

    [Fact]
    public async Task SelectIconSize_SwapsTheDisplayedFrame()
    {
        using TempDir dir = new();
        string path = FixtureFactory.SaveIco(dir, "select.ico", 16, 256, 48);

        ViewerImage image = await ImageLoader.LoadAsync(path);
        try
        {
            image.SelectIconSize(2);

            Assert.Equal(2, image.IconSizeIndex);
            Assert.Equal(16, (int)image.Width);
            Assert.Equal(16, (int)image.Height);

            image.SelectIconSize(0);

            Assert.Equal(256, (int)image.Width);
            Assert.Equal(256, (int)image.Height);
        }
        finally
        {
            image.Dispose();
        }
    }

    [Fact]
    public async Task SelectIconSize_KeepsTransformsOnTheFrameTheyWereAppliedTo()
    {
        using TempDir dir = new();
        string path = FixtureFactory.SaveIco(dir, "transform.ico", 256, 16);

        ViewerImage image = await ImageLoader.LoadAsync(path);
        try
        {
            image.Crop(0, 0, 100, 40);
            Assert.Equal(100, (int)image.Width);

            image.SelectIconSize(1);
            Assert.Equal(16, (int)image.Width);

            // Back to the cropped one: it stayed cropped, it was not silently reloaded.
            image.SelectIconSize(0);
            Assert.Equal(100, (int)image.Width);
            Assert.Equal(40, (int)image.Height);
        }
        finally
        {
            image.Dispose();
        }
    }

    [Theory]
    [InlineData("single.ico")]
    [InlineData("single.png")]
    public async Task HasIconSizes_FalseWithoutSizeVariants(string fileName)
    {
        using TempDir dir = new();
        string path = fileName.EndsWith(".ico") ? FixtureFactory.SaveIco(dir, fileName, 32) : FixtureFactory.Save(dir, fileName, 32, 32);

        ViewerImage image = await ImageLoader.LoadAsync(path);
        try
        {
            Assert.False(image.HasIconSizes);
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

    [Fact]
    public async Task Modified_TrueAfterMemoryLoad()
    {
        // A memory/clipboard-loaded image has no backing file: it must start modified so the
        // title shows the unsaved indicator and Save routes to Save As.
        using TempDir dir = new();
        string path = FixtureFactory.Save(dir, "mem.png", 4, 2);

        using MemoryStream stream = new(File.ReadAllBytes(path));
        ViewerImage image = await ImageLoader.LoadAsync(stream.AsInputStream());
        try
        {
            Assert.True(image.Modified);
        }
        finally
        {
            image.Dispose();
        }
    }

    [Theory]
    [InlineData(".jpg")]
    [InlineData(".webp")]
    public async Task Save_LowerQuality_ProducesSmallerFile(string type)
    {
        using TempDir dir = new();
        string path = FixtureFactory.SaveNoisy(dir, "src.png", 256, 256);

        ViewerImage image = await ImageLoader.LoadAsync(path);
        try
        {
            string high = dir.File("high" + type);
            string low = dir.File("low" + type);

            await image.Save(high, type, 95);
            await image.Save(low, type, 20);

            long highSize = new System.IO.FileInfo(high).Length;
            long lowSize = new System.IO.FileInfo(low).Length;

            Assert.True(lowSize < highSize, $"expected q20 ({lowSize}) < q95 ({highSize})");
        }
        finally
        {
            image.Dispose();
        }
    }
}
