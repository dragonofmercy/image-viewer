using System;
using System.Collections.Generic;
using System.Net.Http;
using System.Net.Http.Json;
using System.Text.Json;
using System.Threading.Tasks;
using System.IO;
using System.Text.RegularExpressions;
using System.Diagnostics;

namespace ImageViewer
{
    internal class Update
    {
        private const uint MAX_DOWNLOAD_ATTEMPTS = 3;
        private const string GITHUB_API_RELEASE_PATH = "https://api.github.com/repos/dragonofmercy/image-viewer/releases/latest";
        public static JsonElement JsonCache;
        public static bool HasUpdate = false;

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

            if(string.Compare(remoteVersion, Context.GetProductVersion(), StringComparison.InvariantCulture) > 0)
            {
                HasUpdate = true;
                return true;
            }

            HasUpdate = false;
            return false;
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

                    if(!reg.IsMatch(tmp)) continue;
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

            for(uint i = 0; i < MAX_DOWNLOAD_ATTEMPTS; i++)
            {
                try
                {
                    Stream s = await httpClient.GetStreamAsync(downloadUri);
                    FileStream fs = new(Path.Combine(tempDirectory, "imageviewer.update.exe"), FileMode.CreateNew);

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
                    FileName = Path.Combine(tempDirectory, "imageviewer.update.exe"),
                    UseShellExecute = true,
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
}