using System;
using System.Runtime.InteropServices;

using Microsoft.UI.Xaml;
using Microsoft.Win32;

namespace ImageViewer
{
    public static class ThemeHelpers
    {
        [DllImport("dwmapi.dll")]
        private static extern int DwmSetWindowAttribute(IntPtr hwnd, int attr, ref int attrValue, int attrSize);

        internal const string HKeyRoot = "HKEY_CURRENT_USER";
        internal const string HkeyWindowsTheme = @"SOFTWARE\Microsoft\Windows\CurrentVersion\Themes";
        internal const string HkeyWindowsPersonalizeTheme = $@"{HkeyWindowsTheme}\Personalize";
        internal const string HValueAppTheme = "AppsUseLightTheme";
        internal const int DWMWAImmersiveDarkMode = 20;

        public static ElementTheme GetAppTheme()
        {
            int value = (int)Registry.GetValue($"{HKeyRoot}\\{HkeyWindowsPersonalizeTheme}", HValueAppTheme, 1);
            return value == 1 ? ElementTheme.Light : ElementTheme.Dark;
        }

        public static void SetImmersiveDarkMode(IntPtr window, bool enabled)
        {
            int useImmersiveDarkMode = enabled ? 1 : 0;
            _ = DwmSetWindowAttribute(window, DWMWAImmersiveDarkMode, ref useImmersiveDarkMode, sizeof(int));
        }
    }
}
