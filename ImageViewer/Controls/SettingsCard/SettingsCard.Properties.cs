using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;

namespace ImageViewer.Controls;

public partial class SettingsCard : ButtonBase
{
    public static readonly DependencyProperty LabelProperty = DependencyProperty.Register(nameof(Label), typeof(string), typeof(SettingsCard), new PropertyMetadata(default(string)));
    public static readonly DependencyProperty DescriptionProperty = DependencyProperty.Register(nameof(Description), typeof(string), typeof(SettingsCard), new PropertyMetadata(default(string)));
    public static readonly DependencyProperty IconProperty  = DependencyProperty.Register(nameof(Icon), typeof(IconElement), typeof(SettingsCard), new PropertyMetadata(defaultValue: null, (d, _) => ((SettingsCard)d).OnDescriptionPropertyChanged()));

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
    
    public IconElement Icon
    {
        get => (IconElement)GetValue(IconProperty);
        set => SetValue(IconProperty, value);
    }
    
    protected virtual void OnDescriptionPropertyChanged()
    {
        OnDescriptionChanged();
    }
}