namespace DimScreen;

internal sealed class DimScreenContext : ApplicationContext
{
    private readonly AppSettings settings;
    private readonly OverlayController overlay = new();
    private readonly HotkeyHost hotkeyHost = new();
    private readonly NotifyIcon trayIcon;
    private readonly ToolStripMenuItem toggleMenuItem;
    private readonly System.Windows.Forms.Timer topmostTimer;
    private readonly System.Windows.Forms.Timer targetTimer;
    private MainForm? settingsForm;
    private bool shadeEnabled;
    private bool startHidden;
    private bool registeredLaunchAtStartup;
    private bool registeredStartHidden;

    public DimScreenContext(bool startHidden)
    {
        settings = AppSettings.Load();
        this.startHidden = startHidden || settings.StartHidden;
        registeredLaunchAtStartup = settings.LaunchAtStartup;
        registeredStartHidden = settings.StartHidden;

        var menu = new ContextMenuStrip();
        var openMenuItem = new ToolStripMenuItem("Open dimscreen", null, (_, _) => ShowSettings());
        toggleMenuItem = new ToolStripMenuItem("Enable screen shade") { CheckOnClick = false };
        toggleMenuItem.Click += (_, _) => SetOverlayEnabled(!shadeEnabled);
        var exitMenuItem = new ToolStripMenuItem("Exit", null, (_, _) => ExitApplication());
        menu.Items.Add(openMenuItem);
        menu.Items.Add(toggleMenuItem);
        menu.Items.Add(new ToolStripSeparator());
        menu.Items.Add(exitMenuItem);

        trayIcon = new NotifyIcon
        {
            Icon = SystemIcons.Application,
            Text = "dimscreen desktop dimmer",
            ContextMenuStrip = menu,
            Visible = true
        };
        trayIcon.DoubleClick += (_, _) => ShowSettings();

        try
        {
            StartupRegistration.SetEnabled(settings.LaunchAtStartup, settings.StartHidden);
        }
        catch (Exception exception)
        {
            trayIcon.ShowBalloonTip(3500, "dimscreen", exception.Message, ToolTipIcon.Warning);
        }

        hotkeyHost.Pressed += () => SetOverlayEnabled(!shadeEnabled);
        topmostTimer = new System.Windows.Forms.Timer { Interval = 1100 };
        topmostTimer.Tick += (_, _) =>
        {
            if (overlay.IsVisible && (settingsForm is null || !settingsForm.Visible || settingsForm.WindowState == FormWindowState.Minimized))
            {
                overlay.KeepOnTop();
            }
        };
        topmostTimer.Start();
        targetTimer = new System.Windows.Forms.Timer { Interval = 300 };
        targetTimer.Tick += (_, _) => UpdateOverlayVisibility();
        targetTimer.Start();

        ConfigureHotkey();
        if (!this.startHidden)
        {
            ShowSettings();
        }
    }

    private void ShowSettings()
    {
        if (settingsForm is null || settingsForm.IsDisposed)
        {
            settingsForm = new MainForm(
                settings,
                () => shadeEnabled,
                () => SetOverlayEnabled(!shadeEnabled),
                OnSettingsChanged);
        }

        settingsForm.Present();
    }

    private void SetOverlayEnabled(bool enabled)
    {
        shadeEnabled = enabled;
        UpdateOverlayVisibility(true);
        toggleMenuItem.Checked = enabled;
        toggleMenuItem.Text = enabled ? "Disable screen shade" : "Enable screen shade";
    }

    private void OnSettingsChanged()
    {
        try
        {
            settings.Save();
            if (registeredLaunchAtStartup != settings.LaunchAtStartup || registeredStartHidden != settings.StartHidden)
            {
                StartupRegistration.SetEnabled(settings.LaunchAtStartup, settings.StartHidden);
                registeredLaunchAtStartup = settings.LaunchAtStartup;
                registeredStartHidden = settings.StartHidden;
            }

            ConfigureHotkey();
            UpdateOverlayVisibility(true);
        }
        catch (Exception exception)
        {
            trayIcon.ShowBalloonTip(3500, "dimscreen", exception.Message, ToolTipIcon.Warning);
        }
    }

    private void UpdateOverlayVisibility(bool applySettings = false)
    {
        var targetMatches = IsTargetAppForeground();
        var shouldShow = shadeEnabled && targetMatches;
        if (shouldShow != overlay.IsEnabled)
        {
            overlay.SetEnabled(shouldShow, settings, GetVisibleSettingsHandle());
        }
        else if (shouldShow && applySettings)
        {
            overlay.Apply(settings, false, GetVisibleSettingsHandle());
        }

        settingsForm?.SetOverlayState(shadeEnabled, shadeEnabled && !targetMatches);
    }

    private bool IsTargetAppForeground()
    {
        if (string.IsNullOrWhiteSpace(settings.TargetProcessName))
        {
            return true;
        }

        var foregroundWindow = NativeMethods.GetForegroundWindow();
        if (foregroundWindow == default)
        {
            return false;
        }

        NativeMethods.GetWindowThreadProcessId(foregroundWindow, out var processId);
        if (processId == 0)
        {
            return false;
        }

        try
        {
            using var process = System.Diagnostics.Process.GetProcessById((int)processId);
            if (!process.ProcessName.Equals(settings.TargetProcessName, StringComparison.OrdinalIgnoreCase))
            {
                return false;
            }

            if (string.IsNullOrWhiteSpace(settings.TargetExecutablePath))
            {
                return true;
            }

            try
            {
                return string.Equals(process.MainModule?.FileName, settings.TargetExecutablePath, StringComparison.OrdinalIgnoreCase);
            }
            catch
            {
                return true;
            }
        }
        catch
        {
            return false;
        }
    }

    private nint GetVisibleSettingsHandle()
    {
        return settingsForm is not null && settingsForm.Visible && settingsForm.WindowState != FormWindowState.Minimized
            ? settingsForm.Handle
            : default;
    }

    private void ConfigureHotkey()
    {
        if (!hotkeyHost.Configure(settings.HotkeyModifiers, settings.HotkeyKey))
        {
            trayIcon?.ShowBalloonTip(3500, "dimscreen hotkey unavailable", "Another app is using this shortcut. Choose a different one in dimscreen settings.", ToolTipIcon.Warning);
        }
    }

    private void ExitApplication()
    {
        topmostTimer.Stop();
        targetTimer.Stop();
        overlay.Dispose();
        settingsForm?.CloseForExit();
        trayIcon.Visible = false;
        trayIcon.Dispose();
        hotkeyHost.Dispose();
        ExitThread();
    }

    protected override void Dispose(bool disposing)
    {
        if (disposing)
        {
            topmostTimer.Dispose();
            targetTimer.Dispose();
            overlay.Dispose();
            trayIcon.Dispose();
            hotkeyHost.Dispose();
            settingsForm?.Dispose();
        }

        base.Dispose(disposing);
    }
}
