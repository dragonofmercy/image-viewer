using System.Threading.Tasks;

using Microsoft.UI.Dispatching;

namespace ImageViewer.Helpers;

/// <summary>
/// Small UI-thread plumbing helpers.
/// </summary>
internal static class UIThread
{
    /// <summary>
    /// Yield once to the dispatcher queue so the in-flight keyboard event finishes routing
    /// before a native modal dialog (file picker, print UI) opens.
    ///
    /// WinUI 3 race: when a Ctrl+key KeyboardAccelerator opens a native modal dialog
    /// synchronously, the dialog grabs the keyboard before the framework processes the Ctrl
    /// KeyUp, leaving the input state with Ctrl "stuck down" until the user presses Ctrl
    /// again (e.g. paste then Ctrl+S, then the save picker name field swallows typing as
    /// shortcuts). Deferring the dialog out of the accelerator's handler lets the KeyUp be
    /// delivered first. No-op off the UI thread.
    /// </summary>
    internal static Task YieldAsync()
    {
        DispatcherQueue queue = DispatcherQueue.GetForCurrentThread();
        if (queue == null) return Task.CompletedTask;

        TaskCompletionSource completion = new();

        // Low priority so this runs after any pending input (the Ctrl KeyUp) is dispatched.
        if (!queue.TryEnqueue(DispatcherQueuePriority.Low, () => completion.SetResult()))
        {
            completion.SetResult();
        }

        return completion.Task;
    }
}
