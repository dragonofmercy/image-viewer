using System;
using System.Collections.Generic;
using System.Net.Http;

using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace ImageViewer
{
    public sealed partial class DialogAbout : Page
    {
        private readonly ContentDialog Dialog;

        public DialogAbout(ContentDialog e)
        {
            InitializeComponent();
            Dialog = e;

            CurrentVersionText.Text = string.Concat("v", Context.GetProductVersion());
            LastCheckedText.Text = string.Concat(Culture.GetString("ABOUT_LABEL_LAST_UPDATE"), Settings.LastUpdateCheck.ToUpdateDate());

            if(Update.HasUpdate)
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
                if(await Update.CheckNewVersionAsync())
                {
                    DisplayUpdateMessage();
                }
                else
                {
                    UpdateStatusInfo.Severity = InfoBarSeverity.Success;
                    UpdateStatusInfo.Title = Culture.GetString("ABOUT_UPDATE_INFO_UPDATE_LATEST");
                    UpdateStatusInfo.IsOpen = true;
                }

                LastCheckedText.Text = string.Concat(Culture.GetString("ABOUT_LABEL_LAST_UPDATE"), Settings.LastUpdateCheck.ToUpdateDate());
            }
            catch(KeyNotFoundException)
            {
                UpdateStatusInfo.Severity = InfoBarSeverity.Error;
                UpdateStatusInfo.Title = Culture.GetString("ABOUT_UPDATE_INFO_ERROR_KEY_NOT_FOUND");
                UpdateStatusInfo.IsOpen = true;
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

            try
            {
                await Update.ApplyUpdate();
            }
            catch(Exception ex)
            {
                ButtonDownloadUpdate.Visibility = Visibility.Collapsed;

                UpdateStatusInfo.Severity = InfoBarSeverity.Error;
                UpdateStatusInfo.Title = ex.Message;
                UpdateStatusInfo.IsOpen = true;
            }
        }
    }
}
