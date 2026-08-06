using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace ImageViewer.Controls;

public class SettingsGroup : ItemsControl
{
    public static readonly DependencyProperty TitleProperty = DependencyProperty.Register(nameof(Title), typeof(string), typeof(SettingsGroup), new PropertyMetadata(defaultValue: null, (d, _) => ((SettingsGroup)d).OnTitleChanged()));

    public SettingsGroup()
    {
        DefaultStyleKey = typeof(SettingsGroup);
    }

    public string Title
    {
        get => (string)GetValue(TitleProperty);
        set => SetValue(TitleProperty, value);
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
