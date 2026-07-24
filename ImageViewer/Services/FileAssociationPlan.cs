using System;
using System.Collections.Generic;
using System.IO;

namespace ImageViewer.Services;

/// <summary>
/// One registry value to write under HKEY_CURRENT_USER. A null ValueName targets the key's
/// (Default) value.
/// </summary>
internal readonly record struct RegistryEntry(string KeyPath, string ValueName, string Value);

/// <summary>
/// Pure description of the Windows file-association layout for a given executable and set of
/// extensions. No I/O: it turns (exe, extensions) into the exact values to write and the key
/// paths to clean up, so the layout is unit-testable without touching the real registry. All
/// key paths are relative to HKEY_CURRENT_USER.
///
/// The app only ever proposes itself through OpenWithProgids. It never writes the (Default)
/// value of an extension key, so it never steals another application's default handler -
/// Windows keeps asking the user which application to open an image with.
/// </summary>
internal sealed class FileAssociationPlan
{
    // Bump whenever the shape of the keys themselves changes (new key, renamed value, changed
    // value format). The stamp stored in the settings key stops matching and every install
    // re-registers once. Adding or removing a supported extension does NOT need a bump - the
    // extension set is already part of the stamp (see Stamp below).
    internal const int LAYOUT_VERSION = 1;

    private static readonly Dictionary<string, string> TypeLabels = new(StringComparer.OrdinalIgnoreCase)
    {
        { ".jpg", "JPEG image" },
        { ".jpeg", "JPEG image" },
        { ".bmp", "Bitmap image" },
        { ".png", "PNG image" },
        { ".gif", "GIF image" },
        { ".tif", "TIFF image" },
        { ".tiff", "TIFF image" },
        { ".tga", "Targa image" },
        { ".ico", "Icon" },
        { ".webp", "WebP image" },
        { ".svg", "SVG image" }
    };

    private readonly string ExePath;
    private readonly string[] Extensions;

    internal FileAssociationPlan(string exePath, string[] extensions)
    {
        ExePath = exePath;
        Extensions = extensions;
    }

    internal string ExpectedCommand => $"\"{ExePath}\" \"%1\"";

    internal string IconValue => $"{ExePath},0";

    internal string ApplicationKeyPath => $@"Software\Classes\Applications\{Path.GetFileName(ExePath)}";

    /// <summary>
    /// Layout version + executable path + extension set. EnsureRegistered re-registers when it
    /// moves - including when the extension set changes, so a newly supported format gets
    /// registered even if LAYOUT_VERSION was not bumped.
    /// </summary>
    internal string Stamp => $"{LAYOUT_VERSION}|{ExePath}|{string.Join(";", Extensions)}";

    internal static string ProgIdFor(string extension) => "ImageViewer" + extension.ToLowerInvariant();

    internal static string ExtensionKeyPath(string extension) => $@"Software\Classes\{extension.ToLowerInvariant()}";

    internal static string ProgIdKeyPath(string extension) => $@"Software\Classes\{ProgIdFor(extension)}";

    internal static string OpenWithProgidsKeyPath(string extension) => $@"{ExtensionKeyPath(extension)}\OpenWithProgids";

    internal static string LabelFor(string extension) => TypeLabels.TryGetValue(extension, out string label) ? label : "Image file";

    internal IReadOnlyList<RegistryEntry> EntriesToWrite()
    {
        List<RegistryEntry> entries = [];

        foreach (string extension in Extensions)
        {
            string progIdKey = ProgIdKeyPath(extension);

            entries.Add(new RegistryEntry(progIdKey, null, LabelFor(extension)));
            entries.Add(new RegistryEntry($@"{progIdKey}\DefaultIcon", null, IconValue));
            entries.Add(new RegistryEntry($@"{progIdKey}\shell\open\command", null, ExpectedCommand));
            entries.Add(new RegistryEntry(OpenWithProgidsKeyPath(extension), ProgIdFor(extension), ""));
        }

        // The Applications block is what puts the app in "Open with > Choose another app".
        entries.Add(new RegistryEntry(ApplicationKeyPath, "FriendlyAppName", "Image Viewer"));
        entries.Add(new RegistryEntry($@"{ApplicationKeyPath}\DefaultIcon", null, IconValue));
        entries.Add(new RegistryEntry($@"{ApplicationKeyPath}\shell\open\command", null, ExpectedCommand));

        foreach (string extension in Extensions)
        {
            entries.Add(new RegistryEntry($@"{ApplicationKeyPath}\SupportedTypes", extension.ToLowerInvariant(), ""));
        }

        return entries;
    }

    internal bool CommandMatchesCurrentExe(string readCommandValue)
    {
        return string.Equals(readCommandValue, ExpectedCommand, StringComparison.OrdinalIgnoreCase);
    }
}
