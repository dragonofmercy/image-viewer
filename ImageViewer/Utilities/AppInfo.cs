using System.Diagnostics;
using System.Reflection;

namespace ImageViewer.Utilities;

/// <summary>
/// Product identity read from the executing assembly's version info.
/// </summary>
internal static class AppInfo
{
    // The executable's version resource never changes at runtime, so read it once: ProductName
    // is reached on the navigation hot path through MainWindow.UpdateTitle.
    private static readonly FileVersionInfo Info = FileVersionInfo.GetVersionInfo(Assembly.GetExecutingAssembly().Location);

    public static string ProductName => Info.ProductName;

    public static string ProductVersion => Info.ProductVersion;
}
