using System;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;

using ImageViewer.Utilities;

namespace ImageViewer.Helpers
{
    internal class Culture
    {
        private static string _Language;
        private static Type _Class;

        public static void Init()
        {
            string classname;

            if (Settings.Language != "" && GetAvailableLanguages().Contains(Settings.Language))
            {
                classname = string.Concat("ImageViewer.Localization.", Settings.Language.UcFirst());
            }
            else
            {
                _Language = Windows.System.UserProfile.GlobalizationPreferences.Languages[0].Split('-')[0];
                classname = string.Concat("ImageViewer.Localization.", _Language.UcFirst());
            }

            _Class = Type.GetType(Type.GetType(classname) != null ? classname : "ImageViewer.Localization.En");
        }

        public static string GetLanguage()
        {
            return _Language;
        }

        public static List<string> GetAvailableLanguages()
        {
            List<string> list = new();
            list.AddRange(from Type type in Assembly.GetExecutingAssembly().GetTypes().Where(t => string.Equals(t.Namespace, "ImageViewer.Localization", StringComparison.Ordinal)).ToArray()
                          select type.Name.ToLower());
            return list;
        }

        public static string GetString(string key)
        {
            Dictionary<string, string> strings = (Dictionary<string, string>)_Class.GetMethod("GetStrings", BindingFlags.Public | BindingFlags.Static).Invoke(null, null);

            return strings.TryGetValue(key, out var s) ? s : $"[{key}]";
        }
    }
}
