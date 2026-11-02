//! Small Windows helpers: monitors, running apps, process paths, key names and the startup entry.

use std::ffi::c_void;
use std::path::Path;

use serde::Serialize;
use windows_sys::Win32::Foundation::{CloseHandle, HWND, LPARAM, RECT};
use windows_sys::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use windows_sys::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
};
use windows_sys::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW};
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE,
    REG_OPTION_NON_VOLATILE, REG_SZ,
};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetKeyNameTextW, MapVirtualKeyW, MAPVK_VK_TO_VSC};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindow, GetWindowLongPtrW, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
    IsWindowVisible, GWL_EXSTYLE, GW_OWNER, WS_EX_TOOLWINDOW,
};

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn from_wide(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

// ───────────── monitors ─────────────

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Monitor {
    /// `\\.\DISPLAY1`: what `DisplayTarget` stores.
    pub device: String,
    /// "Display 1", numbered like Windows numbers them.
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub primary: bool,
}

impl Monitor {
    pub fn rect(&self) -> RECT {
        RECT { left: self.x, top: self.y, right: self.x + self.width, bottom: self.y + self.height }
    }
}

pub fn monitors() -> Vec<Monitor> {
    unsafe extern "system" fn each(hmon: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> i32 {
        let out = &mut *(data as *mut Vec<Monitor>);
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if GetMonitorInfoW(hmon, &mut info as *mut MONITORINFOEXW as *mut MONITORINFO) != 0 {
            let r = info.monitorInfo.rcMonitor;
            let device = from_wide(&info.szDevice);
            let number: String = device.chars().rev().take_while(|c| c.is_ascii_digit()).collect::<Vec<_>>().into_iter().rev().collect();
            out.push(Monitor {
                name: format!("Display {}", if number.is_empty() { (out.len() + 1).to_string() } else { number }),
                device,
                x: r.left,
                y: r.top,
                width: r.right - r.left,
                height: r.bottom - r.top,
                primary: info.monitorInfo.dwFlags & 1 != 0,
            });
        }
        1
    }
    let mut out: Vec<Monitor> = Vec::new();
    unsafe { EnumDisplayMonitors(std::ptr::null_mut(), std::ptr::null(), Some(each), &mut out as *mut _ as LPARAM) };
    out.sort_by_key(|m| (m.name.len(), m.name.clone()));
    out
}

// ───────────── processes and apps ─────────────

pub fn process_path(pid: u32) -> Option<String> {
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return None;
        }
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len) != 0;
        CloseHandle(h);
        ok.then(|| String::from_utf16_lossy(&buf[..len as usize]))
    }
}

pub fn window_pid(hwnd: HWND) -> u32 {
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
    pid
}

