using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace ImageViewer.Controls;

public partial class SettingsGroup : ItemsControl
{
    public static readonly DependencyProperty TitleProperty = DependencyProperty.Register(nameof(Title), typeof(string), typeof(SettingsGroup), new PropertyMetadata(defaultValue: null, (d, _) => ((SettingsGroup)d).OnTitlePropertyChanged()));
    
    public string Title
    {
        get => (string)GetValue(TitleProperty);
        set => SetValue(TitleProperty, value);
    }
    
    protected virtual void OnTitlePropertyChanged()
    {
        OnTitleChanged();
    }
}