﻿using System;
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
        private const uint MAX_DOWNLOAD_ATTEMPTS = 3;
        private const string GITHUB_API_RELEASE_PATH = "https://api.github.com/repos/dragonofmercy/image-viewer/releases/latest";

        private const string APP_RUNTIME_SEARCH = "Microsoft.WindowsAppRuntime.1.6_6000.266.2241.0_x64";
        private const string APP_RUNTIME_DOWNLOAD = "https://aka.ms/windowsappsdk/1.6/1.6.240923002/windowsappruntimeinstall-x64.exe";

        private const string NET_RUNTIME_SEARCH = "Microsoft.WindowsDesktop.App 7.0.20";
        private const string NET_RUNTIME_DOWNLOAD = "https://download.visualstudio.microsoft.com/download/pr/08bbfe8f-812d-479f-803b-23ea0bffce47/c320e4b037f3e92ab7ea92c3d7ea3ca1/windowsdesktop-runtime-7.0.20-win-x64.exe";
        
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

        private async Task<bool> DownloadFile(string url, string savePath)
        {
            HttpClient httpClient = new HttpClient();
            httpClient.DefaultRequestHeaders.Add("User-Agent", "Image Viewer Updater");

            for(uint i = 0; i < MAX_DOWNLOAD_ATTEMPTS; i++)
            {
                try
                {
                    Application.DoEvents();

                    Stream s = await httpClient.GetStreamAsync(new Uri(url));
                    FileStream fs = new FileStream(savePath, FileMode.CreateNew);

                    await s.CopyToAsync(fs);

                    fs.Dispose();
                    s.Dispose();
                    httpClient.Dispose();

                    return true;
                }
                catch(Exception)
                {
                    // ignored
                }
            }

            httpClient.Dispose();
            return false;
        }

        public void CopyDirectory(DirectoryInfo source, DirectoryInfo target)
        {
            foreach(DirectoryInfo dir in source.GetDirectories())
            {
                CopyDirectory(dir, target.CreateSubdirectory(dir.Name));
            }
            
            foreach(FileInfo file in source.GetFiles())
            {
                Application.DoEvents();
                file.CopyTo(Path.Combine(target.FullName, file.Name));
            }
        }

        public void CheckAppRuntime()
        {
            TextInstallStatus.Text = "Checking Windows AppRuntime...";
            ProgressStatus.Value = 10;

            ProcessStartInfo psi = new ProcessStartInfo
            {
                FileName = "powershell",
                Arguments = "get-appxpackage *appruntime* | Select-String '" + APP_RUNTIME_SEARCH + "'",
                UseShellExecute = false,
                RedirectStandardOutput = true,
                CreateNoWindow = true
            };

            Process process = new Process
            {
                StartInfo = psi
            };

            process.Start();
            Application.DoEvents();
            string output = process.StandardOutput.ReadToEnd();
            process.Dispose();

            ProgressStatus.Value = 20;

            if(!output.Contains(APP_RUNTIME_SEARCH))
            {
                Application.DoEvents();
                InstallRuntime();
            }
            else
            {
                Application.DoEvents();
                CheckNet6();
            }
        }

        public async void InstallRuntime()
        {
            TextInstallStatus.Text = "Download Windows AppRuntime...";

            if(await DownloadFile(APP_RUNTIME_DOWNLOAD, Path.Combine(TempDirectory, "windowsappruntime.exe")))
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

                ProgressStatus.Value = 30;
                Application.DoEvents();
                CheckNet6();
            }
            else
            {
                MessageBox.Show("Cannot download Windows AppRuntime, please try again later...", "Install error", MessageBoxButtons.OK, MessageBoxIcon.Error);
                Environment.Exit(0);
            }
        }

        public void CheckNet6()
        {
            TextInstallStatus.Text = "Checking .NET Desktop Runtime...";

            ProcessStartInfo psi = new ProcessStartInfo
            {
                FileName = "powershell",
                Arguments = "dotnet --list-runtimes | Select-String '" + NET_RUNTIME_SEARCH + "'",
                UseShellExecute = false,
                RedirectStandardOutput = true,
                CreateNoWindow = true
            };

            Process process = new Process
            {
                StartInfo = psi
            };

            process.Start();
            Application.DoEvents();
            string output = process.StandardOutput.ReadToEnd();
            process.Dispose();

            ProgressStatus.Value = 40;

            if(!output.Contains(NET_RUNTIME_SEARCH))
            {
                Application.DoEvents();
                InstallNet6();
            }
            else
            {
                Application.DoEvents();
                Download();
            }
        }

        public async void InstallNet6()
        {
            TextInstallStatus.Text = "Downloading .NET Desktop Runtime...";

            if(await DownloadFile(NET_RUNTIME_DOWNLOAD, Path.Combine(TempDirectory, "windowsdesktopruntime.exe")))
            {
                TextInstallStatus.Text = "Installing .NET Desktop Runtime...";

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

                ProgressStatus.Value = 50;
                Download();
            }
            else
            {
                MessageBox.Show("Cannot download .NET 6.0 Desktop Runtime, please try again later...", "Install error", MessageBoxButtons.OK, MessageBoxIcon.Error);
                Environment.Exit(0);
            }
        }

        public async void Download()
        {
            TextInstallStatus.Text = "Downloading Image Viewer...";
            Application.DoEvents();

            HttpClient httpClient = new HttpClient();
            httpClient.DefaultRequestHeaders.Add("User-Agent", "Image Viewer Updater");

            string responseContent = "";

            try
            {
                HttpResponseMessage responseMessage = await httpClient.GetAsync(GITHUB_API_RELEASE_PATH);
                responseContent = await responseMessage.Content.ReadAsStringAsync();
                httpClient.Dispose();
            }
            catch(Exception)
            {
                httpClient.Dispose();

                MessageBox.Show("No internet access", "Install error", MessageBoxButtons.OK, MessageBoxIcon.Error);
                Environment.Exit(0);
            }

            Regex reg = new Regex("\"browser_download_url\":\"((\\\\\"|[^\"])*release.zip)\"", RegexOptions.IgnoreCase);
            MatchCollection matches = reg.Matches(responseContent);

            if(matches.Count == 1)
            {
                if(await DownloadFile(matches[0].Groups[1].Value, Path.Combine(TempDirectory, "release.zip")))
                {
                    TextInstallStatus.Text = "Extracting files...";
                    ProgressStatus.Value = 60;
                    Application.DoEvents();
                    Extract();
                }
                else
                {
                    MessageBox.Show("Cannot download remote version", "Install error", MessageBoxButtons.OK, MessageBoxIcon.Error);
                    Environment.Exit(0);
                }
            }
            else
            {
                MessageBox.Show("Cannot get remote package", "Install error", MessageBoxButtons.OK, MessageBoxIcon.Error);
                Environment.Exit(0);
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
            ProgressStatus.Value = 70;
            Application.DoEvents();
            Install();
        }

        public void Install()
        {
            try
            {
                if(Directory.Exists(InstallDestinationDirectory))
                {
                    Directory.Delete(InstallDestinationDirectory, true);
                }

                Directory.CreateDirectory(InstallDestinationDirectory);

                CopyDirectory(new DirectoryInfo(InstallSourcesDirectory), new DirectoryInfo(Path.Combine(InstallDestinationDirectory, "Image Viewer")));

                if(File.Exists(ShortcutFilePath))
                {
                    File.Delete(ShortcutFilePath);
                }

                IShellLink link = (IShellLink)new ShellLink();
                link.SetPath(Path.Combine(InstallDestinationDirectory, "Image Viewer", "ImageViewer.exe"));

                IPersistFile file = (IPersistFile)link;
                file.Save(ShortcutFilePath, false);

                
            }
            catch(Exception ex)
            {
                MessageBox.Show(ex.Message, "Install error", MessageBoxButtons.OK, MessageBoxIcon.Error);
                Environment.Exit(0);
            }

            TextInstallStatus.Text = "Cleaning up...";
            ProgressStatus.Value = 80;
            Application.DoEvents();
            Cleanup();
        }

        public void Cleanup()
        {
            ProgressStatus.Value = 100;
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
