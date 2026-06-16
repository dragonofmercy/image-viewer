using System.Diagnostics;
using System.Reflection;

namespace ImageViewer.Utilities;

/// <summary>
/// Product identity read from the executing assembly's version info.
/// </summary>
internal static class AppInfo
{
    public static string ProductName => FileVersionInfo.GetVersionInfo(Assembly.GetExecutingAssembly().Location).ProductName;

    public static string ProductVersion => FileVersionInfo.GetVersionInfo(Assembly.GetExecutingAssembly().Location).ProductVersion;
}
