using System.Collections.Generic;
using System.Globalization;
using System.Linq;

using ImageViewer.Helpers;
using ImageViewer.Utilities;

using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace ImageViewer.Views;

public sealed partial class DialogSettings : Page
{
    private readonly Dictionary<string, string> AvailableLanguages = new()
    {
        { Culture.GetString("DEFAULT_SYSTEM_LANGUAGE"), "" }
    };

    private readonly Dictionary<string, string> UpdatesIntervals = new()
    {
        { Culture.GetString("SETTINGS_FIELD_UPDATE_INTERVAL_DAY"), "day" },
        { Culture.GetString("SETTINGS_FIELD_UPDATE_INTERVAL_WEEK"), "week" },
        { Culture.GetString("SETTINGS_FIELD_UPDATE_INTERVAL_MONTH"), "month" },
        { Culture.GetString("SETTINGS_FIELD_UPDATE_INTERVAL_MANUAL"), "" },
    };
    private readonly ContentDialog Dialog;

    public DialogSettings(ContentDialog e)
    {
        InitializeComponent();
        Dialog = e;

        foreach(string languagesIso in Culture.GetAvailableLanguages()) 
        {
            AvailableLanguages.Add(new CultureInfo(languagesIso).NativeName.UcFirst(), languagesIso.ToLower());
        }

        CboOptionsLanguage.ItemsSource = AvailableLanguages.Select(kv => new { Key = kv.Key, Value = kv.Value }).ToList();
        CboOptionsUpdateInterval.ItemsSource = UpdatesIntervals.Select(kv => new { Key = kv.Key, Value = kv.Value }).ToList();
    }

    private void Page_Loaded(object sender, RoutedEventArgs e)
    {
        CboOptionsLanguage.SelectedValue = Settings.Language;
        CboOptionsUpdateInterval.SelectedValue = Settings.UpdateInterval;
    }

    private void CloseButton_Click(object sender, RoutedEventArgs e)
    {
        Dialog.Hide();
    }

    private void CboOptionsLanguage_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        Settings.Language = CboOptionsLanguage.SelectedValue.ToString();
    }

    private void CboOptionsUpdateInterval_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        Settings.UpdateInterval = CboOptionsUpdateInterval.SelectedValue.ToString();
    }
}