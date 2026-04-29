using System;
using System.Net.Http;

using ImageViewer.Helpers;
using ImageViewer.Utilities;

using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

using Velopack;

namespace ImageViewer.Views;

public sealed partial class DialogAbout : Page
{
    private readonly ContentDialog Dialog;

    public DialogAbout(ContentDialog e)
    {
        InitializeComponent();
        Dialog = e;

        UpdateSettingsCard.Label = string.Concat("v", Context.GetProductVersion());
        UpdateSettingsCard.Description = string.Concat(Culture.GetString("ABOUT_LABEL_LAST_UPDATE"), Settings.LastUpdateCheck.ToUpdateDate());

        if(Context.Instance().PendingUpdate != null)
        {
            DisplayUpdateMessage();
        }
    }

    private void CloseButton_Click(object sender, RoutedEventArgs e)
    {
        Dialog.Hide();
    }

    private async void ButtonCheckUpdate_Click(object sender, RoutedEventArgs e)
    {
        UpdateStatusInfo.IsOpen = false;
        UpdateCheckingProgress.IsActive = true;
        ButtonCheckUpdate.Visibility = Visibility.Collapsed;
        UpdateCheckingText.Visibility = Visibility.Visible;
        ButtonDownloadUpdate.Visibility = Visibility.Collapsed;

        try
        {
            UpdateInfo info = await Context.Instance().UpdateMgr.CheckForUpdatesAsync();
            Settings.LastUpdateCheck = DateTime.Now.ToString("yyyy-MM-dd HH:mm:ss");

            if(info != null)
            {
                Context.Instance().SetPendingUpdate(info);
                DisplayUpdateMessage();
            }
            else
            {
                Context.Instance().SetPendingUpdate(null);
                UpdateStatusInfo.Severity = InfoBarSeverity.Success;
                UpdateStatusInfo.Title = Culture.GetString("ABOUT_UPDATE_INFO_UPDATE_LATEST");
                UpdateStatusInfo.IsOpen = true;
            }

            UpdateSettingsCard.Description = string.Concat(Culture.GetString("ABOUT_LABEL_LAST_UPDATE"), Settings.LastUpdateCheck.ToUpdateDate());
        }
        catch(HttpRequestException)
        {
            UpdateStatusInfo.Severity = InfoBarSeverity.Error;
            UpdateStatusInfo.Title = Culture.GetString("ABOUT_UPDATE_INFO_ERROR_NO_INTERNET");
            UpdateStatusInfo.IsOpen = true;
        }

        UpdateCheckingProgress.IsActive = false;
        ButtonCheckUpdate.Visibility = Visibility.Visible;
        UpdateCheckingText.Visibility = Visibility.Collapsed;
    }

    private void DisplayUpdateMessage()
    {
        UpdateStatusInfo.Severity = InfoBarSeverity.Warning;
        UpdateStatusInfo.Title = Culture.GetString("ABOUT_UPDATE_INFO_UPDATE_AVAILABLE");
        UpdateStatusInfo.IsOpen = true;

        ButtonDownloadUpdate.Visibility = Visibility.Visible;
    }

    private async void ButtonDownloadUpdate_Click(object sender, RoutedEventArgs e)
    {
        ButtonDownloadUpdate.IsEnabled = false;
        ButtonDownloadUpdate.Content = Culture.GetString("ABOUT_BTN_DOWNLOAD_UPDATE_DOWNLOADING");

        UpdateInfo pending = Context.Instance().PendingUpdate;
        if(pending == null) return;

        try
        {
            await Context.Instance().UpdateMgr.DownloadUpdatesAsync(pending);
            Context.Instance().UpdateMgr.ApplyUpdatesAndRestart(pending);
        }
        catch(Exception ex)
        {
            UpdateStatusInfo.Severity = InfoBarSeverity.Error;
            UpdateStatusInfo.Title = ex.Message;
            UpdateStatusInfo.IsOpen = true;

            ButtonDownloadUpdate.IsEnabled = true;
            ButtonDownloadUpdate.Content = Culture.GetString("ABOUT_BTN_DOWNLOAD_UPDATE_RETRY");
        }
    }
}
