using System;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;

namespace ImageViewer
{
    internal class Culture
    {
        private static string Language;
        private static Type Class;

        public static void Init()
        {
            string classname;

            if(Settings.Language != "" && GetAvailableLanguages().Contains(Settings.Language))
            {
                classname = string.Concat("ImageViewer.Localization.", Settings.Language.UcFirst());
            }
            else
            {
                Language = Windows.System.UserProfile.GlobalizationPreferences.Languages[0].Split('-')[0];
                classname = string.Concat("ImageViewer.Localization.", Language.UcFirst());
            }
            
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

        public static List<string> GetAvailableLanguages() 
        {
            List<string> list = new();
            list.AddRange(from Type type in Assembly.GetExecutingAssembly().GetTypes().Where(t => string.Equals(t.Namespace, "ImageViewer.Localization", StringComparison.Ordinal)).ToArray()
                          select type.Name.ToLower());
            return list;
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
