using System;
using System.Collections.Generic;
using System.Globalization;
using System.Runtime.InteropServices;
using System.Security;
using Svg;
using ImageViewer.Helpers;

// NOTE: System.Drawing is required here because the Svg library (3.4.7) depends on it
// SvgDocument.GetDimensions() returns System.Drawing.SizeF
// To eliminate this dependency, replace "Svg" package with "Svg.Skia" (see migration guide)
using System.Drawing;

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
        // GetDimensions() returns System.Drawing.SizeF from Svg library
        System.Drawing.SizeF svgSize = original.GetDimensions();

        float width = svgSize.Width;
        float height = svgSize.Height;

        if (width > maxWidth)
        {
            float ratio = width / maxWidth;
            height /= ratio;
            width = maxWidth;
        }

        if (height > maxHeight)
        {
            float ratio = height / maxHeight;
            width /= ratio;
            height = maxHeight;
        }

        original.Width = width;
        original.Height = height;

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