using System;
using System.Threading.Tasks;

using Microsoft.UI;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;
using Microsoft.UI.Xaml.Printing;

using Windows.Foundation;
using Windows.Graphics.Printing;

using WinRT.Interop;

using XamlImage = Microsoft.UI.Xaml.Controls.Image;

namespace ImageViewer.Services;

/// <summary>
/// Isolates the WinUI 3 print plumbing: prints a single image, fit-to-page, through the
/// native Windows print dialog. The page element lives in a zero-size, transparent Canvas
/// kept in the visual tree (the print framework renders the element where it sits).
/// </summary>
internal sealed class PrintService
{
    // White margin kept around the image on the page (~0.5 inch at 96 DIP/inch).
    private const double PageMarginDip = 48;

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

        PrintImage = new XamlImage { Stretch = Stretch.Uniform, Margin = new Thickness(PageMarginDip) };
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
        Size pageSize = new(desc.ImageableRect.Width, desc.ImageableRect.Height);

        PrintPage.Width = pageSize.Width;
        PrintPage.Height = pageSize.Height;

        // Force the off-screen page to lay out at the printable size now. Without this the
        // print framework captures the image at its native bitmap size (the host Canvas
        // measures children unbounded), so it prints cropped instead of fit-to-page.
        PrintPage.Measure(pageSize);
        PrintPage.Arrange(new Rect(new Point(0, 0), pageSize));
        PrintPage.UpdateLayout();

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
