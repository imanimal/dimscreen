using System.Diagnostics;

namespace DimScreen;

internal sealed class MainForm : Form
{
    private static readonly Color Page = Color.FromArgb(10, 15, 22);
    private static readonly Color Card = Color.FromArgb(18, 26, 37);
    private static readonly Color Raised = Color.FromArgb(25, 35, 48);
    private static readonly Color Border = Color.FromArgb(42, 56, 74);
    private static readonly Color Accent = Color.FromArgb(94, 180, 255);
    private static readonly Color PrimaryText = Color.FromArgb(236, 242, 250);
    private static readonly Color SecondaryText = Color.FromArgb(143, 158, 178);
    private readonly AppSettings settings;
    private readonly Func<bool> getOverlayEnabled;
    private readonly Action toggleOverlay;
    private readonly Action settingsChanged;
    private readonly Button overlayButton;
    private readonly Label statePill;
    private readonly Label strengthValue;
    private readonly TrackBar strengthTrackBar;
    private readonly Panel tintSwatch;
    private readonly ComboBox displayPicker;
    private readonly ComboBox appPicker;
    private readonly Button refreshAppsButton;
    private readonly Button hotkeyButton;
    private readonly CheckBox clickThroughCheckBox;
    private readonly CheckBox launchAtStartupCheckBox;
    private readonly CheckBox startHiddenCheckBox;
    private bool capturingHotkey;
    private bool allowExit;

