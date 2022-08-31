using Microsoft.UI.Xaml;
using WinUIEx;

namespace ImageViewer
{
    public static class Startup
    {
        public static string[] Args;

        [global::System.Runtime.InteropServices.DllImport("Microsoft.ui.xaml.dll")]
        private static extern void XamlCheckProcessRequirements();

        [global::System.CodeDom.Compiler.GeneratedCodeAttribute("Microsoft.UI.Xaml.Markup.Compiler", " 1.0.0.0")]
        [global::System.Diagnostics.DebuggerNonUserCodeAttribute()]
        [global::System.STAThreadAttribute]
        static void Main(string[] args)
        {
            Args = args;

            XamlCheckProcessRequirements();

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
            Context context = new()
            {
                LaunchArgs = Startup.Args
            };

            this.InitializeComponent();
        }

        protected override void OnLaunched(LaunchActivatedEventArgs args)
        {
            MainWindow m_window = new MainWindow();
            WindowManager manager = WindowManager.Get(m_window);

            _ = new Culture();

            Context.Instance().Manager = manager;
            Context.Instance().MainWindow = m_window;
            Context.Instance().MainWindow.Activate();
            Context.Instance().LoadDefaultImage();

            m_window.SetWindowSize(1280, 768);
            m_window.CenterOnScreen();

            Context.Instance().Manager.MinWidth = 680;
            Context.Instance().Manager.MinHeight = 400;
        }
    }
}
