using System;
using System.Threading.Tasks;

using Microsoft.UI;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;
using Microsoft.UI.Xaml.Printing;

using Windows.Graphics.Printing;

using WinRT.Interop;

using XamlImage = Microsoft.UI.Xaml.Controls.Image;

namespace ImageViewer.Helpers;

/// <summary>
/// Isolates the WinUI 3 print plumbing: prints a single image, fit-to-page, through the
/// native Windows print dialog. The page element lives in a zero-size, transparent Canvas
/// kept in the visual tree (the print framework renders the element where it sits).
/// </summary>
internal sealed class PrintService
{
    private readonly IntPtr Hwnd;

    private readonly Grid PrintPage;
    private readonly XamlImage PrintImage;

    private PrintManager PrintManager;
    private PrintDocument PrintDocument;
    private IPrintDocumentSource PrintDocumentSource;
    private bool Registered;
    private string JobName = "Image";

    internal PrintService(Window window, Panel host)
    {
        Hwnd = WindowNative.GetWindowHandle(window);

        PrintImage = new XamlImage { Stretch = Stretch.Uniform };
        PrintPage = new Grid
        {
            Background = new SolidColorBrush(Colors.White)
        };
        PrintPage.Children.Add(PrintImage);

        // Off-screen, zero-footprint host. Canvas does not clip overflow, so PrintPage
        // still renders at its explicit (printable-area) size when the framework asks.
        Canvas offscreen = new() { Width = 0, Height = 0, Opacity = 0, IsHitTestVisible = false };
        offscreen.Children.Add(PrintPage);
        host.Children.Add(offscreen);
    }

    internal async Task PrintAsync(WriteableBitmap bitmap, string jobName)
    {
        JobName = string.IsNullOrEmpty(jobName) ? "Image" : jobName;
        PrintImage.Source = bitmap;

        EnsureRegistered();

        await PrintManagerInterop.ShowPrintUIForWindowAsync(Hwnd);
    }

    private void EnsureRegistered()
    {
        if(Registered) return;

        PrintDocument = new PrintDocument();
        PrintDocumentSource = PrintDocument.DocumentSource;
        PrintDocument.Paginate += OnPaginate;
        PrintDocument.GetPreviewPage += OnGetPreviewPage;
        PrintDocument.AddPages += OnAddPages;

        PrintManager = PrintManagerInterop.GetForWindow(Hwnd);
        PrintManager.PrintTaskRequested += OnPrintTaskRequested;

        Registered = true;
    }

    private void OnPaginate(object sender, PaginateEventArgs e)
    {
        // Size the page to the printable area; Stretch.Uniform fits the image inside it.
        PrintPageDescription desc = e.PrintTaskOptions.GetPageDescription(0);
        PrintPage.Width = desc.ImageableRect.Width;
        PrintPage.Height = desc.ImageableRect.Height;

        PrintDocument.SetPreviewPageCount(1, PreviewPageCountType.Final);
    }

    private void OnGetPreviewPage(object sender, GetPreviewPageEventArgs e)
    {
        PrintDocument.SetPreviewPage(e.PageNumber, PrintPage);
    }

    private void OnAddPages(object sender, AddPagesEventArgs e)
    {
        PrintDocument.AddPage(PrintPage);
        PrintDocument.AddPagesComplete();
    }

    private void OnPrintTaskRequested(PrintManager sender, PrintTaskRequestedEventArgs args)
    {
        args.Request.CreatePrintTask(JobName, request => request.SetSource(PrintDocumentSource));
    }
}
