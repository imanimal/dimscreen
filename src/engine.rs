//! The shade itself: one layered, always-on-top window per display, owned by a thread with its own
//! message loop so the settings UI can never stall it.
//!
//! - Every shade window is excluded from screen capture (`WDA_EXCLUDEFROMCAPTURE`), so screenshots,
//!   the Snipping Tool's frozen frame and screen recordings show the screen without the dim. Before
//!   this, a screenshot captured the shade and the shade then dimmed that capture a second time.
//! - It stays on top: re-raised when the foreground window changes and once a second.
//! - It follows the display layout: rebuilt after monitors change, resolution changes and resume.
//! - With a target app it shows only while that app is in front. Switching to dimscreen itself, the
//!   taskbar, Start or Alt+Tab keeps the current state instead of flickering.

use std::cell::RefCell;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, FillRect, InvalidateRect, PAINTSTRUCT,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::GetCurrentProcessId;
use windows_sys::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{RegisterHotKey, UnregisterHotKey, MOD_NOREPEAT};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::sys::{self, Monitor};

/// What the shade should do. `main.rs` builds it from the settings.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Config {
    pub on: bool,
    pub alpha: u8,
    /// COLORREF 0x00BBGGRR
    pub color: u32,
    /// A monitor's device name, or empty for every display.
    pub display: String,
    /// The exe name (without `.exe`) to follow, or empty for every app.
    pub target: String,
    pub click_through: bool,
    pub hotkey: (u32, u32),
    /// Off while the settings window records a new shortcut.
    pub hotkey_paused: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// The shade is on screen (or fading in).
    pub shown: bool,
    /// On, but the target app isn't in front.
    pub waiting: bool,
    pub hotkey_ok: bool,
}

pub enum Event {
    Hotkey,
    Status(Status),
    DisplaysChanged,
}

enum Msg {
    Config(Config),
    Settings { hwnd: isize, focused: bool },
}

pub struct Engine {
    tx: Sender<Msg>,
    host: isize,
}

const WM_WAKE: u32 = WM_APP + 1;
const T_RAISE: usize = 1;
const T_FADE: usize = 2;
const T_LAYOUT: usize = 3;
const FADE_TICK_MS: u32 = 15;
const FADE_STEPS: f32 = 10.0;

/// Window classes that take the foreground without meaning "I switched apps".
const SHELL_CLASSES: &[&str] = &[
    "Shell_TrayWnd",
    "Shell_SecondaryTrayWnd",
    "NotifyIconOverflowWindow",
    "TopLevelWindowForOverflowXamlIsland",
    "XamlExplorerHostIslandWindow",
    "MultitaskingViewFrame",
    "TaskSwitcherWnd",
    "ForegroundStaging",
    "Windows.UI.Core.CoreWindow",
];

static TINT: AtomicU32 = AtomicU32::new(0);

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
    static ON_EVENT: RefCell<Option<Box<dyn Fn(Event)>>> = const { RefCell::new(None) };
}

impl Engine {
    pub fn start(on_event: impl Fn(Event) + Send + 'static) -> Engine {
        let (tx, rx) = channel();
        let (host_tx, host_rx) = channel();
        std::thread::Builder::new()
            .name("shade".into())
            .spawn(move || run(rx, host_tx, Box::new(on_event)))
            .expect("start the shade thread");
        let host = host_rx.recv().unwrap_or(0);
        Engine { tx, host }
    }

    pub fn apply(&self, cfg: Config) {
        self.post(Msg::Config(cfg));
    }

    /// The settings window, so it can sit above the shade while it has focus.
    pub fn settings_window(&self, hwnd: isize, focused: bool) {
        self.post(Msg::Settings { hwnd, focused });
    }

    fn post(&self, msg: Msg) {
        let _ = self.tx.send(msg);
        unsafe { PostMessageW(self.host as HWND, WM_WAKE, 0, 0) };
    }
}

struct State {
    host: HWND,
    rx: Receiver<Msg>,
    cfg: Config,
    shades: Vec<HWND>,
    /// The display selection, click-through and monitor layout the shades were built for.
    built_for: String,
    /// The target app is in front (always true without a target).
    target_front: bool,
    /// The alpha on screen right now; fades toward `desired()`.
    alpha_now: f32,
    fading: bool,
    hotkey_now: Option<(u32, u32)>,
    hotkey_ok: bool,
    settings: HWND,
    settings_focused: bool,
    status: Option<Status>,
}

