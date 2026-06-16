using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Threading.Tasks;

using Microsoft.UI.Xaml;

using Windows.Storage;
using Windows.Storage.Pickers;
using WinRT.Interop;

using ImageViewer.Wrapper;

namespace ImageViewer.Helpers;

/// <summary>
/// Owns the image persistence mechanics: the Save As file picker, the per-format quality
/// policy, the actual encode, and the save error dialog. Holds only the owning window;
/// the document state (current path, navigation) stays in Context.
/// </summary>
internal sealed class SaveService
{
    private readonly MainWindow Window;

    internal SaveService(MainWindow window)
    {
        Window = window;
    }

    /// <summary>
    /// Normalize a file extension to its canonical save type: lower-case, jpeg -> jpg, tif -> tiff.
    /// </summary>
    internal static string NormalizeExtension(string ext)
    {
        string type = ext.ToLowerInvariant();
        if (type == ".jpeg") type = ".jpg";
        if (type == ".tif") type = ".tiff";
        return type;
    }

    /// <summary>
    /// Encode the image to the given path/type at the configured quality. Returns false and
    /// shows the localized error dialog on failure.
    /// </summary>
    internal async Task<bool> WriteAsync(Image image, string path, string type)
    {
        try
        {
            await image.Save(path, type, QualityForType(type));
            return true;
        }
        catch (Exception ex)
        {
            Debug.WriteLine($"Save failed: {ex.Message}");
            await ShowSaveErrorAsync();
            return false;
        }
    }

    /// <summary>
    /// Show the Save As picker. Returns the chosen path and normalized type, or null when the
    /// user cancels or picks an unsupported type.
    /// </summary>
    internal async Task<(string Path, string Type)?> PickSaveTargetAsync(string suggestedName)
    {
        FileSavePicker saveFilePicker = new()
        {
            SuggestedFileName = suggestedName
        };

        foreach (string fileType in Image.SaveFileTypes)
        {
            saveFilePicker.FileTypeChoices.Add(Culture.GetString("FOOTER_TOOLBAR_MENU_FILE_SAVE_FORMAT").Replace("{0}", fileType.Remove(0, 1).ToUpper()), new List<string>{ fileType });
        }

        InitializeWithWindow.Initialize(saveFilePicker, WindowNative.GetWindowHandle(Window));
        StorageFile outputFile = await saveFilePicker.PickSaveFileAsync();

        if (outputFile == null) return null;

        string type = outputFile.FileType.ToLowerInvariant();
        if (!Image.SaveFileTypes.Contains(type)) return null;

        return (outputFile.Path, type);
    }

    // JPEG and WebP re-encode at their configured quality; other formats ignore the parameter.
    private static int? QualityForType(string type)
    {
        return type switch
        {
            ".jpg" => Settings.JpegQuality,
            ".webp" => Settings.WebpQuality,
            _ => null
        };
    }

    private async Task ShowSaveErrorAsync()
    {
        Microsoft.UI.Xaml.Controls.ContentDialog errorDialog = new()
        {
            XamlRoot = Window.Content.XamlRoot,
            RequestedTheme = ((FrameworkElement)Window.Content).ActualTheme,
            Content = Culture.GetString("SYSTEM_SAVING_ERROR"),
            CloseButtonText = Culture.GetString("SYSTEM_OK")
        };

        await errorDialog.ShowAsync();
    }
}
