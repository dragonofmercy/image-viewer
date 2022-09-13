using System;
using System.Collections.Generic;
using System.Reflection;

namespace ImageViewer
{
    internal class Culture
    {
        private static string Language;
        private static Type Class;

        public Culture()
        {
            Language = Windows.System.UserProfile.GlobalizationPreferences.Languages[0].Split('-')[0];
            string classname = string.Concat("ImageViewer.Localization.", char.ToUpper(Language[0]) + Language[1..]);

            if(Type.GetType(classname) != null)
            {
                Class = Type.GetType(classname);
            }
            else
            {
                Class = Type.GetType("ImageViewer.Localization.En");
            }
        }

        public static string GetLanguage()
        {
            return Language;
        }

        public static string GetString(string key)
        {
            Dictionary<string, string> strings = (Dictionary<string, string>)Class.GetMethod("GetStrings", BindingFlags.Public | BindingFlags.Static).Invoke(null, null);

            if(strings.ContainsKey(key))
            {
                return strings[key];
            }

            return string.Format("[{0}]", key);
        }
    }
}
