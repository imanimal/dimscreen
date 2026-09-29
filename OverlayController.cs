namespace DimScreen;

internal sealed class OverlayController : IDisposable
{
    private readonly List<OverlayWindow> windows = new();
    private string appliedTarget = string.Empty;
    private string appliedScreens = string.Empty;
    private bool enabled;

    public bool IsEnabled => enabled;
    public bool IsVisible => windows.Count > 0;

    public void SetEnabled(bool value, AppSettings settings, nint placeBelow = default)
    {
        enabled = value;
        Apply(settings, true, placeBelow);
    }

    public void Apply(AppSettings settings, bool forceRebuild = false, nint placeBelow = default)
    {
        if (!enabled)
        {
            DisposeWindows();
            return;
        }

        if (settings.Strength == 0)
        {
            DisposeWindows();
            return;
        }

        var screens = Screen.AllScreens;
        var screenSignature = string.Join("|", screens.Select(screen => $"{screen.DeviceName}:{screen.Bounds}"));
        if (forceRebuild || windows.Count == 0 || appliedTarget != settings.DisplayTarget || appliedScreens != screenSignature)
        {
            Rebuild(screens, settings, screenSignature, placeBelow);
            return;
        }

        foreach (var window in windows)
        {
            window.UpdateAppearance(settings, placeBelow);
        }
    }

    public void KeepOnTop()
    {
        foreach (var window in windows)
        {
            window.KeepOnTop();
        }
    }

    private void Rebuild(Screen[] screens, AppSettings settings, string screenSignature, nint placeBelow)
    {
        DisposeWindows();
        var selected = settings.DisplayTarget == "All displays"
            ? screens
            : screens.Where(screen => screen.DeviceName == settings.DisplayTarget).ToArray();
        if (selected.Length == 0)
        {
            selected = screens;
            settings.DisplayTarget = "All displays";
        }

        foreach (var screen in selected)
        {
            var window = new OverlayWindow(screen, settings);
            windows.Add(window);
            window.Show();
            if (placeBelow != default)
            {
                window.PlaceBelow(placeBelow);
            }
        }

        appliedTarget = settings.DisplayTarget;
        appliedScreens = screenSignature;
    }

    private void DisposeWindows()
    {
        foreach (var window in windows)
        {
            window.Close();
            window.Dispose();
        }

        windows.Clear();
    }

    public void Dispose()
    {
        enabled = false;
        DisposeWindows();
    }
}
