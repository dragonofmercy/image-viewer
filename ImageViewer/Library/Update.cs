using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Net.Http;
using System.Net.Http.Json;
using System.Text.Json;
using System.Threading.Tasks;
using System.IO;
using System.IO.Compression;
using Microsoft.UI.Xaml.Controls;

namespace ImageViewer
{
    internal class Update
    {
        const uint MAX_DOWNLOAD_ATTEMPTS = 3;
        const string GITHUB_API_RELEASE_PATH = "https://api.github.com/repos/dragonofmercy/image-viewer/releases/latest";
        public static JsonElement JsonCache;

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

        public static string GetRemoteVersion()
        {
            if(JsonCache.ValueKind == JsonValueKind.Object)
            {
                return JsonCache.GetProperty("name").GetString();
            }

            throw new KeyNotFoundException("Json cache date not loaded");
        }

        public static string GetRemoteFileUrl()
        {
            try
            {
                return JsonCache.GetProperty("assets")[0].GetProperty("browser_download_url").GetString();
            }
            catch(Exception e)
            {
                Debug.Write(e.Message);
                throw new KeyNotFoundException("Download file not found");
            }
        }

        public static async Task ApplyUpdate()
        {
            string tempDirectory = Path.Combine(Path.GetTempPath(), "imageviewer_installer");

            if(Directory.Exists(tempDirectory))
            {
                Directory.Delete(tempDirectory, true);
            }

            Directory.CreateDirectory(tempDirectory);

            HttpClient httpClient = new();
            httpClient.DefaultRequestHeaders.Add("User-Agent", "Image View Update Downloader");
            
            bool downloadSuccess = true;

            for(uint i = 0; i < MAX_DOWNLOAD_ATTEMPTS; i++)
            {
                try
                {
                    Stream s = await httpClient.GetStreamAsync(GetRemoteFileUrl());
                    FileStream fs = new(Path.Combine(tempDirectory, "install.zip"), FileMode.CreateNew);
                    
                    await s.CopyToAsync(fs);

                    fs.Dispose();
                    s.Dispose();

                    downloadSuccess = true;

                    break;
                }
                catch(Exception)
                {

                }
            }

            httpClient.Dispose();

            if(downloadSuccess)
            {
                string installSourcesDirectory = Path.Combine(tempDirectory, "install");
                string installScriptPath = Path.Combine(installSourcesDirectory, "installer", "update.ps1");

                if(Directory.Exists(installSourcesDirectory))
                {
                    Directory.Delete(installSourcesDirectory, true);
                }

                ZipFile.ExtractToDirectory(Path.Combine(tempDirectory, "install.zip"), installSourcesDirectory);

                if(File.Exists(installScriptPath))
                {
                    ProcessStartInfo pStartInfo = new()
                    {
                        FileName = @"powershell.exe",
                        Arguments = string.Concat("& '", installScriptPath, "' '", installSourcesDirectory, "' '", Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Dragon Industries", "Image Viewer"), "'"),
                        WindowStyle = ProcessWindowStyle.Hidden,
                    };

                    Process process = new()
                    {
                        StartInfo = pStartInfo
                    };

                    process.Start();
                    Environment.Exit(0);
                }
            }   
        }
    }
}
