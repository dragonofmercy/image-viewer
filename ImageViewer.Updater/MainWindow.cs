using System;
using System.IO;
using System.IO.Compression;
using System.Windows.Forms;
using System.Net.Http;
using System.Text.RegularExpressions;
using System.Reflection;
using System.Runtime.InteropServices.ComTypes;
using System.Diagnostics;
using System.Threading.Tasks;

namespace ImageViewer.Updater
{
    public partial class MainWindow : Form
    {
        const uint MAX_DOWNLOAD_ATTEMPTS = 3;
        const string GITHUB_API_RELEASE_PATH = "https://api.github.com/repos/dragonofmercy/image-viewer/releases/latest";
        const string RUNTIME_DOWNLOAD = "https://aka.ms/windowsappsdk/1.1/1.1.5/windowsappruntimeinstall-x64.exe";
        const string NET6_RUNTIME_DOWNLOAD = "https://download.visualstudio.microsoft.com/download/pr/fe8415d4-8a35-4af9-80a5-51306a96282d/05f9b2a1b4884238e69468e49b3a5453/windowsdesktop-runtime-6.0.9-win-x64.exe";

        private string TempDirectory;
        private string InstallSourcesDirectory;
        private string InstallDestinationDirectory;
        private string ShortcutFilePath;
        private string Callable;

        private readonly Timer TimerWait = new Timer();

        public MainWindow()
        {
            InitializeComponent();
        }

        private void MainWindow_Load(object sender, EventArgs e)
        {
            TempDirectory = Path.Combine(Path.GetTempPath(), "imageviewer_install");
            InstallSourcesDirectory = Path.Combine(TempDirectory, "update");
            InstallDestinationDirectory = Path.Combine(
                Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
                FileVersionInfo.GetVersionInfo(Assembly.GetEntryAssembly().Location).CompanyName
            );

            ShortcutFilePath = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.StartMenu), "Programs", "Image Viewer.lnk");

            if(Directory.Exists(TempDirectory))
            {
                Directory.Delete(TempDirectory, true);
            }

            Directory.CreateDirectory(TempDirectory);

            TimerWait.Tick += TimerWait_Tick;

