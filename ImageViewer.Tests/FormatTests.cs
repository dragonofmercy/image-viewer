using ImageViewer.Utilities;
using Xunit;

namespace ImageViewer.Tests;

public class FormatTests
{
    // Whole-number sizes only: the fractional path uses CurrentCulture (e.g. "1,5" in fr-FR),
    // so it is not asserted here to keep the test culture-independent.
    [Theory]
    [InlineData(0, "0 B")]
    [InlineData(1, "1 B")]
    [InlineData(1023, "1023 B")]
    [InlineData(1024, "1 KB")]
    [InlineData(1048576, "1 MB")]
    [InlineData(1073741824, "1 GB")]
    [InlineData(1099511627776, "1 TB")]
    [InlineData(1125899906842624, "1024 TB")] // caps at TB, no PB unit
    public void HumanizeBytes_StepsAndCaps(double bytes, string expected)
    {
        Assert.Equal(expected, Format.HumanizeBytes(bytes));
    }

    [Theory]
    [InlineData(0, 0)]
    [InlineData(5, 0)]
    [InlineData(12, 10)]
    [InlineData(15, 10)]
    [InlineData(19, 10)]
    [InlineData(25, 20)]
    [InlineData(99, 90)]
    [InlineData(100, 100)]
    public void RoundToTen_FloorsToMultipleOfTen(double input, float expected)
    {
        Assert.Equal(expected, Format.RoundToTen(input));
    }
}
