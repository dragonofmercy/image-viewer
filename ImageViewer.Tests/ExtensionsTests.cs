using ImageViewer.Utilities;
using Xunit;

namespace ImageViewer.Tests;

public class ExtensionsTests
{
    [Fact]
    public void RemoveAtIndex_RemovesMiddleElement()
    {
        string[] result = new[] { "a", "b", "c" }.RemoveAtIndex(1);
        Assert.Equal(new[] { "a", "c" }, result);
    }

    [Fact]
    public void RemoveAtIndex_RemovesFirstElement()
    {
        string[] result = new[] { "a", "b", "c" }.RemoveAtIndex(0);
        Assert.Equal(new[] { "b", "c" }, result);
    }

    [Fact]
    public void RemoveAtIndex_RemovesLastElement()
    {
        string[] result = new[] { "a", "b", "c" }.RemoveAtIndex(2);
        Assert.Equal(new[] { "a", "b" }, result);
    }

    [Theory]
    [InlineData(-1)]
    [InlineData(3)]
    public void RemoveAtIndex_OutOfRange_ReturnsOriginalUnchanged(int index)
    {
        string[] original = new[] { "a", "b", "c" };
        string[] result = original.RemoveAtIndex(index);
        Assert.Equal(new[] { "a", "b", "c" }, result);
    }

    [Fact]
    public void UcFirst_CapitalizesFirstCharacter()
    {
        Assert.Equal("Hello", "hello".UcFirst());
    }
}
