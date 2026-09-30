using System;
using System.Net.Http;
using System.Threading;
using System.Threading.Tasks;

using ImageViewer.Helpers;
using ImageViewer.Utilities;

using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace ImageViewer.Views;

public sealed partial class DialogAbout : Page
{
    private readonly ContentDialog Dialog;

    // Non-null while an update runs (check then download): guards against a second start and lets closing the dialog abort it
    private CancellationTokenSource Updating;

    public DialogAbout(ContentDialog e, bool startUpdate = false)
    {
        InitializeComponent();
        Dialog = e;

        UpdateSettingsCard.Label = string.Concat("v", AppInfo.ProductVersion);
        RefreshLastUpdateCheck();

        // startUpdate comes from the update toast: the user already asked for the update, so run it straight away.
        // Wait for Opened: a dialog that never makes it on screen must not leave an invisible download behind.
        if(startUpdate)
        {
            Dialog.Opened += (_, _) => StartUpdate();
        }
        else if(Context.Instance().UpdateService.PendingUpdate != null)
        {
            DisplayUpdateMessage();
        }

        // The dialog is the only place showing the download's progress and errors, so it must not outlive it
        Dialog.Closed += (_, _) => Updating?.Cancel();
    }

    /// <summary>
    /// Start the update unless one is already running in this dialog.
    /// </summary>
    public void StartUpdate()
    {
        if(Updating == null)
        {
            _ = DownloadUpdate();
        }
    }

    private void CloseButton_Click(object sender, RoutedEventArgs e)
    {
        Dialog.Hide();
    }

    private async void ButtonCheckUpdate_Click(object sender, RoutedEventArgs e)
    {
        await CheckForUpdate();
    }

    private void ButtonDownloadUpdate_Click(object sender, RoutedEventArgs e)
    {
        StartUpdate();
    }

    /// <summary>
    /// Query the update source and show the outcome. Returns true when an update is available.
    /// </summary>
    private async Task<bool> CheckForUpdate()
    {
        UpdateStatusInfo.IsOpen = false;
        ButtonDownloadUpdate.Visibility = Visibility.Collapsed;
        UpdateCheckingProgress.IsActive = true;
        ButtonCheckUpdate.Visibility = Visibility.Collapsed;
        UpdateCheckingText.Visibility = Visibility.Visible;

        try
        {
            if(await Context.Instance().UpdateService.CheckForUpdateAsync() != null)
            {
                DisplayUpdateMessage();
                return true;
            }

            DisplayStatus(InfoBarSeverity.Success, Culture.GetString("ABOUT_UPDATE_INFO_UPDATE_LATEST"));
            return false;
        }
        catch(Exception ex)
        {
            DisplayError(ex);
            return false;
        }
        finally
        {
            UpdateCheckingProgress.IsActive = false;
            ButtonCheckUpdate.Visibility = Visibility.Visible;
            UpdateCheckingText.Visibility = Visibility.Collapsed;
            RefreshLastUpdateCheck();
        }
    }

    /// <summary>
    /// Download the pending update and restart into it, reporting the percentage on the button.
    /// Re-resolves the update when nothing is pending: the toast can outlive the process that
    /// raised it, so an instance started by the click has an empty cache.
    /// </summary>
    private async Task DownloadUpdate()
    {
        CancellationTokenSource updating = Updating = new CancellationTokenSource();

        // A check running under the download would hide its progress and swap the pending update
        ButtonCheckUpdate.IsEnabled = false;

        try
        {
            if(Context.Instance().UpdateService.PendingUpdate == null && !await CheckForUpdate())
            {
                return;
            }

            string downloading = Culture.GetString("ABOUT_BTN_DOWNLOAD_UPDATE_DOWNLOADING");

            DisplayUpdateMessage();
            ButtonDownloadUpdate.IsEnabled = false;
            ButtonDownloadUpdate.Content = downloading;

            await Context.Instance().UpdateService.DownloadAndRestartAsync(percent => DispatcherQueue.TryEnqueue(() => ButtonDownloadUpdate.Content = string.Concat(downloading, " ", percent, "%")), updating.Token);
        }
        catch(OperationCanceledException)
        {
            // The dialog was closed: nobody is left to read a status
        }
        catch(Exception ex)
        {
            DisplayError(ex);
            ButtonDownloadUpdate.Content = Culture.GetString("ABOUT_BTN_DOWNLOAD_UPDATE_RETRY");
        }
        finally
        {
            Updating = null;
            updating.Dispose();
            ButtonDownloadUpdate.IsEnabled = true;
            ButtonCheckUpdate.IsEnabled = true;
        }
    }

    private void DisplayUpdateMessage()
    {
        DisplayStatus(InfoBarSeverity.Warning, Culture.GetString("ABOUT_UPDATE_INFO_UPDATE_AVAILABLE"));

        ButtonDownloadUpdate.Content = Culture.GetString("ABOUT_BTN_DOWNLOAD_UPDATE");
        ButtonDownloadUpdate.Visibility = Visibility.Visible;
    }

    private void DisplayError(Exception ex)
    {
        DisplayStatus(InfoBarSeverity.Error, ex is HttpRequestException ? Culture.GetString("ABOUT_UPDATE_INFO_ERROR_NO_INTERNET") : ex.Message);
    }

    private void DisplayStatus(InfoBarSeverity severity, string title)
    {
        UpdateStatusInfo.Severity = severity;
        UpdateStatusInfo.Title = title;
        UpdateStatusInfo.IsOpen = true;
    }

    private void RefreshLastUpdateCheck()
    {
        UpdateSettingsCard.Description = string.Concat(Culture.GetString("ABOUT_LABEL_LAST_UPDATE"), Settings.LastUpdateCheck.ToUpdateDate());
    }
}