    public MainForm(AppSettings settings, Func<bool> getOverlayEnabled, Action toggleOverlay, Action settingsChanged)
    {
        this.settings = settings;
        this.getOverlayEnabled = getOverlayEnabled;
        this.toggleOverlay = toggleOverlay;
        this.settingsChanged = settingsChanged;

        Text = "dimscreen";
        StartPosition = FormStartPosition.CenterScreen;
        FormBorderStyle = FormBorderStyle.FixedSingle;
        MaximizeBox = false;
        MinimizeBox = true;
        ShowInTaskbar = true;
        ClientSize = new Size(1050, 790);
        MinimumSize = new Size(1050, 829);
        BackColor = Page;
        ForeColor = PrimaryText;
        Font = new Font("Segoe UI", 9.5f, FontStyle.Regular);
        KeyPreview = true;

        var root = new TableLayoutPanel
        {
            Dock = DockStyle.Fill,
            ColumnCount = 1,
            RowCount = 2,
            Padding = new Padding(26, 22, 26, 22),
            BackColor = Page
        };
        root.RowStyles.Add(new RowStyle(SizeType.Absolute, 82));
        root.RowStyles.Add(new RowStyle(SizeType.Percent, 100));
        root.Controls.Add(BuildHeader(out overlayButton, out statePill), 0, 0);

        var grid = new TableLayoutPanel
        {
            Dock = DockStyle.Fill,
            ColumnCount = 2,
            RowCount = 2,
            BackColor = Page,
            Padding = new Padding(0, 8, 0, 0)
        };
        grid.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 56));
        grid.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 44));
        grid.RowStyles.Add(new RowStyle(SizeType.Percent, 57));
        grid.RowStyles.Add(new RowStyle(SizeType.Percent, 43));

        var shadeCard = BuildShadeCard(out strengthValue, out strengthTrackBar);
        var appearanceCard = BuildAppearanceCard(out tintSwatch);
        var displayCard = BuildDisplayCard(out displayPicker, out appPicker, out refreshAppsButton);
        var behaviorCard = BuildBehaviorCard(out hotkeyButton, out clickThroughCheckBox, out launchAtStartupCheckBox, out startHiddenCheckBox);
        grid.Controls.Add(shadeCard, 0, 0);
        grid.Controls.Add(appearanceCard, 1, 0);
        grid.Controls.Add(displayCard, 0, 1);
        grid.Controls.Add(behaviorCard, 1, 1);
        root.Controls.Add(grid, 0, 1);
        Controls.Add(root);

        overlayButton.Click += (_, _) => toggleOverlay();
        strengthTrackBar.ValueChanged += (_, _) =>
        {
            settings.Strength = strengthTrackBar.Value;
            strengthValue.Text = $"{settings.Strength}%";
            settingsChanged();
        };
        HookTintControls(appearanceCard, tintSwatch);
        displayPicker.SelectedIndexChanged += (_, _) =>
        {
            if (displayPicker.SelectedItem is DisplayChoice choice)
            {
                settings.DisplayTarget = choice.Key;
                settingsChanged();
            }
        };
        appPicker.SelectedIndexChanged += (_, _) =>
        {
            if (appPicker.SelectedItem is AppChoice choice)
            {
                settings.TargetProcessName = choice.ProcessName;
                settings.TargetExecutablePath = choice.ExecutablePath;
                settings.TargetAppLabel = choice.ProcessName.Length == 0 ? string.Empty : choice.Text.Replace("  ·  not running", string.Empty);
                settingsChanged();
            }
        };
        refreshAppsButton.Click += (_, _) => PopulateApps(appPicker);
        hotkeyButton.Click += (_, _) => BeginHotkeyCapture();
        clickThroughCheckBox.CheckedChanged += (_, _) =>
        {
            settings.ClickThrough = clickThroughCheckBox.Checked;
            settingsChanged();
        };
        launchAtStartupCheckBox.CheckedChanged += (_, _) =>
        {
            settings.LaunchAtStartup = launchAtStartupCheckBox.Checked;
            startHiddenCheckBox.Enabled = launchAtStartupCheckBox.Checked;
            settingsChanged();
        };
        startHiddenCheckBox.CheckedChanged += (_, _) =>
        {
            settings.StartHidden = startHiddenCheckBox.Checked;
            settingsChanged();
        };
        KeyDown += CaptureHotkey;

        strengthTrackBar.Value = settings.Strength;
        strengthValue.Text = $"{settings.Strength}%";
        tintSwatch.BackColor = settings.TintColor;
        clickThroughCheckBox.Checked = settings.ClickThrough;
        launchAtStartupCheckBox.Checked = settings.LaunchAtStartup;
        startHiddenCheckBox.Checked = settings.StartHidden;
        startHiddenCheckBox.Enabled = settings.LaunchAtStartup;
        PopulateDisplays(displayPicker);
        PopulateApps(appPicker);
        hotkeyButton.Text = FormatHotkey(settings.HotkeyModifiers, settings.HotkeyKey);
        SetOverlayState(getOverlayEnabled(), getOverlayEnabled() && !string.IsNullOrWhiteSpace(settings.TargetProcessName));
    }

    public void Present()
    {
        if (!Visible)
        {
            Show();
        }

        TopMost = true;
        BringToFront();
        Activate();
    }

    public void SetOverlayState(bool enabled, bool waitingForApp)
    {
        overlayButton.Text = enabled ? "DISABLE SHADE" : "ENABLE SHADE";
        overlayButton.BackColor = enabled ? Color.FromArgb(44, 118, 101) : Raised;
        statePill.Text = !enabled ? "○  PAUSED" : waitingForApp ? "◷  WAITING" : "●  ACTIVE";
        statePill.ForeColor = enabled && !waitingForApp ? Color.FromArgb(122, 232, 191) : SecondaryText;
    }

    public void CloseForExit()
    {
        allowExit = true;
        Close();
    }

    protected override void OnFormClosing(FormClosingEventArgs e)
    {
        if (!allowExit && e.CloseReason == CloseReason.UserClosing)
        {
            e.Cancel = true;
            Hide();
            TopMost = false;
            return;
        }

        base.OnFormClosing(e);
    }

    private Control BuildHeader(out Button toggleButton, out Label status)
    {
        var header = new TableLayoutPanel { Dock = DockStyle.Fill, ColumnCount = 3, RowCount = 1, BackColor = Page };
        header.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 58));
        header.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        header.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 270));

        var logo = new Label
        {
            Text = "S",
            Dock = DockStyle.Fill,
            TextAlign = ContentAlignment.MiddleCenter,
            BackColor = Color.FromArgb(34, 91, 137),
            ForeColor = Color.White,
            Font = new Font("Segoe UI Semibold", 22, FontStyle.Bold),
            Margin = new Padding(0, 5, 14, 8)
        };
        var titleStack = new Panel { Dock = DockStyle.Fill, BackColor = Page };
        var title = new Label
        {
            Text = "dimscreen",
            Font = new Font("Segoe UI Semibold", 19, FontStyle.Bold),
            ForeColor = PrimaryText,
            AutoSize = true,
            Location = new Point(0, 4)
        };
        var caption = new Label
        {
            Text = "A calm layer over your desktop",
            ForeColor = SecondaryText,
            AutoSize = true,
            Location = new Point(1, 39)
        };
        titleStack.Controls.Add(title);
        titleStack.Controls.Add(caption);

        var right = new TableLayoutPanel { Dock = DockStyle.Fill, ColumnCount = 2, RowCount = 1, BackColor = Page };
        right.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 42));
        right.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 58));
        status = new Label
        {
            Text = "○  PAUSED",
            Dock = DockStyle.Fill,
            TextAlign = ContentAlignment.MiddleCenter,
            ForeColor = SecondaryText,
            Font = new Font("Segoe UI Semibold", 8.5f, FontStyle.Bold),
            Margin = new Padding(0, 12, 8, 13)
        };
        toggleButton = CreateButton("ENABLE SHADE", Accent, Color.FromArgb(7, 19, 29), true);
        toggleButton.Dock = DockStyle.Fill;
        toggleButton.Margin = new Padding(4, 14, 0, 14);
        right.Controls.Add(status, 0, 0);
        right.Controls.Add(toggleButton, 1, 0);
        header.Controls.Add(logo, 0, 0);
        header.Controls.Add(titleStack, 1, 0);
        header.Controls.Add(right, 2, 0);
        return header;
    }

    private Panel BuildShadeCard(out Label valueLabel, out TrackBar trackBar)
    {
        var card = CreateCard("Screen shade", "Set how much light the overlay removes.", out var body);
        var valueRow = new TableLayoutPanel { Dock = DockStyle.Top, Height = 36, ColumnCount = 2, BackColor = Card, Margin = Padding.Empty };
        valueRow.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        valueRow.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 70));
        var dimLabel = MakeLabel("Dim strength", SecondaryText, 9.5f, true);
        valueLabel = MakeLabel($"{settings.Strength}%", Accent, 15f, true);
        valueLabel.TextAlign = ContentAlignment.MiddleRight;
        valueLabel.Dock = DockStyle.Fill;
        valueRow.Controls.Add(dimLabel, 0, 0);
        valueRow.Controls.Add(valueLabel, 1, 0);

        trackBar = new TrackBar
        {
            Minimum = 0,
            Maximum = 100,
            Value = settings.Strength,
            TickStyle = TickStyle.None,
            LargeChange = 10,
            SmallChange = 1,
            Height = 42,
            Dock = DockStyle.Top,
            BackColor = Card,
            Margin = new Padding(-6, 0, -6, 0)
        };
        var range = new TableLayoutPanel { Dock = DockStyle.Top, Height = 22, ColumnCount = 2, BackColor = Card, Margin = new Padding(0, -2, 0, 5) };
        range.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 50));
        range.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 50));
        range.Controls.Add(MakeLabel("OFF", SecondaryText, 8.5f, true), 0, 0);
        var maxLabel = MakeLabel("FULL SHADE  ·  100%", SecondaryText, 8.5f, true);
        maxLabel.TextAlign = ContentAlignment.MiddleRight;
        range.Controls.Add(maxLabel, 1, 0);

        var presets = new TableLayoutPanel { Dock = DockStyle.Top, Height = 39, ColumnCount = 3, BackColor = Card, Margin = new Padding(0, 7, 0, 7) };
        presets.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 33.33f));
        presets.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 33.34f));
        presets.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 33.33f));
        AddPreset(presets, "SOFT  ·  25%", 25, 0);
        AddPreset(presets, "FOCUS  ·  50%", 50, 1);
        AddPreset(presets, "NIGHT  ·  70%", 70, 2);
        AddBody(body, valueRow);
        AddBody(body, trackBar);
        AddBody(body, range);
        AddBody(body, presets);
        var hint = MakeLabel("Changes appear instantly across the selected displays.", SecondaryText, 9f);
        hint.Padding = new Padding(0, 8, 0, 0);
        AddBody(body, hint);
        return card;
    }

    private Panel BuildAppearanceCard(out Panel swatch)
    {
        var card = CreateCard("Appearance", "Choose a tint or create your own color.", out var body);
        var colorRow = new TableLayoutPanel { Dock = DockStyle.Top, Height = 50, ColumnCount = 2, BackColor = Card, Margin = new Padding(0, 0, 0, 14) };
        colorRow.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        colorRow.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 132));
        var sample = new Panel { Dock = DockStyle.Fill, BackColor = Card, Padding = new Padding(0), Margin = Padding.Empty };
        swatch = new Panel { Size = new Size(38, 38), Location = new Point(0, 5), BackColor = settings.TintColor };
        var colorTitle = MakeLabel("Overlay color", PrimaryText, 10f, true);
        colorTitle.Location = new Point(52, 3);
        colorTitle.Size = new Size(155, 20);
        var colorCode = MakeLabel(settings.TintHex.ToUpperInvariant(), SecondaryText, 9f);
        colorCode.Name = "TintCode";
        colorCode.Location = new Point(52, 24);
        sample.Controls.Add(swatch);
        sample.Controls.Add(colorTitle);
        sample.Controls.Add(colorCode);
        var choose = CreateButton("CHOOSE COLOR", Raised, PrimaryText, true);
        choose.Dock = DockStyle.Fill;
        colorRow.Controls.Add(sample, 0, 0);
        colorRow.Controls.Add(choose, 1, 0);

        var paletteLabel = MakeLabel("QUICK TINTS", SecondaryText, 8.5f, true);
        paletteLabel.Dock = DockStyle.Top;
        paletteLabel.Height = 24;
        var palette = new TableLayoutPanel { Dock = DockStyle.Top, Height = 86, ColumnCount = 2, RowCount = 2, BackColor = Card, Margin = Padding.Empty };
        palette.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 50));
        palette.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 50));
        palette.RowStyles.Add(new RowStyle(SizeType.Percent, 50));
        palette.RowStyles.Add(new RowStyle(SizeType.Percent, 50));
        AddTintPreset(palette, "●  Ink", "#000000", 0, 0);
        AddTintPreset(palette, "●  Warm amber", "#32200A", 1, 0);
        AddTintPreset(palette, "●  Blue hour", "#0B1932", 0, 1);
        AddTintPreset(palette, "●  Rose dusk", "#2B101C", 1, 1);
        AddBody(body, colorRow);
        AddBody(body, paletteLabel);
        AddBody(body, palette);
        choose.Click += (_, _) =>
        {
            using var dialog = new ColorDialog { Color = settings.TintColor, FullOpen = true, AnyColor = true };
            if (dialog.ShowDialog(this) == DialogResult.OK)
            {
                ApplyTint(ColorTranslator.ToHtml(dialog.Color));
            }
        };
        return card;
    }

    private Panel BuildDisplayCard(out ComboBox picker, out ComboBox appFilterPicker, out Button refreshButton)
    {
        var card = CreateCard("Displays", "Apply the shade where you need it.", out var body);
        var label = MakeLabel("APPLY TO DISPLAY", SecondaryText, 8.5f, true);
        label.Dock = DockStyle.Top;
        label.Height = 24;
        picker = new ComboBox
        {
            Dock = DockStyle.Top,
            DropDownStyle = ComboBoxStyle.DropDownList,
            FlatStyle = FlatStyle.Flat,
            BackColor = Raised,
            ForeColor = PrimaryText,
            Font = new Font("Segoe UI", 10f),
            Height = 38,
            Margin = Padding.Empty
        };
        var appHeader = new TableLayoutPanel { Dock = DockStyle.Top, Height = 29, ColumnCount = 2, BackColor = Card, Margin = new Padding(0, 4, 0, 0) };
        appHeader.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        appHeader.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 92));
        var appLabel = MakeLabel("ONLY WHEN THIS APP IS ACTIVE", SecondaryText, 8.5f, true);
        appLabel.Dock = DockStyle.Fill;
        refreshButton = CreateButton("REFRESH", Raised, SecondaryText, true);
        refreshButton.Dock = DockStyle.Fill;
        refreshButton.Margin = Padding.Empty;
        appHeader.Controls.Add(appLabel, 0, 0);
        appHeader.Controls.Add(refreshButton, 1, 0);
        appFilterPicker = new ComboBox
        {
            Dock = DockStyle.Top,
            DropDownStyle = ComboBoxStyle.DropDownList,
            FlatStyle = FlatStyle.Flat,
            BackColor = Raised,
            ForeColor = PrimaryText,
            Font = new Font("Segoe UI", 9.5f),
            Height = 38,
            Margin = Padding.Empty
        };
        var helper = MakeLabel("Select “All apps” to keep the shade on everywhere.", SecondaryText, 8.8f);
        helper.Dock = DockStyle.Top;
        helper.Height = 27;
        helper.Padding = new Padding(0, 5, 0, 0);
        AddBody(body, label);
        AddBody(body, picker);
        AddBody(body, appHeader);
        AddBody(body, appFilterPicker);
        AddBody(body, helper);
        return card;
    }

    private Panel BuildBehaviorCard(out Button shortcut, out CheckBox clickThrough, out CheckBox launchAtStartup, out CheckBox startHidden)
    {
        var card = CreateCard("Behavior", "Shortcuts and startup options.", out var body);
        var hotkeyTitle = MakeLabel("TOGGLE SHORTCUT", SecondaryText, 8.5f, true);
        hotkeyTitle.Dock = DockStyle.Top;
        hotkeyTitle.Height = 24;
        shortcut = CreateButton(FormatHotkey(settings.HotkeyModifiers, settings.HotkeyKey), Raised, PrimaryText, true);
        shortcut.Dock = DockStyle.Top;
        shortcut.Height = 38;
        shortcut.Margin = new Padding(0, 0, 0, 8);
        clickThrough = MakeCheckBox("Clicks pass through the shade", Card);
        launchAtStartup = MakeCheckBox("Launch dimscreen with Windows", Card);
        startHidden = MakeCheckBox("Start quietly in the system tray", Card);
        startHidden.Enabled = settings.LaunchAtStartup;
        AddBody(body, hotkeyTitle);
        AddBody(body, shortcut);
        AddBody(body, clickThrough);
        AddBody(body, launchAtStartup);
        AddBody(body, startHidden);
        return card;
    }

    private Panel CreateCard(string title, string subtitle, out Panel body)
    {
        var shell = new Panel
        {
            Dock = DockStyle.Fill,
            BackColor = Card,
            Padding = new Padding(20, 17, 20, 15),
            Margin = new Padding(6)
        };
        var heading = new Panel { Dock = DockStyle.Top, Height = 59, BackColor = Card };
        var titleLabel = MakeLabel(title, PrimaryText, 13f, true);
        titleLabel.Location = new Point(0, 0);
        titleLabel.Size = new Size(300, 25);
        var subtitleLabel = MakeLabel(subtitle, SecondaryText, 8.8f);
        subtitleLabel.Location = new Point(0, 29);
        subtitleLabel.Size = new Size(410, 22);
        heading.Controls.Add(titleLabel);
        heading.Controls.Add(subtitleLabel);
        body = new Panel
        {
            Dock = DockStyle.Fill,
            AutoScroll = true,
            BackColor = Card,
            Padding = Padding.Empty,
            Margin = Padding.Empty
        };
        var bodyPanel = body;
        bodyPanel.SizeChanged += (_, _) => ResizeBodyChildren(bodyPanel);
        shell.Controls.Add(body);
        shell.Controls.Add(heading);
        shell.Paint += (_, e) =>
        {
            using var pen = new Pen(Border);
            e.Graphics.DrawRectangle(pen, 0, 0, shell.Width - 1, shell.Height - 1);
        };
        return shell;
    }

    private void AddBody(Panel body, Control control)
    {
        var bottom = body.Controls.Cast<Control>().Select(child => child.Bottom).DefaultIfEmpty(0).Max();
        control.Dock = DockStyle.None;
        control.Location = new Point(0, bottom + (body.Controls.Count == 0 ? 0 : 4));
        control.Width = Math.Max(120, body.ClientSize.Width - 2);
        body.Controls.Add(control);
    }

    private void ResizeBodyChildren(Panel body)
    {
        foreach (Control control in body.Controls)
        {
            control.Width = Math.Max(120, body.ClientSize.Width - control.Margin.Horizontal - 2);
        }
    }

    private void AddPreset(TableLayoutPanel row, string text, int value, int column)
    {
        var button = CreateButton(text, Raised, SecondaryText, true);
        button.Dock = DockStyle.Fill;
        button.Margin = new Padding(0, 0, column == 2 ? 0 : 7, 0);
        button.Click += (_, _) => strengthTrackBar.Value = value;
        row.Controls.Add(button, column, 0);
    }

    private void AddTintPreset(TableLayoutPanel palette, string text, string hex, int column, int row)
    {
        var button = CreateButton(text, Raised, PrimaryText, false);
        button.Dock = DockStyle.Fill;
        button.TextAlign = ContentAlignment.MiddleLeft;
        button.Padding = new Padding(10, 0, 0, 0);
        button.Margin = new Padding(0, 0, column == 0 ? 7 : 0, 7);
        button.Click += (_, _) => ApplyTint(hex);
        palette.Controls.Add(button, column, row);
    }

    private void HookTintControls(Panel appearanceCard, Panel swatch)
    {
        _ = appearanceCard;
        swatch.BackColor = settings.TintColor;
    }

    private void ApplyTint(string hex)
    {
        settings.TintHex = hex;
        tintSwatch.BackColor = settings.TintColor;
        foreach (Control control in Controls.Find("TintCode", true))
        {
            control.Text = hex.ToUpperInvariant();
        }

        settingsChanged();
    }

    private void PopulateDisplays(ComboBox picker)
    {
        picker.Items.Clear();
        picker.Items.Add(new DisplayChoice("All displays", "All displays"));
        var screens = Screen.AllScreens;
        for (var index = 0; index < screens.Length; index++)
        {
            var screen = screens[index];
            picker.Items.Add(new DisplayChoice(
                $"Display {index + 1}  ·  {screen.Bounds.Width} × {screen.Bounds.Height}",
                screen.DeviceName));
        }

        var selectedIndex = 0;
        for (var index = 0; index < picker.Items.Count; index++)
        {
            if (picker.Items[index] is DisplayChoice choice && choice.Key == settings.DisplayTarget)
            {
                selectedIndex = index;
                break;
            }
        }

        picker.SelectedIndex = selectedIndex;
        settings.DisplayTarget = ((DisplayChoice)picker.SelectedItem!).Key;
    }

    private void PopulateApps(ComboBox picker)
    {
        var choices = new List<AppChoice> { new("All apps", string.Empty, string.Empty) };
        var seenPaths = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        foreach (var process in Process.GetProcesses())
        {
            using (process)
            {
                try
                {
                    if (process.Id == Environment.ProcessId || process.MainWindowHandle == default)
                    {
                        continue;
                    }

                    var title = process.MainWindowTitle.Trim();
                    if (title.Length == 0)
                    {
                        continue;
                    }

                    var processName = process.ProcessName;
                    var executablePath = string.Empty;
                    try
                    {
                        executablePath = process.MainModule?.FileName ?? string.Empty;
                    }
                    catch
                    {
                    }

                    var identity = executablePath.Length == 0 ? processName : executablePath;
                    if (!seenPaths.Add(identity))
                    {
                        continue;
                    }

                    choices.Add(new AppChoice($"{title}  ·  {processName}.exe", processName, executablePath));
                }
                catch
                {
                }
            }
        }

        var runningChoices = choices.Skip(1).OrderBy(choice => choice.Text, StringComparer.CurrentCultureIgnoreCase).ToList();
        choices = new List<AppChoice> { choices[0] };
        choices.AddRange(runningChoices);
        var selectedIndex = 0;
        if (!string.IsNullOrWhiteSpace(settings.TargetProcessName))
        {
            selectedIndex = choices.FindIndex(choice =>
                choice.ProcessName.Equals(settings.TargetProcessName, StringComparison.OrdinalIgnoreCase) &&
                (string.IsNullOrWhiteSpace(settings.TargetExecutablePath) || string.IsNullOrWhiteSpace(choice.ExecutablePath) ||
                 choice.ExecutablePath.Equals(settings.TargetExecutablePath, StringComparison.OrdinalIgnoreCase)));
            if (selectedIndex < 0)
            {
                var label = string.IsNullOrWhiteSpace(settings.TargetAppLabel)
                    ? $"{settings.TargetProcessName}.exe"
                    : settings.TargetAppLabel;
                choices.Insert(1, new AppChoice($"{label}  ·  not running", settings.TargetProcessName, settings.TargetExecutablePath));
                selectedIndex = 1;
            }
        }

        picker.BeginUpdate();
        picker.Items.Clear();
        foreach (var choice in choices)
        {
            picker.Items.Add(choice);
        }

        picker.SelectedIndex = selectedIndex;
        picker.EndUpdate();
    }

    private void BeginHotkeyCapture()
    {
        capturingHotkey = true;
        hotkeyButton.Text = "PRESS A KEY COMBINATION…";
        hotkeyButton.BackColor = Color.FromArgb(35, 73, 105);
        Focus();
    }

    private void CaptureHotkey(object? sender, KeyEventArgs e)
    {
        if (!capturingHotkey)
        {
            return;
        }

        if (e.KeyCode == Keys.Escape)
        {
            capturingHotkey = false;
            hotkeyButton.Text = FormatHotkey(settings.HotkeyModifiers, settings.HotkeyKey);
            hotkeyButton.BackColor = Raised;
            e.Handled = true;
            return;
        }

        if (e.KeyCode is Keys.ControlKey or Keys.ShiftKey or Keys.Menu)
        {
            return;
        }

        var modifiers = 0;
        if (e.Alt) modifiers |= (int)NativeMethods.ModAlt;
        if (e.Control) modifiers |= (int)NativeMethods.ModControl;
        if (e.Shift) modifiers |= (int)NativeMethods.ModShift;
        if (modifiers == 0)
        {
            return;
        }

        settings.HotkeyModifiers = modifiers;
        settings.HotkeyKey = (int)e.KeyCode;
        hotkeyButton.Text = FormatHotkey(modifiers, settings.HotkeyKey);
        hotkeyButton.BackColor = Raised;
        capturingHotkey = false;
        settingsChanged();
        e.Handled = true;
    }

    private static string FormatHotkey(int modifiers, int key)
    {
        var parts = new List<string>();
        if ((modifiers & NativeMethods.ModControl) != 0) parts.Add("Ctrl");
        if ((modifiers & NativeMethods.ModAlt) != 0) parts.Add("Alt");
        if ((modifiers & NativeMethods.ModShift) != 0) parts.Add("Shift");
        if ((modifiers & NativeMethods.ModWin) != 0) parts.Add("Win");
        parts.Add(((Keys)key).ToString().Replace("D0", "0").Replace("D1", "1").Replace("D2", "2").Replace("D3", "3").Replace("D4", "4").Replace("D5", "5").Replace("D6", "6").Replace("D7", "7").Replace("D8", "8").Replace("D9", "9"));
        return string.Join(" + ", parts);
    }

    private Button CreateButton(string text, Color backColor, Color foreColor, bool centered)
    {
        return new Button
        {
            Text = text,
            BackColor = backColor,
            ForeColor = foreColor,
            FlatStyle = FlatStyle.Flat,
            Font = new Font("Segoe UI Semibold", 8.5f, FontStyle.Bold),
            TextAlign = centered ? ContentAlignment.MiddleCenter : ContentAlignment.MiddleLeft,
            Height = 37,
            Cursor = Cursors.Hand,
            UseVisualStyleBackColor = false,
            FlatAppearance = { BorderColor = Border, BorderSize = 1, MouseOverBackColor = Color.FromArgb(37, 51, 69), MouseDownBackColor = Color.FromArgb(47, 64, 85) }
        };
    }

    private static Label MakeLabel(string text, Color color, float size, bool semibold = false)
    {
        return new Label
        {
            Text = text,
            ForeColor = color,
            Font = new Font(semibold ? "Segoe UI Semibold" : "Segoe UI", size, semibold ? FontStyle.Bold : FontStyle.Regular),
            AutoSize = false,
            TextAlign = ContentAlignment.MiddleLeft,
            BackColor = Color.Transparent
        };
    }

    private static CheckBox MakeCheckBox(string text, Color backColor)
    {
        return new CheckBox
        {
            Text = text,
            ForeColor = PrimaryText,
            BackColor = backColor,
            Font = new Font("Segoe UI", 9f),
            AutoSize = false,
            Height = 28,
            TextAlign = ContentAlignment.MiddleLeft,
            CheckAlign = ContentAlignment.MiddleLeft,
            Padding = new Padding(2, 0, 0, 0),
            Cursor = Cursors.Hand
        };
    }

    private sealed record DisplayChoice(string Text, string Key)
    {
        public override string ToString() => Text;
    }

    private sealed record AppChoice(string Text, string ProcessName, string ExecutablePath)
    {
        public override string ToString() => Text;
    }
}
