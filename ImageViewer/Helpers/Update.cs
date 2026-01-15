using System;
using System.Collections.Generic;
using System.Net.Http;
using System.Net.Http.Json;
using System.Text.Json;
using System.Threading.Tasks;
using System.IO;
using System.Text.RegularExpressions;
using System.Diagnostics;

namespace ImageViewer.Helpers;

internal class Update
{
    private const uint MAX_DOWNLOAD_ATTEMPTS = 3;
    private const string GITHUB_API_RELEASE_PATH = "https://api.github.com/repos/dragonofmercy/image-viewer/releases/latest";
    public static JsonElement JsonCache;
    public static bool HasUpdate;

    public static async Task GetRemoteData()
    {
        HttpClient httpClient = new();
        httpClient.DefaultRequestHeaders.Add("User-Agent", "Image View Update Check");

        HttpResponseMessage responseMessage = await httpClient.GetAsync(GITHUB_API_RELEASE_PATH);
        JsonDocument responseJson = await responseMessage.Content.ReadFromJsonAsync<JsonDocument>();

        JsonCache = responseJson.RootElement;
        responseMessage.Dispose();

        httpClient.Dispose();
    }

    public static async Task<bool> CheckNewVersionAsync()
    {
        await GetRemoteData();

        string remoteVersion = GetRemoteVersion();

        DateTime dateTimeNow = DateTime.Now;
        Settings.LastUpdateCheck = dateTimeNow.ToString("yyyy-MM-dd HH:mm:ss");

        if(IsNewerVersion(remoteVersion, Context.GetProductVersion()))
        {
            HasUpdate = true;
            return true;
        }

        HasUpdate = false;
        return false;
    }

    /// <summary>
    /// Compare two semantic versions (supports formats like "1.2.3", "0.1.10-beta", etc.)
    /// </summary>
    /// <param name="remoteVersion">Remote version string</param>
    /// <param name="currentVersion">Current version string</param>
    /// <returns>True if remote version is newer than current version</returns>
    private static bool IsNewerVersion(string remoteVersion, string currentVersion)
    {
        if (string.IsNullOrEmpty(remoteVersion) || string.IsNullOrEmpty(currentVersion))
        {
            return false;
        }

        // Remove prefixes like "v" if present
        remoteVersion = remoteVersion.TrimStart('v', 'V');
        currentVersion = currentVersion.TrimStart('v', 'V');

        // Extract version numbers (remove suffixes like "-beta", "-alpha", etc.)
        string remoteVersionNumber = ExtractVersionNumber(remoteVersion);
        string currentVersionNumber = ExtractVersionNumber(currentVersion);

        try
        {
            Version remote = new(remoteVersionNumber);
            Version current = new(currentVersionNumber);

            int comparison = remote.CompareTo(current);

            // If versions are different, return the comparison result
            if (comparison != 0)
            {
                return comparison > 0;
            }

            // If version numbers are equal, check pre-release tags
            string remoteSuffix = GetVersionSuffix(remoteVersion);
            string currentSuffix = GetVersionSuffix(currentVersion);

            // No suffix (stable) is newer than any pre-release
            if (string.IsNullOrEmpty(remoteSuffix) && !string.IsNullOrEmpty(currentSuffix))
            {
                return true;
            }

            if (!string.IsNullOrEmpty(remoteSuffix) && string.IsNullOrEmpty(currentSuffix))
            {
                return false;
            }

            // Both have suffixes, compare lexicographically
            return string.Compare(remoteSuffix, currentSuffix, StringComparison.OrdinalIgnoreCase) > 0;
        }
        catch (Exception)
        {
            // Fallback to string comparison if version parsing fails
            return string.Compare(remoteVersion, currentVersion, StringComparison.OrdinalIgnoreCase) > 0;
        }
    }

    /// <summary>
    /// Extract version number from version string (removes pre-release suffix)
    /// </summary>
    private static string ExtractVersionNumber(string version)
    {
        int dashIndex = version.IndexOf('-');
        return dashIndex > 0 ? version.Substring(0, dashIndex) : version;
    }

    /// <summary>
    /// Get version suffix (pre-release tag like "beta", "alpha", etc.)
    /// </summary>
    private static string GetVersionSuffix(string version)
    {
        int dashIndex = version.IndexOf('-');
        return dashIndex > 0 ? version.Substring(dashIndex + 1) : string.Empty;
    }

    public static string GetRemoteVersion()
    {
        if(JsonCache.ValueKind == JsonValueKind.Object)
        {
            return JsonCache.GetProperty("name").GetString();
        }

        throw new KeyNotFoundException("Json cache date not loaded");
    }

    public static async Task ApplyUpdate()
    {
        string tempDirectory = Path.GetTempPath();

        HttpClient httpClient = new();
        httpClient.DefaultRequestHeaders.Add("User-Agent", "Image Viewer Updater");

        string downloadUri = "";

        try
        {
            for(int i = 0; i < JsonCache.GetProperty("assets").GetArrayLength(); i++)
            {
                string tmp = JsonCache.GetProperty("assets")[i].GetProperty("browser_download_url").GetString();
                Regex reg = new("ImageViewer.Updater.exe$", RegexOptions.IgnoreCase);

                if (!reg.IsMatch(tmp)) continue;
                downloadUri = tmp;
                break;
            }

            if(string.IsNullOrEmpty(downloadUri))
            {
                throw new Exception(Culture.GetString("ABOUT_UPDATE_INFO_ERROR_KEY_NOT_FOUND"));
            }
        }
        catch(Exception)
        {
            throw new Exception(Culture.GetString("ABOUT_UPDATE_INFO_ERROR_KEY_NOT_FOUND"));
        }

        bool downloadSuccess = false;
        string filename = Path.Combine(tempDirectory, "ImageViewer.Updater.exe");

        for(uint i = 0; i < MAX_DOWNLOAD_ATTEMPTS; i++)
        {
            try
            {
                if(File.Exists(filename))
                {
                    File.Delete(filename);
                }

                Stream s = await httpClient.GetStreamAsync(downloadUri);
                FileStream fs = new(filename, FileMode.CreateNew);

                await s.CopyToAsync(fs);
                await fs.DisposeAsync();
                await s.DisposeAsync();

                downloadSuccess = true;

                break;
            }
            catch(Exception)
            {
                // ignored
            }
        }

        httpClient.Dispose();

        if(downloadSuccess)
        {
            ProcessStartInfo pStartInfo = new()
            {
                FileName = filename,
                UseShellExecute = true
            };

            Process process = new()
            {
                StartInfo = pStartInfo
            };

            process.Start();
            Environment.Exit(0);
        }
        else
        {
            throw new Exception(Culture.GetString("ABOUT_UPDATE_INFO_ERROR_KEY_NOT_FOUND"));
        }
    }
}