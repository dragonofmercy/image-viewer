using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Xml.Linq;

using Xunit;

namespace ImageViewer.Tests;

public class CultureTests
{
    private static string StringsDir()
    {
        DirectoryInfo dir = new(AppContext.BaseDirectory);
        while (dir != null)
        {
            string candidate = Path.Combine(dir.FullName, "ImageViewer", "Strings");
            if (Directory.Exists(candidate)) return candidate;
            dir = dir.Parent;
        }

        throw new DirectoryNotFoundException("Could not locate ImageViewer/Strings from " + AppContext.BaseDirectory);
    }

    private static HashSet<string> KeysOf(string languageTag)
    {
        string path = Path.Combine(StringsDir(), languageTag, "Resources.resw");
        XDocument doc = XDocument.Load(path);

        return doc.Root.Elements("data")
            .Select(e => (string)e.Attribute("name"))
            .Where(n => n != null)
            .ToHashSet();
    }

    // Every language folder under Strings that has a Resources.resw, except the en-US baseline.
    public static IEnumerable<object[]> NonDefaultLanguages()
    {
        string root = StringsDir();
        return Directory.GetDirectories(root)
            .Where(d => File.Exists(Path.Combine(d, "Resources.resw")))
            .Select(Path.GetFileName)
            .Where(name => !string.Equals(name, "en-US", StringComparison.OrdinalIgnoreCase))
            .Select(name => new object[] { name });
    }

    [Theory]
    [MemberData(nameof(NonDefaultLanguages))]
    public void Language_HasSameKeysAsEnglish(string languageTag)
    {
        HashSet<string> en = KeysOf("en-US");
        HashSet<string> other = KeysOf(languageTag);

        string[] missing = en.Except(other).OrderBy(k => k).ToArray();
        string[] extra = other.Except(en).OrderBy(k => k).ToArray();

        Assert.True(missing.Length == 0, languageTag + " is missing keys: " + string.Join(", ", missing));
        Assert.True(extra.Length == 0, languageTag + " has extra keys: " + string.Join(", ", extra));
    }
}
