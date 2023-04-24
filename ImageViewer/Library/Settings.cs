using Microsoft.UI.Xaml;
using Microsoft.Win32;

namespace ImageViewer
{
    internal enum WindowState
    {
        Normal,
        Maximized,
        Minimized
    }

    internal class Settings
    {
        internal const string HKEY_APP_PATH = @"HKEY_CURRENT_USER\SOFTWARE\Dragon Industries\Image Viewer";
        internal const string H_VALUE_APP_THEME = "Theme";
        internal const string H_VALUE_APP_POSITION_X = "PosX";
        internal const string H_VALUE_APP_POSITION_Y = "PosY";
        internal const string H_VALUE_APP_SIZE_W = "Width";
        internal const string H_VALUE_APP_SIZE_H = "Height";
        internal const string H_VALUE_APP_STATE = "State";
        internal const string H_VALUE_LAST_UPDATE_CHECK = "LastUpdateCheck";
        internal const string H_VALUE_LANGUAGE = "Lang";
        internal const string H_VALUE_CHECK_UPDATE_INTERVAL = "UpdateInterval";

        public static ElementTheme Theme 
        {
            get
            {
                object tmp = Registry.GetValue(HKEY_APP_PATH, H_VALUE_APP_THEME, ElementTheme.Default);
                return tmp != null ? (ElementTheme)tmp : ElementTheme.Default;
            }

            set
            {
                Registry.SetValue(HKEY_APP_PATH, H_VALUE_APP_THEME, (int)value);
            }
        }

        public static int? AppPositionX
        {
            get
            {
                object tmp = Registry.GetValue(HKEY_APP_PATH, H_VALUE_APP_POSITION_X, null);
                return tmp != null ? int.Parse(tmp.ToString()) : null;
            }

            set
            {
                Registry.SetValue(HKEY_APP_PATH, H_VALUE_APP_POSITION_X, value.ToString());
            }
        }

        public static int? AppPositionY
        {
            get
            {
                object tmp = Registry.GetValue(HKEY_APP_PATH, H_VALUE_APP_POSITION_Y, null);
                return tmp != null ? int.Parse(tmp.ToString()) : null;
            }

            set
            {
                Registry.SetValue(HKEY_APP_PATH, H_VALUE_APP_POSITION_Y, value.ToString());
            }
        }

        public static uint AppSizeW
        {
            get
            {
                object tmp = Registry.GetValue(HKEY_APP_PATH, H_VALUE_APP_SIZE_W, null);
                return tmp != null ? uint.Parse(tmp.ToString()) : 1280;
            }

            set
            {
                Registry.SetValue(HKEY_APP_PATH, H_VALUE_APP_SIZE_W, value.ToString());
            }
        }

        public static uint AppSizeH
        {
            get
            {
                object tmp = Registry.GetValue(HKEY_APP_PATH, H_VALUE_APP_SIZE_H, null);
                return tmp != null ? uint.Parse(tmp.ToString()) : 768;
            }

            set
            {
                Registry.SetValue(HKEY_APP_PATH, H_VALUE_APP_SIZE_H, value.ToString());
            }
        }

        public static WindowState WindowState
        {
            get
            {
                return (WindowState)Registry.GetValue(HKEY_APP_PATH, H_VALUE_APP_STATE, WindowState.Normal);
            }

            set
            {
                Registry.SetValue(HKEY_APP_PATH, H_VALUE_APP_STATE, (int)value);
            }
        }

        public static string LastUpdateCheck
        {
            get
            {
                return (string)Registry.GetValue(HKEY_APP_PATH, H_VALUE_LAST_UPDATE_CHECK, "");
            }

            set
            {
                Registry.SetValue(HKEY_APP_PATH, H_VALUE_LAST_UPDATE_CHECK, value);
            }
        }

        public static string UpdateInterval
        {
            get
            {
                return (string)Registry.GetValue(HKEY_APP_PATH, H_VALUE_CHECK_UPDATE_INTERVAL, "");
            }

            set
            {
                Registry.SetValue(HKEY_APP_PATH, H_VALUE_CHECK_UPDATE_INTERVAL, value);
            }
        }

        public static string Language
        {
            get
            {
                return (string)Registry.GetValue(HKEY_APP_PATH, H_VALUE_LANGUAGE, "");
            }

            set
            {
                Registry.SetValue(HKEY_APP_PATH, H_VALUE_LANGUAGE, value);
            }
        }
    }
}
