using ImageViewer.Helpers;
using Xunit;

namespace ImageViewer.Tests;

public class SettingsTests
{
    [Theory]
    [InlineData("50", 100, 50)]
    [InlineData("100", 100, 100)]
    [InlineData("1", 100, 1)]
    [InlineData("0", 100, 1)]
    [InlineData("150", 100, 100)]
    [InlineData("-5", 100, 1)]
    [InlineData("abc", 100, 100)]
    public void ClampQuality_ParsesAndClampsStrings(string raw, int defaultValue, int expected)
    {
        Assert.Equal(expected, Settings.ClampQuality(raw, defaultValue));
    }

    [Fact]
    public void ClampQuality_NullReturnsDefault()
    {
        Assert.Equal(100, Settings.ClampQuality(null, 100));
        Assert.Equal(80, Settings.ClampQuality(null, 80));
    }

    [Fact]
    public void ClampQuality_AcceptsBoxedRegistryInt()
    {
        // Registry.GetValue returns a boxed Int32 for a REG_DWORD, not a string.
        Assert.Equal(75, Settings.ClampQuality(75, 100));
    }
}
