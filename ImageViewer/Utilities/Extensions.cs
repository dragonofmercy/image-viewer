using System;
using System.Collections.Generic;
using System.Globalization;
using System.Runtime.InteropServices;
using System.Security;
using ImageViewer.Helpers;

namespace ImageViewer.Utilities;

public static class Extensions
{
    public static T[] RemoveAtIndex<T>(this T[] original, int index)
    {
        if (index < 0 || index >= original.Length) return original;
        return [..original[..index], ..original[(index + 1)..]];
    }

    public static string UcFirst(this string original)
    {
        return char.ToUpper(original[0]) + original[1..];
    }

    public static string ToUpdateDate(this string original)
    {
        // A corrupted registry value must not crash the About dialog: treat it as "never checked"
        return DateTime.TryParseExact(original, Settings.UPDATE_DATE_FORMAT, CultureInfo.InvariantCulture, DateTimeStyles.None, out DateTime parsed)
            ? parsed.ToString(CultureInfo.CurrentCulture)
            : Culture.GetString("ABOUT_LABEL_LAST_UPDATE_NEVER");
    }
}

[SuppressUnmanagedCodeSecurity]
internal static class SafeNativeMethods
{
    [DllImport("shlwapi.dll", CharSet = CharSet.Unicode)]
    public static extern int StrCmpLogicalW(string psz1, string psz2);
}

public sealed class NaturalStringComparer : IComparer<string>
{
    public int Compare(string a, string b)
    {
        return SafeNativeMethods.StrCmpLogicalW(a, b);
    }
}