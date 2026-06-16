using System;
using System.Diagnostics;
using System.Globalization;
using System.Threading.Tasks;

using Microsoft.Windows.AppNotifications.Builder;

using Velopack;
using Velopack.Sources;

namespace ImageViewer.Helpers;

/// <summary>
/// Owns the Velopack update flow: querying the GitHub source, caching the pending update,
/// applying it, and the interval-gated background check that raises the update toast.
/// </summary>
internal sealed class UpdateService
{
    // Built lazily so the Velopack assemblies are not loaded on the startup path:
    // the first touch happens from the deferred CheckUpdate(), after the window is shown.
    private UpdateManager _UpdateManager;
    private UpdateManager UpdateManager => _UpdateManager ??= new UpdateManager(
        new GithubSource(
            repoUrl: "https://github.com/dragonofmercy/image-viewer",
            accessToken: null,
            prerelease: false));

    public UpdateInfo PendingUpdate { get; private set; }

    /// <summary>
    /// Query the update source and cache the result in <see cref="PendingUpdate"/>.
    /// Returns null when no update is available or when running outside a Velopack install.
    /// Network/parse errors bubble to the caller so the UI can react.
    /// </summary>
    public async Task<UpdateInfo> CheckForUpdateAsync()
    {
        if (!UpdateManager.IsInstalled)
        {
            PendingUpdate = null;
            return null;
        }

        UpdateInfo info = await UpdateManager.CheckForUpdatesAsync();
        Settings.TouchLastUpdateCheck();
        PendingUpdate = info;
        return info;
    }

    /// <summary>
    /// Download <see cref="PendingUpdate"/> and restart into the new version.
    /// No-op when there is no pending update.
    /// </summary>
    public async Task ApplyPendingUpdateAsync()
    {
        if (PendingUpdate == null) return;

        await UpdateManager.DownloadUpdatesAsync(PendingUpdate);
        UpdateManager.ApplyUpdatesAndRestart(PendingUpdate);
    }

    /// <summary>
    /// Background update check honoring the UpdateInterval setting. Errors are swallowed.
    /// Shows the "update available" toast through the supplied notifications manager.
    /// </summary>
    public async void CheckUpdate(NotificationsManger notifications)
    {
        if (!UpdateManager.IsInstalled) return;

        if (string.IsNullOrEmpty(Settings.UpdateInterval))
        {
            return;
        }

        // A corrupted registry value must not prevent startup: an unparseable date simply triggers a new check
        if (!string.IsNullOrEmpty(Settings.LastUpdateCheck) && DateTime.TryParseExact(Settings.LastUpdateCheck, Settings.UPDATE_DATE_FORMAT, CultureInfo.InvariantCulture, DateTimeStyles.None, out DateTime lastCheck))
        {
            switch (Settings.UpdateInterval)
            {
                case "day":
                    lastCheck = lastCheck.AddDays(1);
                    break;
                case "week":
                    lastCheck = lastCheck.AddDays(7);
                    break;
                default:
                    lastCheck = lastCheck.AddMonths(1);
                    break;
            }

            if (lastCheck.Date > DateTime.Now.Date)
            {
                return;
            }
        }

        try
        {
            if (await CheckForUpdateAsync() == null) return;
        }
        catch (Exception ex)
        {
            Debug.WriteLine($"Update check failed: {ex.Message}");
            return;
        }

        AppNotificationBuilder builder = new AppNotificationBuilder()
            .AddText(Culture.GetString("ABOUT_UPDATE_INFO_UPDATE_AVAILABLE"))
            .AddButton(new AppNotificationButton(Culture.GetString("ABOUT_BTN_DOWNLOAD_UPDATE")).AddArgument("action", "doUpdate"));

        notifications.Runtime.Show(builder.BuildNotification());
    }
}