fn run(rx: Receiver<Msg>, host_tx: Sender<isize>, on_event: Box<dyn Fn(Event)>) {
    unsafe {
        let inst = GetModuleHandleW(std::ptr::null());
        let host_class = sys::wide("dimscreen.host");
        let shade_class = sys::wide("dimscreen.shade");
        let mut wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(host_proc),
            hInstance: inst,
            lpszClassName: host_class.as_ptr(),
            ..Default::default()
        };
        RegisterClassExW(&wc);
        wc.lpfnWndProc = Some(shade_proc);
        wc.lpszClassName = shade_class.as_ptr();
        wc.hCursor = LoadCursorW(std::ptr::null_mut(), IDC_ARROW);
        RegisterClassExW(&wc);

        // A hidden top-level window (not message-only) so display and power broadcasts reach it.
        let host = CreateWindowExW(
            WS_EX_TOOLWINDOW,
            host_class.as_ptr(),
            sys::wide("dimscreen").as_ptr(),
            WS_POPUP,
            0,
            0,
            0,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            inst,
            std::ptr::null(),
        );
        ON_EVENT.with(|e| *e.borrow_mut() = Some(on_event));
        STATE.with(|s| {
            *s.borrow_mut() = Some(State {
                host,
                rx,
                cfg: Config::default(),
                shades: Vec::new(),
                built_for: String::new(),
                target_front: true,
                alpha_now: 0.0,
                fading: false,
                hotkey_now: None,
                hotkey_ok: true,
                settings: std::ptr::null_mut(),
                settings_focused: false,
                status: None,
            })
        });
        let _ = host_tx.send(host as isize);

        let hook: HWINEVENTHOOK = SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            std::ptr::null_mut(),
            Some(on_foreground),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        );
        SetTimer(host, T_RAISE, 1000, None);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        UnhookWinEvent(hook);
    }
}

/// Runs `f` on the state, then delivers the events it raised (outside the borrow, so a handler can
/// call back into the engine).
fn with(f: impl FnOnce(&mut State, &mut Vec<Event>)) {
    let mut events = Vec::new();
    STATE.with(|cell| {
        if let Ok(mut guard) = cell.try_borrow_mut() {
            if let Some(state) = guard.as_mut() {
                f(state, &mut events);
            }
        }
    });
    if !events.is_empty() {
        ON_EVENT.with(|cb| {
            if let Some(cb) = cb.borrow().as_ref() {
                for e in events {
                    cb(e);
                }
            }
        });
    }
}

unsafe extern "system" fn host_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_WAKE => with(|s, ev| s.drain(ev)),
        WM_HOTKEY => with(|_, ev| ev.push(Event::Hotkey)),
        WM_TIMER => match wp {
            T_RAISE => with(|s, ev| s.tick(ev)),
            T_FADE => with(|s, ev| s.fade_step(ev)),
            T_LAYOUT => {
                KillTimer(hwnd, T_LAYOUT);
                with(|s, ev| {
                    s.sync(ev);
                    s.raise();
                    ev.push(Event::DisplaysChanged);
                });
            }
            _ => {}
        },
        // monitors come and go in bursts; rebuild once they settle
        WM_DISPLAYCHANGE => {
            SetTimer(hwnd, T_LAYOUT, 500, None);
        }
        WM_POWERBROADCAST if wp == PBT_APMRESUMEAUTOMATIC as usize => {
            SetTimer(hwnd, T_LAYOUT, 1500, None);
            return 1;
        }
        _ => return DefWindowProcW(hwnd, msg, wp, lp),
    }
    0
}

unsafe extern "system" fn shade_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            let brush = CreateSolidBrush(TINT.load(Ordering::Relaxed));
            FillRect(hdc, &ps.rcPaint, brush);
            DeleteObject(brush);
            EndPaint(hwnd, &ps);
            0
        }
        WM_MOUSEACTIVATE => MA_NOACTIVATE as LRESULT,
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

unsafe extern "system" fn on_foreground(_: HWINEVENTHOOK, _: u32, hwnd: HWND, _: i32, _: i32, _: u32, _: u32) {
    with(|s, ev| s.foreground_changed(hwnd, ev));
}

