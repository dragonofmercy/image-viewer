using System;
using System.Net.Http;
using System.Net.Http.Json;
using System.Net.Sockets;
using System.Text.Json;
using System.Threading.Tasks;

namespace ImageViewer
{
    internal class Update
    {
        const string GITHUB_API_RELEASE_PATH = "https://api.github.com/repos/dragonofmercy/image-viewer/releases/latest";

        public static string GetCurrentVersion()
        {
            return System.Reflection.Assembly.GetExecutingAssembly().ImageRuntimeVersion;
        }

        public static async Task<string> GetRemoteVersion()
        {
            HttpClient client = new();
            JsonDocument response_json;
            HttpResponseMessage response;

            try
            {
                client.DefaultRequestHeaders.Add("User-Agent", "Image View Update Check");
                response = await client.GetAsync(GITHUB_API_RELEASE_PATH);
                client.Dispose();
            }
            catch(HttpRequestException)
            {
                if(client != null)
                {
                    client.Dispose();
                }

                return Culture.GetString("ABOUT_REMOTE_VERSION_ERROR_NO_INTERNET");
            }

            try
            {
                response_json = await response.Content.ReadFromJsonAsync<JsonDocument>();
                JsonElement response_root = response_json.RootElement;

                return response_root.GetProperty("tag_name").GetString();
            }
            catch(Exception)
            {
                return "0.0.0";
            }
        }
    }
}
