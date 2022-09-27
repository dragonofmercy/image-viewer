using System;
using System.Linq;
using System.Drawing;
using System.Drawing.Imaging;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Security;

namespace ImageViewer
{
    public static class Extensions
    {
        public static T[] RemoveFromArray<T>(this T[] original, T itemToRemove)
        {
            int numIdx = System.Array.IndexOf(original, itemToRemove);
            if(numIdx == -1) return original;
            List<T> tmp = new(original);
            tmp.RemoveAt(numIdx);
            return tmp.ToArray();
        }

        public static T[] RemoveAtIndex<T>(this T[] original, int index)
        {
            if(index >= original.Length) return original;
            List<T> tmp = new(original);
            tmp.RemoveAt(index);
            return tmp.ToArray();
        }

        public static string UcFirst(this string original)
        {
            return char.ToUpper(original[0]) + original[1..];
        }

        public static string ToUpdateDate(this string original)
        {
            if(string.IsNullOrEmpty(original))
            {
                return Culture.GetString("ABOUT_LABEL_LAST_UPDATE_NEVER");
            }
            else
            {
                return DateTime.Parse(original).ToString();
            }
        }

        public static void SaveJpeg(this Bitmap original, string filepath, byte quality)
        {
            ImageCodecInfo encoder = ImageCodecInfo.GetImageEncoders().Where(s => s.FormatID == ImageFormat.Jpeg.Guid).First();
            EncoderParameters encoderParameters = new(1);
            encoderParameters.Param[0] = new EncoderParameter(Encoder.Quality, Convert.ToInt64(quality));
            original.Save(filepath, encoder, encoderParameters);
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
}
