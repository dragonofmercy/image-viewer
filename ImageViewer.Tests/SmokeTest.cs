using ImageViewer.Utilities;
using Xunit;

namespace ImageViewer.Tests;

public class SmokeTest
{
    [Fact]
    public void TestHost_CanCallInternalExtension()
    {
        // Proves the host loads the app assembly and InternalsVisibleTo works,
        // without ever touching a WinUI type.
        Assert.Equal("Abc", "abc".UcFirst());
    }
}
