using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls.Primitives;

namespace ImageViewer.Controls;

public partial class SettingsCard : ButtonBase
{
    public static readonly DependencyProperty LabelProperty = DependencyProperty.Register(nameof(Label), typeof(string), typeof(SettingsCard), new PropertyMetadata(default(string)));
    public static readonly DependencyProperty DescriptionProperty = DependencyProperty.Register(nameof(Description), typeof(string), typeof(SettingsCard), new PropertyMetadata(default(string), (d, _) => ((SettingsCard)d).OnDescriptionPropertyChanged()));
    public static readonly DependencyProperty IconProperty  = DependencyProperty.Register(nameof(Icon), typeof(FrameworkElement), typeof(SettingsCard), new PropertyMetadata(defaultValue: null, (d, _) => ((SettingsCard)d).OnIconPropertyChanged()));

    public string Label
    {
        get => (string)GetValue(LabelProperty);
        set => SetValue(LabelProperty, value);
    }

    public string Description
    {
        get => (string)GetValue(DescriptionProperty);
        set => SetValue(DescriptionProperty, value);
    }
    
    public FrameworkElement Icon
    {
        get => (FrameworkElement)GetValue(IconProperty);
        set => SetValue(IconProperty, value);
    }
    
    protected virtual void OnDescriptionPropertyChanged()
    {
        OnDescriptionChanged();
    }
    
    protected virtual void OnIconPropertyChanged()
    {
        OnIconChanged();
    }
}