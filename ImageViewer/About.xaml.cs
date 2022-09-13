using System;

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

            e.Opened += DialogOpened;
            Dialog = e;
        }

        private void DialogOpened(ContentDialog sender, ContentDialogOpenedEventArgs args)
        {
            CheckRemoteVersion();
        }

        private async void CheckRemoteVersion()
        {
            string remoteVersion = await Update.GetRemoteVersion();
            TextRemoteVersion.Text = remoteVersion;

            if(string.Compare(remoteVersion, Context.GetProductVersion(), StringComparison.InvariantCulture) > 0)
            {
                ButtonUpdate.IsEnabled = true;
            }
        }

        private void ButtonClose_Click(object sender, RoutedEventArgs e)
        {
            Dialog.Hide();
        }

        private void ButtonUpdate_Click(object sender, RoutedEventArgs e)
        {

        }
    }
}
