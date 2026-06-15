using System;
using ImageViewer.Utilities;
using Xunit;

namespace ImageViewer.Tests;

public class NaturalStringComparerTests
{
    [Fact]
    public void Compare_OrdersImg2BeforeImg10()
    {
        NaturalStringComparer comparer = new();
        Assert.True(comparer.Compare("img2", "img10") < 0);
    }

    [Fact]
    public void Sort_ProducesExplorerStyleNaturalOrder()
    {
        NaturalStringComparer comparer = new();
        string[] files = { "img10.jpg", "img2.jpg", "img1.jpg" };
        Array.Sort(files, comparer);
        Assert.Equal(new[] { "img1.jpg", "img2.jpg", "img10.jpg" }, files);
    }
}
