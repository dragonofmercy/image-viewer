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

        internal const string   HKey = "HKEY_CURRENT_USER\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize";
        internal const string   HValueAppTheme = "AppsUseLightTheme";
        internal const int      DwmwaImmersiveDarkMode = 20;

        public static ElementTheme GetAppTheme()
        {
            object registryValue = Registry.GetValue(HKey, HValueAppTheme, 1);
            int value = registryValue == null ? 1 : (int)registryValue;
            return value == 1 ? ElementTheme.Light : ElementTheme.Dark;
        }

        public static void SetImmersiveDarkMode(IntPtr window, bool enabled)
        {
            int useImmersiveDarkMode = enabled ? 1 : 0;
            _ = DwmSetWindowAttribute(window, DwmwaImmersiveDarkMode, ref useImmersiveDarkMode, sizeof(int));
        }
    }
}
