namespace ImageViewer
{
    public class CustomGrid : Microsoft.UI.Xaml.Controls.Grid
    {
        public void SetCursor(Windows.UI.Core.CoreCursor cursor)
        {
            ProtectedCursor = Microsoft.UI.Input.InputCursor.CreateFromCoreCursor(cursor);
        }
    }
}
