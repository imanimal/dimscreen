# dimscreen

dimscreen is a portable Windows screen dimmer. It lays a tinted, adjustable shade over one display or all of them and keeps it above your other windows.

It's written in Rust. The shade is a set of plain Win32 overlay windows run on their own thread, and the settings window is a small Tauri webview. It ships as one `.exe` of under 4 MB.

## Run it

Open `dimscreen.exe`. The settings window opens, and the shade's on/off state is restored from last time. Closing the window keeps dimscreen in the system tray.

- **Ctrl + Alt + D** turns the shade on and off from any app. You can change it under **Behavior › Shortcut**.
- **Tray icon:** left-click opens settings. Right-click turns the shade on or off, or quits.

## What you can set

- **Strength:** 0–100 %, with Soft, Focus and Night presets.
- **Tint:** Ink, Warm amber, Blue hour, Rose dusk, or any custom color.
- **Where:** every display, or one display you pick on the map. You can also limit it to one app, so the shade shows only while that app is in front. Switching to dimscreen, the taskbar, Start or Alt+Tab doesn't count as leaving the app.
- **Behavior:** let clicks pass through the shade, start with Windows, start in the tray.
- **Look:** Standard (a plain modern UI) or Pocket (the Pocket UI kit's handheld look), each in System, Light or Dark.

Settings are saved in `%APPDATA%\dimscreen\settings.json`. Settings from the earlier C# version carry over.

## Screenshots

The shade is excluded from screen capture (`WDA_EXCLUDEFROMCAPTURE`, Windows 10 2004 or later). Screenshots, the Snipping Tool and screen recordings show your screen undimmed, and the shade no longer dims a capture of itself a second time.

While the Snipping Tool's overlay is up (Win+Shift+S or Print Screen), the shade hides, so you pick your snip from an undimmed screen. It fades back in when the overlay closes.

## Coverage limits

The shade covers the desktop, the taskbar and ordinary windows, including other always-on-top windows. Windows' secure screens can appear above it: UAC prompts, the lock screen, and Ctrl+Alt+Del. Some exclusive full-screen games can too.

## Build

Requires Rust (stable, MSVC toolchain) and the WebView2 runtime, which is built into Windows 11.

```
cargo build --release
```

The app is `target/release/dimscreen.exe`. `cargo test` runs the unit tests, and `python tools/make_icons.py` redraws the icons.

The bundled fonts in `ui/fonts` (Space Grotesk, Space Mono, Doto) are used by the Pocket look. They're licensed under the SIL Open Font License 1.1 (see `ui/fonts/OFL.txt`).