impl State {
    fn drain(&mut self, ev: &mut Vec<Event>) {
        let mut changed = false;
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                Msg::Config(cfg) => {
                    if cfg.target != self.cfg.target {
                        self.target_front = cfg.target.is_empty();
                        self.cfg.target = cfg.target.clone();
                        self.check_foreground(unsafe { GetForegroundWindow() });
                    }
                    self.cfg = cfg;
                    changed = true;
                }
                Msg::Settings { hwnd, focused } => self.set_settings(hwnd as HWND, focused),
            }
        }
        if changed {
            self.sync(ev);
        }
    }

    /// Brings the windows, hotkey and alpha in line with `cfg`.
    fn sync(&mut self, ev: &mut Vec<Event>) {
        self.sync_hotkey();

        let monitors = sys::monitors();
        let picked: Vec<&Monitor> = match monitors.iter().filter(|m| m.device == self.cfg.display).collect::<Vec<_>>() {
            one if !one.is_empty() => one,
            _ => monitors.iter().collect(),
        };
        let sig = format!(
            "{}|{}",
            self.cfg.click_through,
            picked.iter().map(|m| format!("{}:{},{},{},{}", m.device, m.x, m.y, m.width, m.height)).collect::<Vec<_>>().join(";")
        );
        if sig != self.built_for {
            self.rebuild(&picked);
            self.built_for = sig;
        }

        if TINT.swap(self.cfg.color, Ordering::Relaxed) != self.cfg.color {
            for &w in &self.shades {
                unsafe { InvalidateRect(w, std::ptr::null(), 0) };
            }
        }
        self.retarget(ev);
    }

    fn sync_hotkey(&mut self) {
        let want = (!self.cfg.hotkey_paused).then_some(self.cfg.hotkey);
        if want == self.hotkey_now {
            return;
        }
        unsafe {
            if self.hotkey_now.is_some() {
                UnregisterHotKey(self.host, 1);
            }
            if let Some((mods, vk)) = want {
                self.hotkey_ok = RegisterHotKey(self.host, 1, mods | MOD_NOREPEAT, vk) != 0;
            }
        }
        self.hotkey_now = want;
    }

    fn rebuild(&mut self, picked: &[&Monitor]) {
        unsafe {
            for w in self.shades.drain(..) {
                DestroyWindow(w);
            }
            let class = sys::wide("dimscreen.shade");
            let mut ex = WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST;
            if self.cfg.click_through {
                ex |= WS_EX_TRANSPARENT;
            }
            for m in picked {
                let r = m.rect();
                let w = CreateWindowExW(
                    ex,
                    class.as_ptr(),
                    std::ptr::null(),
                    WS_POPUP,
                    r.left,
                    r.top,
                    r.right - r.left,
                    r.bottom - r.top,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    GetModuleHandleW(std::ptr::null()),
                    std::ptr::null(),
                );
                if w.is_null() {
                    continue;
                }
                SetLayeredWindowAttributes(w, 0, 0, LWA_ALPHA);
                // the screenshot fix: capture APIs skip this window entirely
                SetWindowDisplayAffinity(w, WDA_EXCLUDEFROMCAPTURE);
                self.shades.push(w);
            }
        }
        self.show_alpha();
    }

    /// The alpha the shade should have right now.
    fn desired(&self) -> f32 {
        if self.cfg.on && self.target_front {
            f32::from(self.cfg.alpha)
        } else {
            0.0
        }
    }

    /// Moves toward the desired alpha: a short fade when the shade appears or leaves, instant when
    /// only the strength changes (so the slider tracks the hand).
    fn retarget(&mut self, ev: &mut Vec<Event>) {
        let want = self.desired();
        let appearing_or_leaving = (self.alpha_now == 0.0) != (want == 0.0);
        if appearing_or_leaving || self.fading {
            if !self.fading {
                self.fading = true;
                unsafe { SetTimer(self.host, T_FADE, FADE_TICK_MS, None) };
            }
        } else {
            self.alpha_now = want;
            self.show_alpha();
        }
        self.report(ev);
    }

    fn fade_step(&mut self, ev: &mut Vec<Event>) {
        let want = self.desired();
        let step = (f32::from(self.cfg.alpha).max(want).max(self.alpha_now) / FADE_STEPS).max(1.0);
        self.alpha_now = if self.alpha_now < want {
            (self.alpha_now + step).min(want)
        } else {
            (self.alpha_now - step).max(want)
        };
        if self.alpha_now == want {
            self.fading = false;
            unsafe { KillTimer(self.host, T_FADE) };
        }
        self.show_alpha();
        self.report(ev);
    }

    fn show_alpha(&self) {
        let a = self.alpha_now.round().clamp(0.0, 255.0) as u8;
        unsafe {
            for &w in &self.shades {
                if a == 0 {
                    ShowWindow(w, SW_HIDE);
                } else {
                    SetLayeredWindowAttributes(w, 0, a, LWA_ALPHA);
                    if IsWindowVisible(w) == 0 {
                        ShowWindow(w, SW_SHOWNOACTIVATE);
                    }
                }
            }
        }
        if a > 0 {
            self.raise();
        }
    }

    /// Puts the shade above every other always-on-top window, and the settings window above the
    /// shade while it has focus (so it stays readable even at full strength).
    fn raise(&self) {
        if self.alpha_now == 0.0 {
            return;
        }
        let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER;
        unsafe {
            for &w in &self.shades {
                SetWindowPos(w, HWND_TOPMOST, 0, 0, 0, 0, flags);
            }
            if !self.settings.is_null() && self.settings_focused {
                SetWindowPos(self.settings, HWND_TOPMOST, 0, 0, 0, 0, flags | SWP_ASYNCWINDOWPOS);
            }
        }
    }

    fn set_settings(&mut self, hwnd: HWND, focused: bool) {
        let was_on_top = !self.settings.is_null() && self.settings_focused;
        self.settings = hwnd;
        self.settings_focused = focused && !hwnd.is_null();
        if was_on_top && !self.settings_focused && !hwnd.is_null() {
            // drop back to a normal window, just behind whatever took focus
            unsafe {
                let front = GetForegroundWindow();
                let topmost = !front.is_null() && GetWindowLongPtrW(front, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST != 0;
                let after = if front.is_null() || front == hwnd || topmost { HWND_NOTOPMOST } else { front };
                SetWindowPos(hwnd, after, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS);
            }
        }
        self.raise();
    }

    fn foreground_changed(&mut self, hwnd: HWND, ev: &mut Vec<Event>) {
        self.check_foreground(hwnd);
        self.retarget(ev);
        self.raise();
    }

    /// Updates `target_front` from the window now in front.
    fn check_foreground(&mut self, hwnd: HWND) {
        if self.cfg.target.is_empty() {
            self.target_front = true;
            return;
        }
        if hwnd.is_null() || is_shell_window(hwnd) {
            return;
        }
        let pid = sys::window_pid(hwnd);
        if pid == 0 || pid == unsafe { GetCurrentProcessId() } {
            return;
        }
        self.target_front = sys::process_path(pid)
            .map(|p| sys::exe_stem(&p).eq_ignore_ascii_case(&self.cfg.target))
            .unwrap_or(false);
    }

    /// Once a second: stay on top, and re-check the front app in case an event was missed.
    fn tick(&mut self, ev: &mut Vec<Event>) {
        if !self.cfg.target.is_empty() {
            let before = self.target_front;
            self.check_foreground(unsafe { GetForegroundWindow() });
            if before != self.target_front {
                self.retarget(ev);
            }
        }
        self.raise();
    }

    fn report(&mut self, ev: &mut Vec<Event>) {
        let status = Status {
            shown: self.desired() > 0.0,
            waiting: self.cfg.on && !self.target_front,
            hotkey_ok: self.cfg.hotkey_paused || self.hotkey_ok,
        };
        if self.status != Some(status) {
            self.status = Some(status);
            ev.push(Event::Status(status));
        }
    }
}

fn is_shell_window(hwnd: HWND) -> bool {
    let mut buf = [0u16; 128];
    let n = unsafe { GetClassNameW(hwnd, buf.as_mut_ptr(), buf.len() as i32) }.max(0) as usize;
    let class = String::from_utf16_lossy(&buf[..n]);
    SHELL_CLASSES.contains(&class.as_str())
}
