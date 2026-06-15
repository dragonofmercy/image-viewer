using System.Collections.Generic;
using System.Linq;
using ImageViewer.Helpers;
using ImageViewer.Strings;
using Xunit;

namespace ImageViewer.Tests;

public class CultureTests
{
    [Fact]
    public void En_And_Fr_HaveIdenticalKeySets()
    {
        Dictionary<string, string> en = En.GetStrings();
        Dictionary<string, string> fr = Fr.GetStrings();

        string[] missingInFr = en.Keys.Except(fr.Keys).OrderBy(k => k).ToArray();
        string[] missingInEn = fr.Keys.Except(en.Keys).OrderBy(k => k).ToArray();

        Assert.True(missingInFr.Length == 0, "Keys present in En but missing in Fr: " + string.Join(", ", missingInFr));
        Assert.True(missingInEn.Length == 0, "Keys present in Fr but missing in En: " + string.Join(", ", missingInEn));
    }

    [Fact]
    public void GetString_UnknownKey_ReturnsBracketedKey()
    {
        Assert.Equal("[__DEFINITELY_NOT_A_KEY__]", Culture.GetString("__DEFINITELY_NOT_A_KEY__"));
    }
}
