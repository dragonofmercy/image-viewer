using System;
using System.Diagnostics;
using System.IO;

namespace ImageViewer.Helpers;

internal static class LegacyCleanup
{
    public static void Run()
    {
        // The old shortcut at %StartMenu%\Programs\Image Viewer.lnk shared its name
        // with the one Velopack now creates (--packTitle "Image Viewer"), so Velopack
        // has already overwritten it during install. Do not touch it here, otherwise
        // we delete the fresh Velopack shortcut on first run.
        string localAppData = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
        string legacy = Path.Combine(localAppData, "Dragon Industries");

        try { Directory.Delete(legacy, true); }
        catch (Exception ex) { Debug.WriteLine($"Legacy cleanup: {ex.Message}"); }
    }
}
