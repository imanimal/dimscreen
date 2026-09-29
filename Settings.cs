using System.Text.Json;
using Microsoft.Win32;

namespace DimScreen;

internal sealed class AppSettings
{
    public int Strength { get; set; } = 48;
    public string TintHex { get; set; } = "#000000";
    public string DisplayTarget { get; set; } = "All displays";
    public string TargetProcessName { get; set; } = string.Empty;
    public string TargetExecutablePath { get; set; } = string.Empty;
    public string TargetAppLabel { get; set; } = string.Empty;
    public int HotkeyModifiers { get; set; } = 3;
    public int HotkeyKey { get; set; } = (int)Keys.D;
    public bool ClickThrough { get; set; } = true;
    public bool LaunchAtStartup { get; set; }
    public bool StartHidden { get; set; }

    public static string FilePath => Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
        "dimscreen",
        "settings.json");

    private static string LegacyFilePath => Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
        "ScreenShade",
        "settings.json");

    public static AppSettings Load()
    {
        try
        {
            var settingsPath = File.Exists(FilePath) ? FilePath : LegacyFilePath;
            if (File.Exists(settingsPath))
            {
                var settings = JsonSerializer.Deserialize<AppSettings>(File.ReadAllText(settingsPath));
                if (settings is not null)
                {
                    settings.Strength = Math.Clamp(settings.Strength, 0, 100);
                    settings.TintHex = IsHexColor(settings.TintHex) ? settings.TintHex : "#000000";
                    settings.DisplayTarget ??= "All displays";
                    settings.TargetProcessName ??= string.Empty;
                    settings.TargetExecutablePath ??= string.Empty;
                    settings.TargetAppLabel ??= string.Empty;
                    settings.HotkeyModifiers = Math.Clamp(settings.HotkeyModifiers, 1, 15);
                    if (!Enum.IsDefined((Keys)settings.HotkeyKey))
                    {
                        settings.HotkeyKey = (int)Keys.D;
                    }

                    return settings;
                }
            }
        }
        catch
        {
        }

        return new AppSettings();
    }

    public void Save()
    {
        Directory.CreateDirectory(Path.GetDirectoryName(FilePath)!);
        File.WriteAllText(FilePath, JsonSerializer.Serialize(this, new JsonSerializerOptions { WriteIndented = true }));
    }

    public Color TintColor => ColorTranslator.FromHtml(TintHex);

    private static bool IsHexColor(string value)
    {
        return value.Length == 7 && value[0] == '#' && int.TryParse(value.AsSpan(1), System.Globalization.NumberStyles.HexNumber, null, out _);
    }
}

internal static class StartupRegistration
{
    private const string RunKey = @"Software\Microsoft\Windows\CurrentVersion\Run";
    private const string ValueName = "dimscreen";
    private const string LegacyValueName = "ScreenShade";

    public static void SetEnabled(bool enabled, bool startHidden)
    {
        using var key = Registry.CurrentUser.CreateSubKey(RunKey, true);
        key.DeleteValue(LegacyValueName, false);
        if (!enabled)
        {
            key.DeleteValue(ValueName, false);
            return;
        }

        var executable = Environment.ProcessPath;
        if (string.IsNullOrWhiteSpace(executable))
        {
            throw new InvalidOperationException("Could not find the dimscreen executable path.");
        }

        key.SetValue(ValueName, $"\"{executable}\"{(startHidden ? " --minimized" : string.Empty)}");
    }
}
