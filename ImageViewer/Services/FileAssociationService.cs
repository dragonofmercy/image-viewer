using System;
using System.Diagnostics;
using System.Runtime.InteropServices;

using Microsoft.Win32;

using Velopack.Locators;

using ImageViewer.Helpers;
using ImageViewer.Wrapper;

namespace ImageViewer.Services;

/// <summary>
/// Registers the app as an "Open with" handler for every supported image type, under
/// HKEY_CURRENT_USER only (no elevation, no per-machine state). Writes OpenWithProgids and
/// an Applications block, never the default handler of an extension: Windows keeps asking
/// the user which application to use. Registration only happens for a real Velopack
/// (Setup.exe) install, because only that install form leaves an uninstaller behind to undo it;
/// unregistration itself stays unconditional so an install made under an older build can still
/// be cleaned up.
///
/// Callers: App.OnLaunched (deferred, Release builds only) and the Velopack uninstall hook.
/// </summary>
internal static class FileAssociationService
{
    private const int SHCNE_ASSOCCHANGED = 0x08000000;
    private const uint SHCNF_IDLIST = 0x0000;

    [DllImport("shell32.dll", SetLastError = false)]
    private static extern void SHChangeNotify(int wEventId, uint uFlags, IntPtr dwItem1, IntPtr dwItem2);

    /// <summary>
    /// Write the association keys unless they are already in place for this executable, and
    /// only when running from a real Velopack (Setup.exe) install. On every launch after the
    /// first this costs two registry reads and no write. Never throws: a failed association
    /// must not break startup.
    /// </summary>
    internal static void EnsureRegistered()
    {
        try
        {
            // A null or empty ProcessPath makes FileAssociationPlan.ApplicationKeyPath resolve
            // to "Software\Classes\Applications\" (Path.GetFileName("") is ""), which .NET
            // treats as the shared Applications key itself. Writing to it would stamp values
            // onto every per-user Open With registration on the machine, so bail out instead.
            if (string.IsNullOrEmpty(Environment.ProcessPath)) return;

            // Only a Setup.exe install leaves an uninstaller behind that can later run
            // Unregister() through the OnBeforeUninstallFastCallback hook, so only that install
            // form is allowed to write these keys. CurrentlyInstalledVersion is null when
            // Velopack finds no install manifest next to the executable (a raw bin\Release
            // copy the developer ran directly, never installed at all), and IsPortable is true
            // for the portable zip (it ships with Update.exe and self-updates but has no
            // uninstaller entry). Either case would register keys nothing can ever remove.
            IVelopackLocator locator = VelopackLocator.Current;

            if (locator.CurrentlyInstalledVersion == null || locator.IsPortable) return;

            FileAssociationPlan plan = BuildPlan();

            if (IsRegistered(plan)) return;

            foreach (RegistryEntry entry in plan.EntriesToWrite())
            {
                using RegistryKey key = Registry.CurrentUser.CreateSubKey(entry.KeyPath);
                key.SetValue(entry.ValueName ?? string.Empty, entry.Value, RegistryValueKind.String);
            }

            Settings.FileAssocStamp = plan.Stamp;
            Notify();
        }
        catch (Exception ex)
        {
            Debug.WriteLine($"File association registration failed: {ex.Message}");
        }
    }

    /// <summary>
    /// Remove everything EnsureRegistered wrote, so an uninstalled app stops appearing in the
    /// Explorer "Open with" menu. Never throws.
    /// </summary>
    internal static void Unregister()
    {
        try
        {
            // Same guard as EnsureRegistered: an empty ProcessPath would resolve
            // ApplicationKeyPath to the shared "Software\Classes\Applications\" key, and
            // deleting that tree would wipe every per-user Open With registration on the
            // machine, not just ours.
            if (string.IsNullOrEmpty(Environment.ProcessPath)) return;

            FileAssociationPlan plan = BuildPlan();

            // Clear the stamp before touching the registry: this whole method is wrapped in a
            // catch-all below, so a failure partway through the loop must leave the app looking
            // unregistered (stamp cleared) rather than fully registered - otherwise a later
            // EnsureRegistered() would skip the repair and the partial state would stick.
            Settings.FileAssocStamp = "";

            foreach (string extension in Image.SupportedFileTypes)
            {
                string progId = FileAssociationPlan.ProgIdFor(extension);

                // (a) the ProgID tree we own.
                Registry.CurrentUser.DeleteSubKeyTree(FileAssociationPlan.ProgIdKeyPath(extension), throwOnMissingSubKey: false);

                // (b) only our own entry in the shared OpenWithProgids list.
                using (RegistryKey openWith = Registry.CurrentUser.OpenSubKey(FileAssociationPlan.OpenWithProgidsKeyPath(extension), writable: true))
                {
                    openWith?.DeleteValue(progId, throwOnMissingValue: false);
                }

                // (c) clear the extension default only if the user made one of our now-dead
                // ProgIDs the default handler.
                using (RegistryKey extensionKey = Registry.CurrentUser.OpenSubKey(FileAssociationPlan.ExtensionKeyPath(extension), writable: true))
                {
                    if (string.Equals(extensionKey?.GetValue(null) as string, progId, StringComparison.OrdinalIgnoreCase))
                    {
                        extensionKey.DeleteValue(string.Empty, throwOnMissingValue: false);
                    }
                }
            }

            // Software\Classes\Applications\<basename> is a shared namespace keyed only by
            // executable file name - Windows itself populates it whenever a user picks an app
            // via "Open with > Choose another app > Browse". A foreign application that
            // happens to share our exe's basename could have registered there too, so only
            // delete the tree if its open command still points at our own executable.
            using (RegistryKey applicationKey = Registry.CurrentUser.OpenSubKey($@"{plan.ApplicationKeyPath}\shell\open\command"))
            {
                if (plan.CommandMatchesCurrentExe(applicationKey?.GetValue(null) as string))
                {
                    Registry.CurrentUser.DeleteSubKeyTree(plan.ApplicationKeyPath, throwOnMissingSubKey: false);
                }
            }

            Notify();
        }
        catch (Exception ex)
        {
            Debug.WriteLine($"File association cleanup failed: {ex.Message}");
        }
    }

    private static FileAssociationPlan BuildPlan()
    {
        return new FileAssociationPlan(Environment.ProcessPath ?? "", Image.SupportedFileTypes);
    }

    private static bool IsRegistered(FileAssociationPlan plan)
    {
        if (Settings.FileAssocStamp != plan.Stamp) return false;

        // Belt and braces: the stamp survives a registry cleaner that wiped Software\Classes,
        // so confirm the keys themselves are still there before skipping the write.
        using RegistryKey key = Registry.CurrentUser.OpenSubKey($@"{plan.ApplicationKeyPath}\shell\open\command");
        return plan.CommandMatchesCurrentExe(key?.GetValue(null) as string);
    }

    /// <summary>Tell Explorer to reload associations, no reboot and no explorer restart needed.</summary>
    private static void Notify()
    {
        SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, IntPtr.Zero, IntPtr.Zero);
    }
}
