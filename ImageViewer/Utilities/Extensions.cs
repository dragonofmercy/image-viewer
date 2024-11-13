using System;
using System.Drawing;
using System.Collections.Generic;
using System.Globalization;
using System.Runtime.InteropServices;
using System.Security;
using Svg;
using ImageViewer.Helpers;

namespace ImageViewer.Utilities;

public static class Extensions
{
    public static T[] RemoveAtIndex<T>(this T[] original, int index)
    {
        if (index >= original.Length) return original;
        List<T> tmp = [..original];
        tmp.RemoveAt(index);
        return tmp.ToArray();
    }

    public static SvgDocument AdjustSize(this SvgDocument original, uint maxWidth, uint maxHeight)
    {
        SizeF size = original.GetDimensions();

        if (size.Width > maxWidth)
        {
            float ratio = size.Width / maxWidth;
            size.Height /= ratio;
            size.Width = maxWidth;
        }

        if (size.Height > maxHeight)
        {
            float ratio = size.Height / maxHeight;
            size.Width /= ratio;
            size.Height = maxHeight;
        }

        original.Width = size.Width;
        original.Height = size.Height;

        return original;
    }

    public static string UcFirst(this string original)
    {
        return char.ToUpper(original[0]) + original[1..];
    }

    public static string ToUpdateDate(this string original)
    {
        return string.IsNullOrEmpty(original) ? Culture.GetString("ABOUT_LABEL_LAST_UPDATE_NEVER") : DateTime.Parse(original).ToString(CultureInfo.CurrentCulture);
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