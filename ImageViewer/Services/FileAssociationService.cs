using System;
using System.Diagnostics;
using System.Runtime.InteropServices;

using Microsoft.Win32;

using ImageViewer.Helpers;
using ImageViewer.Wrapper;

namespace ImageViewer.Services;

/// <summary>
/// Registers the app as an "Open with" handler for every supported image type, under
/// HKEY_CURRENT_USER only (no elevation, no per-machine state). Writes OpenWithProgids and
/// an Applications block, never the default handler of an extension: Windows keeps asking
/// the user which application to use.
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
    /// Write the association keys unless they are already in place for this executable. On
    /// every launch after the first this costs two registry reads and no write. Never throws:
    /// a failed association must not break startup.
    /// </summary>
    internal static void EnsureRegistered()
    {
        try
        {
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
                    if (extensionKey?.GetValue(null) as string == progId)
                    {
                        extensionKey.DeleteValue(string.Empty, throwOnMissingValue: false);
                    }
                }
            }

            Registry.CurrentUser.DeleteSubKeyTree(plan.ApplicationKeyPath, throwOnMissingSubKey: false);
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
