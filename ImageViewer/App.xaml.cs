using System;
using ImageViewer.Helpers;

using Microsoft.UI.Dispatching;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;

using SixLabors.ImageSharp;
using SixLabors.ImageSharp.Memory;

using Velopack;

using WinUIEx;

namespace ImageViewer;

public static class Startup
{
    [global::System.CodeDom.Compiler.GeneratedCodeAttribute("Microsoft.UI.Xaml.Markup.Compiler", " 1.0.0.0")]
    [global::System.Diagnostics.DebuggerNonUserCodeAttribute]
    [global::System.STAThreadAttribute]
    private static void Main(string[] args)
    {
        VelopackApp.Build()
            .OnFirstRun(_ => Helpers.LegacyCleanup.Run())
            .Run();

        Context.Instance().LaunchArgs = args;

        global::WinRT.ComWrappersSupport.InitializeComWrappers();
        global::Microsoft.UI.Xaml.Application.Start(p => {
            DispatcherQueueSynchronizationContext context = new global::Microsoft.UI.Dispatching.DispatcherQueueSynchronizationContext(global::Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread());
            global::System.Threading.SynchronizationContext.SetSynchronizationContext(context);
            _ = new App();
        });
    }
}

public partial class App : Application
{
    // Window geometry is tracked in memory and flushed to the registry once on close,
    // because size/position events fire on every tick of a drag/resize
    private static WindowState TrackedWindowState;
    private static int? PendingPositionX;
    private static int? PendingPositionY;
    private static uint? PendingSizeW;
    private static uint? PendingSizeH;

    // True while the window is in fullscreen; suppresses geometry persistence so the
    // fullscreen bounds are never saved as the normal window size/position.
    public static bool IsFullScreen;

    public App()
    {
        long totalMemoryMb = GC.GetGCMemoryInfo().TotalAvailableMemoryBytes / (1024 * 1024);
        int poolSizeMb = (int)Math.Min(totalMemoryMb * 0.15, 512);

        Configuration.Default.MemoryAllocator = MemoryAllocator.Create(new MemoryAllocatorOptions
        {
            MaximumPoolSizeMegabytes = poolSizeMb,
            AllocationLimitMegabytes = poolSizeMb * 2 // Limite totale
        });
        Configuration.Default.PreferContiguousImageBuffers = true;


        InitializeComponent();
        Culture.Init();
    }

    public static void SaveWindowGeometry()
    {
        if(PendingPositionX.HasValue) Settings.AppPositionX = PendingPositionX;
        if(PendingPositionY.HasValue) Settings.AppPositionY = PendingPositionY;
        if(PendingSizeW.HasValue) Settings.AppSizeW = PendingSizeW.Value;
        if(PendingSizeH.HasValue) Settings.AppSizeH = PendingSizeH.Value;
    }

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        MainWindow mWindow = new(Settings.Theme);
        WindowManager manager = WindowManager.Get(mWindow);

        Context.Instance().MainWindow = mWindow;

        manager.MinWidth = 768;
        manager.MinHeight = 400;
        manager.PositionChanged += Window_PositionChanged;
        manager.WindowMessageReceived += Manager_WindowMessageReceived;
        mWindow.SizeChanged += Window_SizeChanged;

        bool positionRestored = false;

        if(Settings.AppPositionX != null && Settings.AppPositionY != null)
        {
            int posX = (int)Settings.AppPositionX;
            int posY = (int)Settings.AppPositionY;

            // Only restore a position that still intersects a connected display (monitor may have been unplugged)
            if(DisplayArea.GetFromRect(new Windows.Graphics.RectInt32(posX, posY, (int)Settings.AppSizeW, (int)Settings.AppSizeH), DisplayAreaFallback.None) != null)
            {
                mWindow.MoveAndResize(posX, posY, Settings.AppSizeW, Settings.AppSizeH);
                positionRestored = true;
            }
        }

        if(!positionRestored)
        {
            mWindow.SetWindowSize(Settings.AppSizeW, Settings.AppSizeH);
            mWindow.CenterOnScreen();
        }

        if(Settings.WindowState == WindowState.Maximized)
        {
            TrackedWindowState = WindowState.Maximized;
            mWindow.Maximize();
        }
        else
        {
            Settings.WindowState = WindowState.Normal;
            TrackedWindowState = WindowState.Normal;
            mWindow.Activate();
        }

        Context.Instance().LoadDefaultImage();

        // Defer non-essential startup work (toast registration + update check, which pulls in
        // the Velopack assemblies) until after the first frame so the window appears sooner.
        mWindow.DispatcherQueue.TryEnqueue(DispatcherQueuePriority.Low, () =>
        {
            Context context = Context.Instance();
            context.NotificationsManger = new NotificationsManger();
            context.CheckUpdate();
        });
    }

    private void Manager_WindowMessageReceived(object sender, WinUIEx.Messaging.WindowMessageEventArgs e)
    {
        if(e.Message.MessageId != 0x0112) return; // WM_SYSCOMMAND

        // The low 4 bits of WParam are used internally by the system and must be masked out
        switch((int) e.Message.WParam & 0xFFF0)
        {
            case 0xF000: // SC_SIZE
            case 0xF010: // SC_MOVE
            case 0xF120: // SC_RESTORE
                Settings.WindowState = TrackedWindowState = WindowState.Normal;
                break;
            case 0xF030: // SC_MAXIMIZE
                Settings.WindowState = TrackedWindowState = WindowState.Maximized;
                break;
            case 0xF020: // SC_MINIMIZE
                Settings.WindowState = TrackedWindowState = WindowState.Minimized;
                break;
        }
    }

    private void Window_SizeChanged(object sender, WindowSizeChangedEventArgs args)
    {
        if(IsFullScreen) return;
        if(TrackedWindowState != WindowState.Normal) return;

        PendingSizeH = (uint)args.Size.Height;
        PendingSizeW = (uint)args.Size.Width;
    }

    private void Window_PositionChanged(object sender, Windows.Graphics.PointInt32 e)
    {
        if(IsFullScreen) return;
        if(TrackedWindowState != WindowState.Normal) return;

        // Some minimize paths bypass WM_SYSCOMMAND (Win+D, Win+Down) and park the window at -32000
        if(e.X <= -32000 || e.Y <= -32000) return;

        PendingPositionX = e.X;
        PendingPositionY = e.Y;
    }
}