using Microsoft.UI.Xaml;
using Microsoft.Win32;

namespace ImageViewer
{
    internal class Settings
    {
        internal const string HkeyAppPath = @"HKEY_CURRENT_USER\SOFTWARE\Dragon Industries\Image Viewer";
        internal const string HValueAppTheme = "AppTheme";
        
        public static ElementTheme Theme 
        {
            get
            {
                int theme = (int)Registry.GetValue(HkeyAppPath, HValueAppTheme, ElementTheme.Default);
                return (ElementTheme)theme;
            }

            set
            {
                Registry.SetValue(HkeyAppPath, HValueAppTheme, (int)value);
            }
         }
    }
}
