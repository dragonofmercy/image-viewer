using Microsoft.UI.Xaml;
using Microsoft.Win32;

namespace ImageViewer
{
    enum WindowState
    {
        Normal,
        Maximized,
        Minimized
    }

    internal class Settings
    {
        internal const string HkeyAppPath = @"HKEY_CURRENT_USER\SOFTWARE\Dragon Industries\Image Viewer";
        internal const string HValueAppTheme = "Theme";
        internal const string HValueAppPositionX = "PosX";
        internal const string HValueAppPositionY = "PosY";
        internal const string HValueAppSizeW = "Width";
        internal const string HValueAppSizeH = "Height";
        internal const string HValueAppState = "State";
        internal const string HValueLastUpdateCheck = "LastUpdateCheck";
        internal const string HValueLanguage = "Lang";
        internal const string HValueCheckUpdateInterval = "UpdateInterval";

        public static ElementTheme Theme 
        {
            get
            {
                object tmp = Registry.GetValue(HkeyAppPath, HValueAppTheme, ElementTheme.Default);
                return tmp != null ? (ElementTheme)tmp : ElementTheme.Default;
            }

            set
            {
                Registry.SetValue(HkeyAppPath, HValueAppTheme, (int)value);
            }
        }

        public static int? AppPositionX
        {
            get
            {
                object tmp = Registry.GetValue(HkeyAppPath, HValueAppPositionX, null);
                return tmp != null ? int.Parse(tmp.ToString()) : null;
            }

            set
            {
                Registry.SetValue(HkeyAppPath, HValueAppPositionX, value.ToString());
            }
        }

        public static int? AppPositionY
        {
            get
            {
                object tmp = Registry.GetValue(HkeyAppPath, HValueAppPositionY, null);
                return tmp != null ? int.Parse(tmp.ToString()) : null;
            }

            set
            {
                Registry.SetValue(HkeyAppPath, HValueAppPositionY, value.ToString());
            }
        }

        public static uint AppSizeW
        {
            get
            {
                object tmp = Registry.GetValue(HkeyAppPath, HValueAppSizeW, null);
                return tmp != null ? uint.Parse(tmp.ToString()) : 1280;
            }

            set
            {
                Registry.SetValue(HkeyAppPath, HValueAppSizeW, value.ToString());
            }
        }

        public static uint AppSizeH
        {
            get
            {
                object tmp = Registry.GetValue(HkeyAppPath, HValueAppSizeH, null);
                return tmp != null ? uint.Parse(tmp.ToString()) : 768;
            }

            set
            {
                Registry.SetValue(HkeyAppPath, HValueAppSizeH, value.ToString());
            }
        }

        public static WindowState WindowState
        {
            get
            {
                return (WindowState)Registry.GetValue(HkeyAppPath, HValueAppState, WindowState.Normal);
            }

            set
            {
                Registry.SetValue(HkeyAppPath, HValueAppState, (int)value);
            }
        }

        public static string LastUpdateCheck
        {
            get
            {
                return (string)Registry.GetValue(HkeyAppPath, HValueLastUpdateCheck, "");
            }

            set
            {
                Registry.SetValue(HkeyAppPath, HValueLastUpdateCheck, value);
            }
        }

        public static string UpdateInterval
        {
            get
            {
                return (string)Registry.GetValue(HkeyAppPath, HValueCheckUpdateInterval, "");
            }

            set
            {
                Registry.SetValue(HkeyAppPath, HValueCheckUpdateInterval, value);
            }
        }

        public static string Language
        {
            get
            {
                return (string)Registry.GetValue(HkeyAppPath, HValueLanguage, "");
            }

            set
            {
                Registry.SetValue(HkeyAppPath, HValueLanguage, value);
            }
        }
    }
}