/// The file name without `.exe`: what the C# version stored as `TargetProcessName`.
pub fn exe_stem(path: &str) -> String {
    Path::new(path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

/// The exe's "File description" ("Google Chrome"), if it has one.
fn file_description(path: &str) -> Option<String> {
    unsafe {
        let wpath = wide(path);
        let size = GetFileVersionInfoSizeW(wpath.as_ptr(), std::ptr::null_mut());
        if size == 0 {
            return None;
        }
        let mut data = vec![0u8; size as usize];
        if GetFileVersionInfoW(wpath.as_ptr(), 0, size, data.as_mut_ptr() as *mut c_void) == 0 {
            return None;
        }
        let mut ptr: *mut c_void = std::ptr::null_mut();
        let mut len = 0u32;
        let q = wide("\\VarFileInfo\\Translation");
        let mut langs = vec![(0x0409u16, 0x04B0u16)];
        if VerQueryValueW(data.as_ptr() as *const c_void, q.as_ptr(), &mut ptr, &mut len) != 0 && len >= 4 {
            let pairs = std::slice::from_raw_parts(ptr as *const u16, (len / 2) as usize);
            langs = pairs.chunks(2).filter(|c| c.len() == 2).map(|c| (c[0], c[1])).chain(langs).collect();
        }
        for (lang, cp) in langs {
            let q = wide(&format!("\\StringFileInfo\\{lang:04x}{cp:04x}\\FileDescription"));
            if VerQueryValueW(data.as_ptr() as *const c_void, q.as_ptr(), &mut ptr, &mut len) != 0 && len > 1 {
                let s = from_wide(std::slice::from_raw_parts(ptr as *const u16, len as usize));
                let s = s.trim().to_string();
                if !s.is_empty() {
                    return Some(s);
                }
            }
        }
        None
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunningApp {
    pub label: String,
    pub process_name: String,
    pub path: String,
    pub title: String,
}

/// Apps with a window on the taskbar, one entry per executable, sorted by name.
pub fn running_apps() -> Vec<RunningApp> {
    unsafe extern "system" fn each(hwnd: HWND, data: LPARAM) -> i32 {
        let out = &mut *(data as *mut Vec<HWND>);
        if IsWindowVisible(hwnd) == 0 || !GetWindow(hwnd, GW_OWNER).is_null() {
            return 1;
        }
        if GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW != 0 || GetWindowTextLengthW(hwnd) == 0 {
            return 1;
        }
        let mut cloaked = 0u32;
        DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED as u32, &mut cloaked as *mut u32 as *mut c_void, 4);
        if cloaked == 0 {
            out.push(hwnd);
        }
        1
    }
    let mut windows: Vec<HWND> = Vec::new();
    unsafe { EnumWindows(Some(each), &mut windows as *mut _ as LPARAM) };

    let me = unsafe { GetCurrentProcessId() };
    let mut apps: Vec<RunningApp> = Vec::new();
    for hwnd in windows {
        let pid = window_pid(hwnd);
        if pid == me {
            continue;
        }
        let Some(path) = process_path(pid) else { continue };
        if apps.iter().any(|a| a.path.eq_ignore_ascii_case(&path)) {
            continue;
        }
        let mut buf = [0u16; 256];
        let n = unsafe { GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32) }.max(0) as usize;
        let title = String::from_utf16_lossy(&buf[..n]);
        let process_name = exe_stem(&path);
        let label = file_description(&path).unwrap_or_else(|| process_name.clone());
        apps.push(RunningApp { label, process_name, path, title });
    }
    apps.sort_by_key(|a| a.label.to_lowercase());
    apps
}

// ───────────── hotkey names ─────────────

pub fn hotkey_label(mods: u32, vk: u32) -> String {
    let mut parts: Vec<String> = Vec::new();
    for (bit, name) in [(2, "Ctrl"), (1, "Alt"), (4, "Shift"), (8, "Win")] {
        if mods & bit != 0 {
            parts.push(name.into());
        }
    }
    parts.push(key_name(vk));
    parts.join(" + ")
}

fn key_name(vk: u32) -> String {
    match vk {
        0x30..=0x39 | 0x41..=0x5A => char::from(vk as u8).to_string(),
        0x70..=0x87 => format!("F{}", vk - 0x6F),
        0x20 => "Space".into(),
        0x25 => "Left".into(),
        0x26 => "Up".into(),
        0x27 => "Right".into(),
        0x28 => "Down".into(),
        0x60..=0x69 => format!("Num {}", vk - 0x60),
        _ => unsafe {
            let mut scan = MapVirtualKeyW(vk, MAPVK_VK_TO_VSC);
            // navigation keys live on the extended half of the keyboard
            if matches!(vk, 0x21..=0x2E | 0x5B | 0x5C | 0x6F | 0x90) {
                scan |= 0x100;
            }
            let mut buf = [0u16; 64];
            let n = GetKeyNameTextW((scan << 16) as i32, buf.as_mut_ptr(), buf.len() as i32);
            if n > 0 {
                String::from_utf16_lossy(&buf[..n as usize])
            } else {
                format!("Key {vk}")
            }
        },
    }
}

// ───────────── start with Windows ─────────────

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";

/// Writes or removes the `HKCU\…\Run` entry (and the old "ScreenShade" one).
pub fn set_startup(enabled: bool, hidden: bool) -> Result<(), String> {
    unsafe {
        let mut key: HKEY = std::ptr::null_mut();
        let sub = wide(RUN_KEY);
        let err = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            sub.as_ptr(),
            0,
            std::ptr::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            std::ptr::null(),
            &mut key,
            std::ptr::null_mut(),
        );
        if err != 0 {
            return Err(format!("Couldn't open the startup list (error {err})."));
        }
        RegDeleteValueW(key, wide("ScreenShade").as_ptr());
        let name = wide("dimscreen");
        let result = if enabled {
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let cmd = format!("\"{}\"{}", exe.display(), if hidden { " --minimized" } else { "" });
            let data = wide(&cmd);
            let err = RegSetValueExW(key, name.as_ptr(), 0, REG_SZ, data.as_ptr() as *const u8, (data.len() * 2) as u32);
            if err == 0 { Ok(()) } else { Err(format!("Couldn't add dimscreen to startup (error {err}).")) }
        } else {
            RegDeleteValueW(key, name.as_ptr());
            Ok(())
        };
        RegCloseKey(key);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_shortcuts() {
        assert_eq!(hotkey_label(3, 0x44), "Ctrl + Alt + D");
        assert_eq!(hotkey_label(4 | 8, 0x71), "Shift + Win + F2");
        assert_eq!(hotkey_label(2, 0x26), "Ctrl + Up");
    }

    #[test]
    fn finds_monitors_and_apps() {
        let mons = monitors();
        assert!(!mons.is_empty());
        assert!(mons.iter().all(|m| m.device.starts_with(r"\\.\DISPLAY") && m.width > 0));
        let apps = running_apps();
        for a in &apps {
            println!("{} · {}.exe · {}", a.label, a.process_name, a.title);
        }
        assert!(apps.iter().all(|a| !a.process_name.is_empty() && !a.path.is_empty()));
    }
}
