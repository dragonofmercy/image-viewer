using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;

using SixLabors.ImageSharp;
using SixLabors.ImageSharp.Memory;

using WinUIEx;

namespace ImageViewer
{
    public static class Startup
    {
        [global::System.CodeDom.Compiler.GeneratedCodeAttribute("Microsoft.UI.Xaml.Markup.Compiler", " 1.0.0.0")]
        [global::System.Diagnostics.DebuggerNonUserCodeAttribute()]
        [global::System.STAThreadAttribute]
        private static void Main(string[] args)
        {
            Context.Instance().LaunchArgs = args;

            global::WinRT.ComWrappersSupport.InitializeComWrappers();
            global::Microsoft.UI.Xaml.Application.Start((p) => {
                DispatcherQueueSynchronizationContext context = new global::Microsoft.UI.Dispatching.DispatcherQueueSynchronizationContext(global::Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread());
                global::System.Threading.SynchronizationContext.SetSynchronizationContext(context);
                _ = new App();
            });
        }
    }

    public partial class App : Application
    {
        public App()
        {
            Configuration.Default.MemoryAllocator = MemoryAllocator.Create(new MemoryAllocatorOptions(){ MaximumPoolSizeMegabytes = 32 });
            Configuration.Default.PreferContiguousImageBuffers = true;

            InitializeComponent();
            Culture.Init();
        }

        public static ElementTheme CurrentTheme
        {
            get
            {
                ElementTheme themeSettings = Settings.Theme;

                if(themeSettings == ElementTheme.Default)
                {
                    themeSettings = ThemeHelpers.GetAppTheme();
                }

                return themeSettings;
            }
        }

        protected override void OnLaunched(LaunchActivatedEventArgs args)
        {
            MainWindow mWindow = new(CurrentTheme);
            WindowManager manager = WindowManager.Get(mWindow);

            Context.Instance().Manager = manager;
            Context.Instance().MainWindow = mWindow;
            Context.Instance().NotificationsManger = new NotificationsManger();
            
            manager.MinWidth = 768;
            manager.MinHeight = 400;
            manager.PositionChanged += Window_PositionChanged;
            manager.WindowMessageReceived += Manager_WindowMessageReceived;
            mWindow.SizeChanged += Window_SizeChanged;

            if(Settings.AppPositionX == null && Settings.AppPositionY == null)
            {
                mWindow.SetWindowSize(Settings.AppSizeW, Settings.AppSizeH);
                mWindow.CenterOnScreen();
            }
            else
            {
                mWindow.MoveAndResize((double)Settings.AppPositionX, (double)Settings.AppPositionY, Settings.AppSizeW, Settings.AppSizeH);
            }

            if(Settings.WindowState == WindowState.Maximized)
            {
                mWindow.Maximize();
            }
            else
            {
                Settings.WindowState = WindowState.Normal;
                mWindow.Activate();
            }

            Context.Instance().CheckUpdate();
            Context.Instance().LoadDefaultImage();
        }

        private void Manager_WindowMessageReceived(object sender, WinUIEx.Messaging.WindowMessageEventArgs e)
        {
            if(e.Message.MessageId != 0x0112) return; // WM_SYSCOMMAND

            switch(e.Message.WParam)
            {
                case 0xF000: // SC_SIZE
                case 0xF010: // SC_MOVE
                case 0xF120: // SC_RESTORE
                case 0xF122: // SC_RESTORE (dbl click)
                    Settings.WindowState = WindowState.Normal;
                    break;
                case 0xF030: // SC_MAXIMIZE
                case 0xF032: // SC_MAXIMIZE (dbl click)
                    Settings.WindowState = WindowState.Maximized;
                    break;
                case 0xF020: // SC_MINIMIZE
                    Settings.WindowState = WindowState.Minimized;
                    break;
            }
        }

        private void Window_SizeChanged(object sender, WindowSizeChangedEventArgs args)
        {
            if(Settings.WindowState != WindowState.Normal) return;

            Settings.AppSizeH = (uint)args.Size.Height;
            Settings.AppSizeW = (uint)args.Size.Width;
        }

        private void Window_PositionChanged(object sender, Windows.Graphics.PointInt32 e)
        {
            if(Settings.WindowState != WindowState.Normal) return;

            Settings.AppPositionX = e.X;
            Settings.AppPositionY = e.Y;
        }
    }
}
