using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace ImageViewer.Controls;

public partial class SettingsGroup : ItemsControl
{
    public SettingsGroup()
    {
        DefaultStyleKey = typeof(SettingsGroup);
    }
    
    protected override void OnApplyTemplate()
    {
        base.OnApplyTemplate();
        OnTitleChanged();
    }
    
    private void OnTitleChanged()
    {
        if(GetTemplateChild("TitlePresenter") is FrameworkElement titlePresenter)
        {
            titlePresenter.Visibility = Title != null ? Visibility.Visible : Visibility.Collapsed;
        }
    }
}