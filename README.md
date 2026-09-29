# dimscreen

dimscreen is a portable Windows desktop dimmer. It places a tinted, adjustable overlay across one or every display and keeps it above ordinary application windows.

## Run it

Open `dimscreen.exe`. The first launch opens settings with the shade off. Click **Shade Off** to enable it. The app then stays in the system tray when its settings window is closed.

The default global shortcut is **Ctrl + Alt + D**. Double-click the tray icon to open settings. Right-click it to toggle the shade or exit.

## Customize it

- Set shade strength from 0% to full black.
- Pick a tint or any color.
- Shade all displays or choose one display.
- Choose **All apps** or a running app. The shade appears while that app is in the foreground and hides when you switch away.
- Change the global shortcut by clicking its button and pressing a key combination.
- Let clicks pass through the overlay, start with Windows, and choose whether startup opens the settings window.

Settings are saved in `%APPDATA%\dimscreen\settings.json`.

## Coverage limits

The overlay covers the normal Windows desktop, including the taskbar and ordinary application windows. Windows secure desktop screens, such as some UAC prompts and the lock screen, and some exclusive full-screen content can appear above it.
