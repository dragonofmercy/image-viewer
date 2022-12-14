using System;
using System.Collections.Generic;
using System.Net.Http;

using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace ImageViewer
{
    public sealed partial class About : Page
    {
        private readonly ContentDialog Dialog;

        public About(ContentDialog e)
        {
            InitializeComponent();
            Dialog = e;

            CurrentVersionText.Text = string.Concat("v", Context.GetProductVersion());
            LastCheckedText.Text = string.Concat(Culture.GetString("ABOUT_LABEL_LAST_UPDATE"), Settings.LastUpdateCheck.ToUpdateDate());
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
                await Update.GetRemoteData();

                string remoteVersion = Update.GetRemoteVersion();

                DateTime dateTimeNow = DateTime.Now;
                LastCheckedText.Text = string.Concat(Culture.GetString("ABOUT_LABEL_LAST_UPDATE"), dateTimeNow.ToString());
                Settings.LastUpdateCheck = dateTimeNow.ToString();

                if(string.Compare(remoteVersion, Context.GetProductVersion(), StringComparison.InvariantCulture) > 0)
                {
                    UpdateStatusInfo.Severity = InfoBarSeverity.Warning;
                    UpdateStatusInfo.Title = Culture.GetString("ABOUT_UPDATE_INFO_UPDATE_AVAILABLE");
                    UpdateStatusInfo.IsOpen = true;

                    ButtonDownloadUpdate.Visibility = Visibility.Visible;
                }
                else
                {
                    UpdateStatusInfo.Severity = InfoBarSeverity.Success;
                    UpdateStatusInfo.Title = Culture.GetString("ABOUT_UPDATE_INFO_UPDATE_LATEST");
                    UpdateStatusInfo.IsOpen = true;
                }
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
