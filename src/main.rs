//! dimscreen: a tinted, adjustable shade over one or every display.
//!
//! `engine` draws the shade on its own thread; this file owns the settings, the tray icon and the
//! settings window, and turns every change into an `engine::Config`.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod engine;
mod settings;
mod sys;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde::Serialize;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder, WindowEvent, Wry};

use engine::{Config, Engine, Event, Status};
use settings::{Settings, ALL_DISPLAYS};

const TRAY: &str = "dimscreen";
static TOGGLE_ITEM: OnceLock<MenuItem<Wry>> = OnceLock::new();
static SEEN_STATUS: AtomicBool = AtomicBool::new(false);

struct App {
    settings: Mutex<Settings>,
    status: Mutex<Status>,
    /// A problem worth showing in the window (the startup entry couldn't be written, say).
    note: Mutex<Option<String>>,
    hotkey_paused: AtomicBool,
    dirty: AtomicBool,
    engine: Engine,
}

/// Everything the window shows.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    settings: Settings,
    status: Status,
    hotkey_label: String,
    displays: Vec<sys::Monitor>,
    note: Option<String>,
    version: &'static str,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn config(s: &Settings, hotkey_paused: bool) -> Config {
    Config {
        on: s.enabled,
        alpha: (u32::from(s.strength) * 255 / 100) as u8,
        color: s.colorref(),
        display: if s.display_target == ALL_DISPLAYS { String::new() } else { s.display_target.clone() },
        target: s.target_process_name.clone(),
        click_through: s.click_through,
        hotkey: (s.hotkey_modifiers, s.hotkey_key),
        hotkey_paused,
    }
}

fn snapshot(app: &AppHandle) -> Snapshot {
    let st = app.state::<App>();
    let s = lock(&st.settings).clone();
    let status = *lock(&st.status);
    let note = lock(&st.note).clone();
    Snapshot {
        hotkey_label: sys::hotkey_label(s.hotkey_modifiers, s.hotkey_key),
        settings: s,
        status,
        displays: sys::monitors(),
        note,
        version: env!("CARGO_PKG_VERSION"),
    }
}

/// Sends the settings to the shade and refreshes the tray and the window.
fn push(app: &AppHandle) {
    let st = app.state::<App>();
    let cfg = config(&lock(&st.settings), st.hotkey_paused.load(Ordering::Relaxed));
    st.engine.apply(cfg);
    refresh(app);
}

fn refresh(app: &AppHandle) {
    let st = app.state::<App>();
    let s = lock(&st.settings).clone();
    let status = *lock(&st.status);
    let state = if !s.enabled {
        "Off".to_string()
    } else if status.waiting {
        format!("Waiting for {}", app_label(&s))
    } else {
        format!("On · {}%", s.strength)
    };
    if let Some(tray) = app.tray_by_id(TRAY) {
        let _ = tray.set_tooltip(Some(format!("dimscreen · {state}")));
        let _ = tray.set_icon(Some(if s.enabled {
            tauri::include_image!("icons/tray-on.png")
        } else {
            tauri::include_image!("icons/tray-off.png")
        }));
    }
    if let Some(item) = TOGGLE_ITEM.get() {
        let _ = item.set_text(if s.enabled { "Turn shade off" } else { "Turn shade on" });
    }
    let _ = app.emit("state", snapshot(app));
}

fn app_label(s: &Settings) -> String {
    if s.target_app_label.is_empty() {
        format!("{}.exe", s.target_process_name)
    } else {
        s.target_app_label.clone()
    }
}

/// Applies a change to the settings, saves it soon, and pushes it out.
fn change(app: &AppHandle, f: impl FnOnce(&mut Settings)) {
    let st = app.state::<App>();
    let startup = {
        let mut s = lock(&st.settings);
        let before = (s.launch_at_startup, s.start_hidden);
        f(&mut s);
        s.sanitize();
        let after = (s.launch_at_startup, s.start_hidden);
        (before != after).then_some(after)
    };
    if let Some((on, hidden)) = startup {
        *lock(&st.note) = sys::set_startup(on, hidden).err();
    }
    st.dirty.store(true, Ordering::Relaxed);
    push(app);
}

fn toggle_shade(app: &AppHandle) {
    change(app, |s| s.enabled = !s.enabled);
}

fn save_now(app: &AppHandle) {
    let st = app.state::<App>();
    if st.dirty.swap(false, Ordering::Relaxed) {
        let s = lock(&st.settings).clone();
        if let Err(e) = settings::save(&s) {
            *lock(&st.note) = Some(format!("Couldn't save settings: {e}"));
        }
    }
}

// ───────────── commands for the window ─────────────

#[tauri::command]
fn get_state(app: AppHandle) -> Snapshot {
    snapshot(&app)
}

