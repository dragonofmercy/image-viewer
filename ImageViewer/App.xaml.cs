using Microsoft.UI.Xaml;
using WinUIEx;

namespace ImageViewer
{
    public static class Startup
    {
        [global::System.CodeDom.Compiler.GeneratedCodeAttribute("Microsoft.UI.Xaml.Markup.Compiler", " 1.0.0.0")]
        [global::System.Diagnostics.DebuggerNonUserCodeAttribute()]
        [global::System.STAThreadAttribute]
        static void Main(string[] args)
        {
            Context.Instance().LaunchArgs = args;

            global::WinRT.ComWrappersSupport.InitializeComWrappers();
            global::Microsoft.UI.Xaml.Application.Start((p) => {
                var context = new global::Microsoft.UI.Dispatching.DispatcherQueueSynchronizationContext(global::Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread());
                global::System.Threading.SynchronizationContext.SetSynchronizationContext(context);
                new App();
            });
        }
    }

    public partial class App : Application
    {
        public App()
        {
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

            switch(Settings.WindowState)
            {
                case WindowState.Maximized:
                    mWindow.Maximize();
                    break;

                default:
                    Settings.WindowState = WindowState.Normal;
                    mWindow.Activate();
                    break;
            }

            Context.Instance().LoadDefaultImage();
        }

        private void Manager_WindowMessageReceived(object sender, WinUIEx.Messaging.WindowMessageEventArgs e)
        {
            if(e.Message.MessageId == 0x0112) // WM_SYSCOMMAND
            {
                switch(e.Message.WParam)
                {
                    case 0xF120: // Restore event - SC_RESTORE from Winuser.h
                        Settings.WindowState = WindowState.Normal;
                        break;

                    case 0xF030: // Maximize event - SC_MAXIMIZE from Winuser.h
                        Settings.WindowState = WindowState.Maximized;
                        break;

                    case 0XF020: // Minimize event - SC_MINIMIZE from Winuser.h
                        Settings.WindowState = WindowState.Minimized;
                        break;
                }
            }
        }

        private void Window_SizeChanged(object sender, WindowSizeChangedEventArgs args)
        {
            if(Settings.WindowState == WindowState.Normal)
            {
                Settings.AppSizeH = (uint)args.Size.Height;
                Settings.AppSizeW = (uint)args.Size.Width;
            }
        }

        private void Window_PositionChanged(object sender, Windows.Graphics.PointInt32 e)
        {
            if(Settings.WindowState == WindowState.Normal)
            {
                Settings.AppPositionX = e.X;
                Settings.AppPositionY = e.Y;
            }
        }
    }
}
