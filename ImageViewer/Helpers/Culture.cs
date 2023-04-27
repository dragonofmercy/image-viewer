using System;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;

using ImageViewer.Utilities;

namespace ImageViewer.Helpers;

internal class Culture
{
    private const string LIBRARY_PATH = "ImageViewer.Strings";
    private static Type _Class;

    public static void Init()
    {
        string classname;

        if(Settings.Language != "" && GetAvailableLanguages().Contains(Settings.Language))
        {
            classname = string.Concat(LIBRARY_PATH, ".", Settings.Language.UcFirst());
        }
        else
        {
            classname = string.Concat(LIBRARY_PATH, ".", Windows.System.UserProfile.GlobalizationPreferences.Languages[0].Split('-')[0].UcFirst());
        }

        _Class = Type.GetType(Type.GetType(classname) != null ? classname : string.Concat(LIBRARY_PATH, ".En"));
    }

    public static List<string> GetAvailableLanguages()
    {
        List<string> list = new();
        list.AddRange(from Type type in Assembly.GetExecutingAssembly().GetTypes().Where(t => string.Equals(t.Namespace, LIBRARY_PATH, StringComparison.Ordinal)).ToArray()
            select type.Name.ToLower());
        return list;
    }

    public static string GetString(string key)
    {
        Dictionary<string, string> strings = (Dictionary<string, string>)_Class.GetMethod("GetStrings", BindingFlags.Public | BindingFlags.Static).Invoke(null, null);

        return strings.TryGetValue(key, out var s) ? s : $"[{key}]";
    }
}