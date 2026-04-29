using System;
using System.Diagnostics;
using System.IO;

namespace ImageViewer.Helpers;

internal static class LegacyCleanup
{
    public static void Run()
    {
        string localAppData = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
        string legacy = Path.Combine(localAppData, "Dragon Industries");
        string shortcut = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.StartMenu),
            "Programs", "Image Viewer.lnk");

        TrySwallow(() => { if (Directory.Exists(legacy)) Directory.Delete(legacy, true); });
        TrySwallow(() => { if (File.Exists(shortcut)) File.Delete(shortcut); });
    }

    private static void TrySwallow(Action action)
    {
        try { action(); }
        catch (Exception ex) { Debug.WriteLine($"Legacy cleanup: {ex.Message}"); }
    }
}
