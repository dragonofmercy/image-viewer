using System.Collections.Generic;

using Microsoft.Windows.ApplicationModel.Resources;

namespace ImageViewer.Helpers;

internal class Culture
{
    private static ResourceManager _Manager;
    private static ResourceContext _Context;

    // Supported BCP-47 tags; en-US is the default language.
    private static readonly string[] AvailableLanguages = { "en-US", "fr-FR", "zh-Hans", "de-DE", "es-ES", "it-IT", "pt-BR" };

    public static void Init()
    {
        _Manager = new ResourceManager();
        _Context = _Manager.CreateResourceContext();

        // Empty Settings.Language means follow the system language (no override)
        if(!string.IsNullOrEmpty(Settings.Language))
        {
            _Context.QualifierValues["Language"] = Settings.Language;
        }
    }

    public static List<string> GetAvailableLanguages()
    {
        return new List<string>(AvailableLanguages);
    }

    public static string GetString(string key)
    {
        if(_Manager == null) return $"[{key}]";

        ResourceCandidate candidate = _Manager.MainResourceMap.TryGetValue($"Resources/{key}", _Context);
        return candidate != null ? candidate.ValueAsString : $"[{key}]";
    }
}
