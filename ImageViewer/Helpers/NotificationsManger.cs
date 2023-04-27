using System;
using System.Threading.Tasks;

using Microsoft.Windows.AppNotifications;
using Microsoft.Windows.AppNotifications.Builder;

namespace ImageViewer.Helpers;

internal class NotificationsManger
{
    public AppNotificationManager Runtime;

    public NotificationsManger()
    {
        AppNotificationManager notificationManager = AppNotificationManager.Default;
        notificationManager.NotificationInvoked += NotificationManager_NotificationInvoked;
        notificationManager.Register();

        Runtime = notificationManager;
    }

    public async void Clear()
    {
        await Runtime.RemoveAllAsync();
    }

    private void NotificationManager_NotificationInvoked(AppNotificationManager sender, AppNotificationActivatedEventArgs args)
    {
        _ = HandleNotificationAsync(args);
    }

    private async Task HandleNotificationAsync(AppNotificationActivatedEventArgs args)
    {
        switch (args.Arguments["action"])
        {
            case "doUpdate":
                try
                {
                    await Update.ApplyUpdate();
                }
                catch (Exception ex)
                {
                    AppNotificationBuilder builder = new AppNotificationBuilder()
                        .AddText(ex.Message);

                    Runtime.Show(builder.BuildNotification());
                }
                break;
        }
    }
}