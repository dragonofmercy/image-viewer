using Microsoft.UI.Input;
using Microsoft.UI.Xaml.Controls;

using Windows.UI.Core;

namespace ImageViewer
{
    public class CustomGrid : Grid
    {
        public void SetCursor(CoreCursor cursor)
        {
            ProtectedCursor = InputCursor.CreateFromCoreCursor(cursor);
        }
    }
}