            Callable = "CheckAppRuntime";
            TimerWait.Interval = 2000;
            TimerWait.Start();
        }

        private void TimerWait_Tick(object sender, EventArgs e)
        {
            (sender as Timer).Stop();

            Type t = GetType();
            MethodInfo mi = t.GetMethod(Callable);
            mi.Invoke(this, null);
        }

        private async Task<bool> DownloadFile(string Url, string SavePath)
        {
            HttpClient httpClient = new HttpClient();
            httpClient.DefaultRequestHeaders.Add("User-Agent", "Image Viewer Updater");

            for(uint i = 0; i < MAX_DOWNLOAD_ATTEMPTS; i++)
            {
                try
                {
                    Application.DoEvents();

                    Stream s = await httpClient.GetStreamAsync(new Uri(Url));
                    FileStream fs = new FileStream(SavePath, FileMode.CreateNew);

                    await s.CopyToAsync(fs);

                    fs.Dispose();
                    s.Dispose();
                    httpClient.Dispose();

                    return true;
                }
                catch(Exception)
                {
                }
            }

            httpClient.Dispose();
            return false;
        }

        public void CheckAppRuntime()
        {
            TextInstallStatus.Text = "Checking Windows AppRuntime...";

            ProcessStartInfo psi = new ProcessStartInfo
            {
                FileName = "powershell",
                Arguments = "get-appxpackage *appruntime* | Select-String 'WindowsAppRuntime.1.1_1005'",
                UseShellExecute = false,
                RedirectStandardOutput = true,
                CreateNoWindow = true
            };

            Process process = new Process
            {
                StartInfo = psi
            };

            process.Start();
            string output = process.StandardOutput.ReadToEnd();
            process.Dispose();

            if(string.IsNullOrEmpty(output))
            {
                Callable = "InstallRuntime";
                TimerWait.Interval = 1000;
                TimerWait.Start();
            }
            else
            {
                Callable = "CheckNet6";
                TimerWait.Interval = 1000;
                TimerWait.Start();
            }
        }

        public async void InstallRuntime()
        {
            TextInstallStatus.Text = "Download Windows AppRuntime...";

            if(await DownloadFile(RUNTIME_DOWNLOAD, Path.Combine(TempDirectory, "windowsappruntime.exe")))
            {
                TextInstallStatus.Text = "Installing Windows AppRuntime...";

                ProcessStartInfo psi = new ProcessStartInfo
                {
                    UseShellExecute = false,
                    FileName = Path.Combine(TempDirectory, "windowsappruntime.exe")
                };

                Process process = new Process
                {
                    StartInfo = psi
                };

                process.Start();

                while(!process.HasExited)
                {
                    Application.DoEvents();
                    System.Threading.Thread.Sleep(100);
                }

                process.Dispose();

                Callable = "CheckNet6";
                TimerWait.Interval = 1000;
                TimerWait.Start();
            }
            else
            {
                throw new Exception("Cannot download Windows AppRuntime, please try again later...");
            }
        }

        public void CheckNet6()
        {
            TextInstallStatus.Text = "Checking .NET 6.0 Desktop Runtime...";

            ProcessStartInfo psi = new ProcessStartInfo
            {
                FileName = "powershell",
                Arguments = "dotnet --list-runtimes | Select-String 'WindowsDesktop.App 6.0.9'",
                UseShellExecute = false,
                RedirectStandardOutput = true,
                CreateNoWindow = true
            };

            Process process = new Process
            {
                StartInfo = psi
            };

            process.Start();
            string output = process.StandardOutput.ReadToEnd();
            process.Dispose();

            if(string.IsNullOrEmpty(output) || output.StartsWith("dotnet : "))
            {
                Callable = "InstallNet6";
                TimerWait.Interval = 1000;
                TimerWait.Start();
            }
            else
            {
                Callable = "Download";
                TimerWait.Interval = 1000;
                TimerWait.Start();
            }
        }

        public async void InstallNet6()
        {
            TextInstallStatus.Text = "Downloading .NET 6.0 Desktop Runtime...";

            if(await DownloadFile(NET6_RUNTIME_DOWNLOAD, Path.Combine(TempDirectory, "windowsdesktopruntime.exe")))
            {
                TextInstallStatus.Text = "Installing .NET 6.0 Desktop Runtime...";

                ProcessStartInfo psi = new ProcessStartInfo
                {
                    UseShellExecute = false,
                    Arguments = "/install /quiet /norestart",
                    FileName = Path.Combine(TempDirectory, "windowsdesktopruntime.exe")
                };

                Process process = new Process
                {
                    StartInfo = psi
                };

                process.Start();

                while(!process.HasExited)
                {
                    Application.DoEvents();
                    System.Threading.Thread.Sleep(100);
                }

                process.Dispose();

                Callable = "Download";
                TimerWait.Interval = 1000;
                TimerWait.Start();
            }
            else
            {
                throw new Exception("Cannot download .NET 6.0 Desktop Runtime, please try again later...");
            }
        }

        public async void Download()
        {
            TextInstallStatus.Text = "Downloading Image Viewer...";

            HttpClient httpClient = new HttpClient();
            httpClient.DefaultRequestHeaders.Add("User-Agent", "Image Viewer Updater");

            string responseContent;

            try
            {
                HttpResponseMessage responseMessage = await httpClient.GetAsync(GITHUB_API_RELEASE_PATH);
                responseContent = await responseMessage.Content.ReadAsStringAsync();
                httpClient.Dispose();
            }
            catch(Exception)
            {
                httpClient.Dispose();
                throw new Exception("No internet access");
            }

            Regex reg = new Regex("\"browser_download_url\":\"((\\\\\"|[^\"])*release.zip)\"", RegexOptions.IgnoreCase);
            MatchCollection matches = reg.Matches(responseContent);

            if(matches.Count == 1)
            {
                if(await DownloadFile(matches[0].Groups[1].Value, Path.Combine(TempDirectory, "release.zip")))
                {
                    TextInstallStatus.Text = "Extracting files...";

                    Callable = "Extract";
                    TimerWait.Interval = 1000;
                    TimerWait.Start();
                }
                else
                {
                    throw new Exception("Cannot download remote version");
                }
            }
            else
            {
                throw new Exception("Cannot get remote package");
            }
        }

        public void Extract()
        {
            if(Directory.Exists(InstallSourcesDirectory))
            {
                Directory.Delete(InstallSourcesDirectory, true);
            }

            ZipFile.ExtractToDirectory(Path.Combine(TempDirectory, "release.zip"), InstallSourcesDirectory);

            TextInstallStatus.Text = "Installing files...";

            Callable = "Install";
            TimerWait.Interval = 1000;
            TimerWait.Start();
        }

        public void Install()
        {
            if(Directory.Exists(InstallDestinationDirectory))
            {
                Directory.Delete(InstallDestinationDirectory, true);
            }

            Directory.CreateDirectory(InstallDestinationDirectory);
            Directory.Move(InstallSourcesDirectory, Path.Combine(InstallDestinationDirectory, "Image Viewer"));

            if(File.Exists(ShortcutFilePath))
            {
                File.Delete(ShortcutFilePath);
            }

            IShellLink link = (IShellLink)new ShellLink();
            link.SetPath(Path.Combine(InstallDestinationDirectory, "Image Viewer", "ImageViewer.exe"));

            IPersistFile file = (IPersistFile)link;
            file.Save(ShortcutFilePath, false);

            TextInstallStatus.Text = "Cleaning up...";

            Callable = "Cleanup";
            TimerWait.Interval = 1000;
            TimerWait.Start();
        }

        public void Cleanup()
        {
            Directory.Delete(TempDirectory, true);

            ProcessStartInfo psi = new ProcessStartInfo
            {
                UseShellExecute = true,
                FileName = Path.Combine(InstallDestinationDirectory, "Image Viewer", "ImageViewer.exe")
            };

            Process process = new Process
            {
                StartInfo = psi
            };

            process.Start();
            Environment.Exit(0);
        }
    }
}
