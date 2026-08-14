using System;
using System.Net.Http;
using System.Threading.Tasks;

using ImageViewer.Helpers;
using ImageViewer.Utilities;

using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace ImageViewer.Views;

public sealed partial class DialogAbout : Page
{
    private readonly ContentDialog Dialog;

    public DialogAbout(ContentDialog e, bool startUpdate = false)
    {
        InitializeComponent();
        Dialog = e;

        UpdateSettingsCard.Label = string.Concat("v", AppInfo.ProductVersion);
        UpdateSettingsCard.Description = string.Concat(Culture.GetString("ABOUT_LABEL_LAST_UPDATE"), Settings.LastUpdateCheck.ToUpdateDate());

        // startUpdate comes from the update toast: the user already asked for the update, so run it
        // straight away rather than making them click the same thing twice.
        if(startUpdate)
        {
            _ = DownloadUpdate();
        }
        else if(Context.Instance().UpdateService.PendingUpdate != null)
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
            if(await Context.Instance().UpdateService.CheckForUpdateAsync() != null)
            {
                DisplayUpdateMessage();
            }
            else
            {
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
        catch(Exception ex)
        {
            UpdateStatusInfo.Severity = InfoBarSeverity.Error;
            UpdateStatusInfo.Title = ex.Message;
            UpdateStatusInfo.IsOpen = true;
        }
        finally
        {
            UpdateCheckingProgress.IsActive = false;
            ButtonCheckUpdate.Visibility = Visibility.Visible;
            UpdateCheckingText.Visibility = Visibility.Collapsed;
        }
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
        await DownloadUpdate();
    }

    /// <summary>
    /// Download the pending update and restart into it, reporting the percentage on the button.
    /// Re-resolves the update when nothing is pending: the toast can outlive the process that
    /// raised it, so an instance started by the click has an empty cache.
    /// </summary>
    private async Task DownloadUpdate()
    {
        string downloading = Culture.GetString("ABOUT_BTN_DOWNLOAD_UPDATE_DOWNLOADING");

        ButtonDownloadUpdate.IsEnabled = false;
        ButtonDownloadUpdate.Visibility = Visibility.Visible;
        ButtonDownloadUpdate.Content = downloading;

        try
        {
            if(Context.Instance().UpdateService.PendingUpdate == null && await Context.Instance().UpdateService.CheckForUpdateAsync() == null)
            {
                UpdateStatusInfo.Severity = InfoBarSeverity.Success;
                UpdateStatusInfo.Title = Culture.GetString("ABOUT_UPDATE_INFO_UPDATE_LATEST");
                UpdateStatusInfo.IsOpen = true;
                ButtonDownloadUpdate.Visibility = Visibility.Collapsed;
                return;
            }

            DisplayUpdateMessage();

            await Context.Instance().UpdateService.ApplyPendingUpdateAsync(percent => DispatcherQueue.TryEnqueue(() => ButtonDownloadUpdate.Content = string.Concat(downloading, " ", percent, "%")));
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