/// Merges `patch` (PascalCase keys, as in settings.json) into the settings.
#[tauri::command]
fn update(app: AppHandle, patch: serde_json::Value) -> Result<Snapshot, String> {
    let next = {
        let st = app.state::<App>();
        let current = lock(&st.settings).clone();
        let mut value = serde_json::to_value(&current).map_err(|e| e.to_string())?;
        if let (Some(obj), Some(p)) = (value.as_object_mut(), patch.as_object()) {
            for (k, v) in p {
                obj.insert(k.clone(), v.clone());
            }
        }
        serde_json::from_value::<Settings>(value).map_err(|e| e.to_string())?
    };
    change(&app, |s| *s = next);
    Ok(snapshot(&app))
}

#[tauri::command]
fn toggle(app: AppHandle) -> Snapshot {
    toggle_shade(&app);
    snapshot(&app)
}

#[tauri::command]
async fn running_apps() -> Vec<sys::RunningApp> {
    sys::running_apps()
}

/// Lets the window record a new shortcut without the current one firing.
#[tauri::command]
fn pause_hotkey(app: AppHandle, paused: bool) {
    app.state::<App>().hotkey_paused.store(paused, Ordering::Relaxed);
    push(&app);
}

// ───────────── window and tray ─────────────

fn open_settings(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let built = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("dimscreen")
        .inner_size(900.0, 660.0)
        .min_inner_size(400.0, 480.0)
        .center()
        .decorations(false)
        .shadow(true)
        .maximizable(false)
        .visible(false)
        .build();
    let Ok(w) = built else { return };
    let hwnd = w.hwnd().map(|h| h.0 as isize).unwrap_or(0);
    let handle = app.clone();
    w.on_window_event(move |event| {
        let st = handle.state::<App>();
        match event {
            WindowEvent::Focused(focused) => st.engine.settings_window(hwnd, *focused),
            WindowEvent::Destroyed => {
                st.engine.settings_window(0, false);
                if st.hotkey_paused.swap(false, Ordering::Relaxed) {
                    push(&handle);
                }
            }
            _ => {}
        }
    });
    // the page shows the window once it has painted; this is the fallback if it never does
    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(1500));
        if let Some(w) = handle.get_webview_window("main") {
            if !w.is_visible().unwrap_or(true) {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }
    });
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let toggle = MenuItem::with_id(app, "toggle", "Turn shade on", true, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", "Open dimscreen", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit dimscreen", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&toggle, &open, &sep, &quit])?;
    let _ = TOGGLE_ITEM.set(toggle);
    TrayIconBuilder::with_id(TRAY)
        .icon(tauri::include_image!("icons/tray-off.png"))
        .tooltip("dimscreen")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "toggle" => toggle_shade(app),
            "open" => open_settings(app),
            "quit" => {
                save_now(app);
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                open_settings(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

fn on_engine(app: &AppHandle, event: Event) {
    let Some(st) = app.try_state::<App>() else { return };
    match event {
        Event::Hotkey => toggle_shade(app),
        Event::Status(status) => {
            *lock(&st.status) = status;
            let first = !SEEN_STATUS.swap(true, Ordering::Relaxed);
            // a shortcut that's already taken at launch: show the window so it can be changed
            if first && !status.hotkey_ok {
                let handle = app.clone();
                let _ = app.run_on_main_thread(move || open_settings(&handle));
            }
            refresh(app);
        }
        Event::DisplaysChanged => refresh(app),
    }
}

fn main() {
    // Per-monitor DPI before any window exists, so the shade covers monitors in real pixels.
    unsafe {
        windows_sys::Win32::UI::HiDpi::SetProcessDpiAwarenessContext(
            windows_sys::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
        );
    }
    let minimized = std::env::args().any(|a| a.eq_ignore_ascii_case("--minimized"));

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| open_settings(app)))
        .invoke_handler(tauri::generate_handler![get_state, update, toggle, running_apps, pause_hotkey])
        .setup(move |app| {
            let (settings, first_run) = settings::load();
            let note = if settings.launch_at_startup {
                sys::set_startup(true, settings.start_hidden).err()
            } else {
                None
            };
            let handle = app.handle().clone();
            let engine = Engine::start(move |e| on_engine(&handle, e));
            app.manage(App {
                settings: Mutex::new(settings),
                status: Mutex::new(Status::default()),
                note: Mutex::new(note),
                hotkey_paused: AtomicBool::new(false),
                dirty: AtomicBool::new(first_run),
                engine,
            });
            build_tray(app.handle())?;
            push(app.handle());

            // settings are written at most every half second, so dragging the slider stays cheap
            let handle = app.handle().clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(Duration::from_millis(500));
                save_now(&handle);
            });

            if first_run || !minimized {
                open_settings(app.handle());
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("start dimscreen");

    app.run(|app, event| match event {
        // closing the window keeps dimscreen in the tray; only Quit ends it
        RunEvent::ExitRequested { code: None, api, .. } => api.prevent_exit(),
        RunEvent::Exit => save_now(app),
        _ => {}
    });
}
