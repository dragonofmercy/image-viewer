using System;
using System.Threading.Tasks;

using Microsoft.Windows.AppNotifications;
using Microsoft.Windows.AppNotifications.Builder;

using ImageViewer.Helpers;

namespace ImageViewer.Services;

internal class NotificationsService
{
    public readonly AppNotificationManager Runtime;

    public NotificationsService()
    {
        AppNotificationManager notificationManager = AppNotificationManager.Default;
        notificationManager.NotificationInvoked += NotificationManager_NotificationInvoked;
        notificationManager.Register();

        Runtime = notificationManager;
    }

    public async Task Clear()
    {
        await Runtime.RemoveAllAsync();
    }

    private void NotificationManager_NotificationInvoked(AppNotificationManager sender, AppNotificationActivatedEventArgs args)
    {
        // Fires on a COM background thread. The argument map is EMPTY when the toast body is
        // clicked instead of one of its buttons, so never index into it - a KeyNotFoundException
        // here is invisible and looks exactly like a dead notification.
        args.Arguments.TryGetValue("action", out string action);

        MainWindow window = Context.Instance().MainWindow;
        if (window == null) return;

        // Never run the update silently in the background: the about dialog is where the download
        // has a progress readout, an error banner and a retry button.
        window.DispatcherQueue.TryEnqueue(async () =>
        {
            try
            {
                await window.ShowAbout(action == "doUpdate");
            }
            catch (Exception ex)
            {
                AppNotificationBuilder builder = new AppNotificationBuilder()
                    .AddText(ex.Message);

                Runtime.Show(builder.BuildNotification());
            }
        });
    }
}