using System.Collections.Generic;
using System.Globalization;
using System.Linq;

using ImageViewer.Helpers;
using ImageViewer.Utilities;

using Microsoft.UI.Composition.SystemBackdrops;
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
    private readonly Dictionary<string, ElementTheme> Themes = new()
    {
        { Culture.GetString("SETTINGS_FIELD_THEME_SYSTEM"), ElementTheme.Default },
        { Culture.GetString("SETTINGS_FIELD_THEME_LIGHT"), ElementTheme.Light },
        { Culture.GetString("SETTINGS_FIELD_THEME_DARK"), ElementTheme.Dark },
    };
    // The combo carries the value as int: a C# enum does not cross into WinRT, so SelectedValue would never match
    private readonly Dictionary<string, Backdrop> Backdrops = new()
    {
        { Culture.GetString("SETTINGS_FIELD_BACKDROP_BASIC"), Backdrop.Basic },
        { Culture.GetString("SETTINGS_FIELD_BACKDROP_MICA"), Backdrop.Mica },
        { Culture.GetString("SETTINGS_FIELD_BACKDROP_ACRYLIC"), Backdrop.Acrylic },
    };
    private readonly ContentDialog Dialog;

    public DialogSettings(ContentDialog e)
    {
        InitializeComponent();
        Dialog = e;

        foreach(string languagesIso in Culture.AvailableLanguages)
        {
            AvailableLanguages.Add(new CultureInfo(languagesIso).NativeName.UcFirst(), languagesIso);
        }

        CboOptionsLanguage.ItemsSource = AvailableLanguages.Select(kv => new { Key = kv.Key, Value = kv.Value }).ToList();
        CboOptionsTheme.ItemsSource = Themes.Select(kv => new { Key = kv.Key, Value = kv.Value }).ToList();
        if(!MicaController.IsSupported())
        {
            Backdrops.Remove(Culture.GetString("SETTINGS_FIELD_BACKDROP_MICA"));
        }

        CboOptionsBackdrop.ItemsSource = Backdrops.Select(kv => new { Key = kv.Key, Value = (int)kv.Value }).ToList();
        CboOptionsUpdateInterval.ItemsSource = UpdatesIntervals.Select(kv => new { Key = kv.Key, Value = kv.Value }).ToList();
    }

    private void Page_Loaded(object sender, RoutedEventArgs e)
    {
        CboOptionsLanguage.SelectedValue = Settings.Language;
        CboOptionsTheme.SelectedValue = Settings.Theme;
        CboOptionsBackdrop.SelectedValue = (int)Settings.Backdrop;
        CboOptionsUpdateInterval.SelectedValue = Settings.UpdateInterval;
        NumOptionsJpegQuality.Value = Settings.JpegQuality;
        NumOptionsWebpQuality.Value = Settings.WebpQuality;
    }

    private void CloseButton_Click(object sender, RoutedEventArgs e)
    {
        Dialog.Hide();
    }

    private void CboOptionsLanguage_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        Settings.Language = CboOptionsLanguage.SelectedValue.ToString();
    }

    private void CboOptionsTheme_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        ElementTheme theme = (ElementTheme)CboOptionsTheme.SelectedValue;
        if(theme == Settings.Theme) return;

        Context.Instance().ChangeTheme(theme);

        // The dialog was themed when it opened; Default here follows the OS, same as the window
        Dialog.RequestedTheme = theme;
    }

    private void CboOptionsBackdrop_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        Backdrop backdrop = (Backdrop)(int)CboOptionsBackdrop.SelectedValue;
        if(backdrop == Settings.Backdrop) return;

        Context.Instance().ChangeBackdrop(backdrop);
    }

    private void CboOptionsUpdateInterval_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        Settings.UpdateInterval = CboOptionsUpdateInterval.SelectedValue.ToString();
    }

    private void NumOptionsJpegQuality_ValueChanged(NumberBox sender, NumberBoxValueChangedEventArgs e)
    {
        if(double.IsNaN(e.NewValue)) return;
        int value = (int)e.NewValue;
        if(value == Settings.JpegQuality) return;
        Settings.JpegQuality = value;
    }

    private void NumOptionsWebpQuality_ValueChanged(NumberBox sender, NumberBoxValueChangedEventArgs e)
    {
        if(double.IsNaN(e.NewValue)) return;
        int value = (int)e.NewValue;
        if(value == Settings.WebpQuality) return;
        Settings.WebpQuality = value;
    }
}