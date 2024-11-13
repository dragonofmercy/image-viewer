using System;
using System.Runtime.InteropServices;

using Microsoft.UI.Xaml;
using Microsoft.Win32;

namespace ImageViewer.Helpers;

public static class Theme
{
    [DllImport("dwmapi.dll")]
    private static extern int DwmSetWindowAttribute(IntPtr hwnd, int attr, ref int attrValue, int attrSize);

    internal const string H_KEY = @"HKEY_CURRENT_USER\SOFTWARE\Microsoft\Windows\CurrentVersion\Themes\Personalize";
    internal const string H_VALUE_APP_THEME = "AppsUseLightTheme";
    internal const int DWMWA_IMMERSIVE_DARK_MODE = 20;

    public static ElementTheme GetSystemTheme()
    {
        object registryValue = Registry.GetValue(H_KEY, H_VALUE_APP_THEME, 1);
        int value = registryValue == null ? 1 : (int)registryValue;
        return value == 1 ? ElementTheme.Light : ElementTheme.Dark;
    }

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