using System.Globalization;
using Microsoft.UI.Xaml;
using Microsoft.Win32;
using WinUIEx;

namespace ImageViewer.Helpers;

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
    internal const string H_VALUE_JPEG_QUALITY = "JpegQuality";
    internal const string UPDATE_DATE_FORMAT = "yyyy-MM-dd HH:mm:ss";
    internal const int JPEG_QUALITY_DEFAULT = 100;

    public static ElementTheme Theme
    {
        get
        {
            object tmp = Registry.GetValue(HKEY_APP_PATH, H_VALUE_APP_THEME, ElementTheme.Default);
            return tmp != null ? (ElementTheme)tmp : ElementTheme.Default;
        }

        set => Registry.SetValue(HKEY_APP_PATH, H_VALUE_APP_THEME, (int)value);
    }

    public static int? AppPositionX
    {
        get
        {
            object tmp = Registry.GetValue(HKEY_APP_PATH, H_VALUE_APP_POSITION_X, null);
            return tmp != null ? int.Parse(tmp.ToString()) : null;
        }

        set => Registry.SetValue(HKEY_APP_PATH, H_VALUE_APP_POSITION_X, value.ToString());
    }

    public static int? AppPositionY
    {
        get
        {
            object tmp = Registry.GetValue(HKEY_APP_PATH, H_VALUE_APP_POSITION_Y, null);
            return tmp != null ? int.Parse(tmp.ToString()) : null;
        }

        set => Registry.SetValue(HKEY_APP_PATH, H_VALUE_APP_POSITION_Y, value.ToString());
    }

    public static uint AppSizeW
    {
        get
        {
            object tmp = Registry.GetValue(HKEY_APP_PATH, H_VALUE_APP_SIZE_W, null);
            return tmp != null ? uint.Parse(tmp.ToString()) : 1280;
        }

        set => Registry.SetValue(HKEY_APP_PATH, H_VALUE_APP_SIZE_W, value.ToString());
    }

    public static uint AppSizeH
    {
        get
        {
            object tmp = Registry.GetValue(HKEY_APP_PATH, H_VALUE_APP_SIZE_H, null);
            return tmp != null ? uint.Parse(tmp.ToString()) : 768;
        }

        set => Registry.SetValue(HKEY_APP_PATH, H_VALUE_APP_SIZE_H, value.ToString());
    }

    public static WindowState WindowState
    {
        get => (WindowState)Registry.GetValue(HKEY_APP_PATH, H_VALUE_APP_STATE, WindowState.Normal);
        set => Registry.SetValue(HKEY_APP_PATH, H_VALUE_APP_STATE, (int)value);
    }

    public static string LastUpdateCheck
    {
        get => (string)Registry.GetValue(HKEY_APP_PATH, H_VALUE_LAST_UPDATE_CHECK, "");
        set => Registry.SetValue(HKEY_APP_PATH, H_VALUE_LAST_UPDATE_CHECK, value);
    }

    public static void TouchLastUpdateCheck()
    {
        LastUpdateCheck = System.DateTime.Now.ToString(UPDATE_DATE_FORMAT, CultureInfo.InvariantCulture);
    }

    public static string UpdateInterval
    {
        get => (string)Registry.GetValue(HKEY_APP_PATH, H_VALUE_CHECK_UPDATE_INTERVAL, "");
        set => Registry.SetValue(HKEY_APP_PATH, H_VALUE_CHECK_UPDATE_INTERVAL, value);
    }

    public static string Language
    {
        get => (string)Registry.GetValue(HKEY_APP_PATH, H_VALUE_LANGUAGE, "");
        set => Registry.SetValue(HKEY_APP_PATH, H_VALUE_LANGUAGE, value);
    }

    public static int JpegQuality
    {
        get
        {
            object tmp = Registry.GetValue(HKEY_APP_PATH, H_VALUE_JPEG_QUALITY, null);
            return tmp != null && int.TryParse(tmp.ToString(), out int q) ? System.Math.Clamp(q, 1, 100) : JPEG_QUALITY_DEFAULT;
        }

        set => Registry.SetValue(HKEY_APP_PATH, H_VALUE_JPEG_QUALITY, System.Math.Clamp(value, 1, 100));
    }
}