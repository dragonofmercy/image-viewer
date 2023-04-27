using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls.Primitives;

namespace ImageViewer.Controls;

public partial class SettingsCard : ButtonBase
{
    public SettingsCard()
    {
        DefaultStyleKey = typeof(SettingsCard);
    }
    
    protected override void OnApplyTemplate()
    {
        base.OnApplyTemplate();
        OnDescriptionChanged();
    }
    
    private void OnDescriptionChanged()
    {
        if (GetTemplateChild("DescriptionPresenter") is FrameworkElement descriptionPresenter)
        {
            descriptionPresenter.Visibility = Description != null ? Visibility.Visible : Visibility.Collapsed;
        }
    }
}