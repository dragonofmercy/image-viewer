using Microsoft.UI.Input;
using Microsoft.UI.Xaml.Controls;

using Windows.UI.Core;

namespace ImageViewer.Controls
{
    public class CursorGrid : Grid
    {
        public void SetCursor(CoreCursor cursor)
        {
            ProtectedCursor = InputCursor.CreateFromCoreCursor(cursor);
        }
    }
}
