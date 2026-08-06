using System;
using System.Runtime.InteropServices;

using Microsoft.UI.Xaml;

namespace ImageViewer.Helpers;

public static class Theme
{
    [DllImport("dwmapi.dll")]
    private static extern int DwmSetWindowAttribute(IntPtr hwnd, int attr, ref int attrValue, int attrSize);

    internal const int DWMWA_IMMERSIVE_DARK_MODE = 20;

    public static void SetImmersiveDarkMode(IntPtr window, bool enabled)
    {
        int useImmersiveDarkMode = enabled ? 1 : 0;
        _ = DwmSetWindowAttribute(window, DWMWA_IMMERSIVE_DARK_MODE, ref useImmersiveDarkMode, sizeof(int));
    }

    public static ResourceDictionary GetThemeResourceDictionary(string mode)
    {
        foreach(ResourceDictionary res in Application.Current.Resources.MergedDictionaries)
        {
            if(res.Source != null && res.Source.AbsolutePath.Contains("/Themes/Colors.xaml"))
            {
                return (ResourceDictionary)res.ThemeDictionaries[mode];
            }
        }

        throw new ApplicationException("Cannot load ImageViewer themes");
    }
}