using ImageViewer.Helpers;
using Xunit;

namespace ImageViewer.Tests;

public class SaveServiceTests
{
    [Theory]
    [InlineData(".jpg", ".jpg")]
    [InlineData(".JPG", ".jpg")]
    [InlineData(".jpeg", ".jpg")]
    [InlineData(".JPEG", ".jpg")]
    [InlineData(".tif", ".tiff")]
    [InlineData(".TIF", ".tiff")]
    [InlineData(".tiff", ".tiff")]
    [InlineData(".png", ".png")]
    [InlineData(".PNG", ".png")]
    [InlineData(".webp", ".webp")]
    public void NormalizeExtension_LowercasesAndCanonicalizes(string ext, string expected)
    {
        Assert.Equal(expected, SaveService.NormalizeExtension(ext));
    }
}
