using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls.Primitives;

namespace ImageViewer.Controls;

public class SettingsCard : ButtonBase
{
    public static readonly DependencyProperty LabelProperty = DependencyProperty.Register(nameof(Label), typeof(string), typeof(SettingsCard), new PropertyMetadata(default(string)));
    public static readonly DependencyProperty DescriptionProperty = DependencyProperty.Register(nameof(Description), typeof(string), typeof(SettingsCard), new PropertyMetadata(default(string), (d, _) => ((SettingsCard)d).OnDescriptionChanged()));
    public static readonly DependencyProperty IconProperty = DependencyProperty.Register(nameof(Icon), typeof(FrameworkElement), typeof(SettingsCard), new PropertyMetadata(defaultValue: null, (d, _) => ((SettingsCard)d).OnIconChanged()));

    public SettingsCard()
    {
        DefaultStyleKey = typeof(SettingsCard);
    }

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

    protected override void OnApplyTemplate()
    {
        base.OnApplyTemplate();

        OnDescriptionChanged();
        OnIconChanged();
    }

    private void OnDescriptionChanged()
    {
        if(GetTemplateChild("DescriptionPresenter") is FrameworkElement descriptionPresenter)
        {
            descriptionPresenter.Visibility = Description != null ? Visibility.Visible : Visibility.Collapsed;
        }
    }

    private void OnIconChanged()
    {
        if(GetTemplateChild("IconPresenter") is FrameworkElement iconPresenter)
        {
            iconPresenter.Visibility = Icon != null ? Visibility.Visible : Visibility.Collapsed;
        }
    }
}
