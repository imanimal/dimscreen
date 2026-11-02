//! Saved settings: `%APPDATA%\dimscreen\settings.json`, in the same PascalCase format the C# version
//! wrote, so an existing file carries over. Unknown keys (the old `TintColor` object) are dropped on
//! the next save.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const ALL_DISPLAYS: &str = "All displays";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Settings {
    /// 0–100: how much light the shade removes.
    pub strength: u8,
    pub tint_hex: String,
    /// `ALL_DISPLAYS` or a monitor's device name (`\\.\DISPLAY2`).
    pub display_target: String,
    /// When set, the shade shows only while this app is in the foreground.
    pub target_process_name: String,
    pub target_executable_path: String,
    pub target_app_label: String,
    /// MOD_ALT 1 · MOD_CONTROL 2 · MOD_SHIFT 4 · MOD_WIN 8, and a virtual-key code.
    pub hotkey_modifiers: u32,
    pub hotkey_key: u32,
    pub click_through: bool,
    pub launch_at_startup: bool,
    pub start_hidden: bool,
    /// The shade's on/off switch, restored at launch.
    pub enabled: bool,
    /// "standard" or "pocket".
    pub look: String,
    /// "system", "light" or "dark".
    pub theme: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            strength: 50,
            tint_hex: "#000000".into(),
            display_target: ALL_DISPLAYS.into(),
            target_process_name: String::new(),
            target_executable_path: String::new(),
            target_app_label: String::new(),
            hotkey_modifiers: 3,
            hotkey_key: 0x44, // Ctrl + Alt + D
            click_through: true,
            launch_at_startup: false,
            start_hidden: false,
            enabled: false,
            look: "standard".into(),
            theme: "system".into(),
        }
    }
}

impl Settings {
    /// Clamps anything a hand-edited or older file might get wrong.
    pub fn sanitize(&mut self) {
        let d = Settings::default();
        self.strength = self.strength.min(100);
        if parse_hex(&self.tint_hex).is_none() {
            self.tint_hex = d.tint_hex;
        }
        self.tint_hex = self.tint_hex.to_ascii_uppercase();
        if self.display_target.is_empty() {
            self.display_target = d.display_target;
        }
        self.hotkey_modifiers &= 0xF;
        if self.hotkey_modifiers == 0 || self.hotkey_key == 0 || self.hotkey_key > 0xFE {
            self.hotkey_modifiers = d.hotkey_modifiers;
            self.hotkey_key = d.hotkey_key;
        }
        if !matches!(self.look.as_str(), "standard" | "pocket") {
            self.look = d.look;
        }
        if !matches!(self.theme.as_str(), "system" | "light" | "dark") {
            self.theme = d.theme;
        }
    }

    /// The tint as a GDI COLORREF (0x00BBGGRR).
    pub fn colorref(&self) -> u32 {
        let (r, g, b) = parse_hex(&self.tint_hex).unwrap_or((0, 0, 0));
        u32::from(r) | u32::from(g) << 8 | u32::from(b) << 16
    }
}

fn parse_hex(s: &str) -> Option<(u8, u8, u8)> {
    let h = s.strip_prefix('#')?;
    if h.len() != 6 || !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some(((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

fn dir() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    base.join("dimscreen")
}

fn file() -> PathBuf {
    dir().join("settings.json")
}

/// The saved settings, and whether this is the first run (no file yet).
pub fn load() -> (Settings, bool) {
    let path = file();
    let legacy = dir().parent().map(|p| p.join("ScreenShade").join("settings.json"));
    let text = std::fs::read_to_string(&path)
        .ok()
        .or_else(|| legacy.and_then(|p| std::fs::read_to_string(p).ok()));
    let first_run = text.is_none();
    let mut s: Settings = text.and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    s.sanitize();
    (s, first_run)
}

pub fn save(s: &Settings) -> std::io::Result<()> {
    std::fs::create_dir_all(dir())?;
    let tmp = dir().join("settings.json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(s).unwrap_or_default())?;
    std::fs::rename(tmp, file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_csharp_settings_file() {
        let old = r##"{ "Strength": 50, "TintHex": "#32200a", "DisplayTarget": "All displays", "TargetProcessName": "chrome",
            "TargetExecutablePath": "C:\\chrome.exe", "TargetAppLabel": "Google Chrome", "HotkeyModifiers": 3, "HotkeyKey": 68,
            "ClickThrough": true, "LaunchAtStartup": false, "StartHidden": false,
            "TintColor": { "R": 50, "G": 32, "B": 10, "A": 255, "IsKnownColor": false, "Name": "ff32200a" } }"##;
        let mut s: Settings = serde_json::from_str(old).unwrap();
        s.sanitize();
        assert_eq!(s.strength, 50);
        assert_eq!(s.tint_hex, "#32200A");
        assert_eq!(s.colorref(), 0x000A2032);
        assert_eq!(s.target_process_name, "chrome");
        assert!(!s.enabled);
        assert_eq!(s.look, "standard");
    }

    #[test]
    fn sanitize_fixes_bad_values() {
        let mut s = Settings { strength: 180, tint_hex: "red".into(), hotkey_modifiers: 0, look: "neon".into(), ..Default::default() };
        s.sanitize();
        assert_eq!((s.strength, s.tint_hex.as_str(), s.hotkey_modifiers, s.look.as_str()), (100, "#000000", 3, "standard"));
    }
}
