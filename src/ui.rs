//! Native, event-driven UI. Hardware work stays in isolated helpers.
use mcc::{
    control::{FanMode, Switch},
    lighting::{Effect, Lighting},
    monitor::Monitor,
    presets::Preset,
    session::Session,
    settings::{PowerPlan, Settings},
};
use std::{
    cell::RefCell,
    ptr,
    sync::mpsc::{self, Receiver, SyncSender},
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        Controls::{
            DRAWITEMSTRUCT, Dialogs::*, ODS_DISABLED, ODS_FOCUS, ODS_NOFOCUSRECT, ODS_SELECTED,
        },
        HiDpi::GetDpiForWindow,
        Input::KeyboardAndMouse::EnableWindow,
        Shell::*,
        WindowsAndMessaging::*,
    },
};

const UPDATE: u32 = WM_APP + 1;
const TRAY: u32 = WM_APP + 2;
const CONNECT: u16 = 101;
const AUTO: u16 = 103;
const BOOST: u16 = 104;
const QUIET: u16 = 105;
const STARTUP: u16 = 124;
const CUSTOM: u16 = 125;
const REGULAR: u16 = 126;
const SAVE_CUSTOM: u16 = 127;
const APPLY_CUSTOM: u16 = 128;
const BALANCED: u16 = 106;
const PERFORMANCE: u16 = 107;
const APPLY_LIGHT: u16 = 108;
const SCREEN_OFF: u16 = 109;
const GPU: u16 = 110;
const KEY: u16 = 111;
const USB: u16 = 113;
const BAR: u16 = 115;
const QUIT: u16 = 120;
const GPU_TEMP: u16 = 121;
const RESTORE: u16 = 123;
const COOLING: u16 = 201;
const LIGHTING: u16 = 202;
const SETTINGS: u16 = 203;
const EFFECT: u16 = 210;
const BRIGHTNESS: u16 = 211;
const SPEED: u16 = 212;
const DIRECTION: u16 = 213;
const COLOR: u16 = 214;
const SWATCH: u16 = 220;
const PALETTE: [[u8; 3]; 6] = [
    [70, 160, 255],
    [143, 96, 220],
    [36, 174, 112],
    [239, 91, 91],
    [244, 177, 62],
    [240, 240, 240],
];
const fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    r as u32 | (g as u32) << 8 | (b as u32) << 16
}
const BG: COLORREF = rgb(246, 248, 247);
const INK: COLORREF = rgb(28, 40, 35);
const MUTED: COLORREF = rgb(105, 117, 111);
const GREEN: COLORREF = rgb(24, 114, 76);
const MINT: COLORREF = rgb(227, 243, 234);
const BORDER: COLORREF = rgb(219, 227, 222);
const WHITE: COLORREF = rgb(255, 255, 255);
const ERROR: COLORREF = rgb(163, 53, 44);
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
fn caption(hwnd: HWND, t: &str) {
    // SAFETY: Live UI-owned handle and NUL-terminated text copied by Windows.
    unsafe {
        SetWindowTextW(hwnd, wide(t).as_ptr());
    }
}
#[derive(Clone)]
enum Job {
    Connect,
    Status,
    Poll,
    ReconnectPoll,
    Preset(Preset),
    Fan(FanMode),
    Power(PowerPlan),
    Combination(FanMode, PowerPlan),
    Startup(bool),
    Switch(Switch, bool),
    Light(Lighting),
    Gpu,
    Restore,
    ResponsivenessTest,
}
struct Completed {
    status: Option<mcc::hardware::HardwareStatus>,
    observation: Option<mcc::hardware::Observation>,
    status_failed: bool,
    passive: bool,
    message: String,
    error: bool,
    lighting: Option<Lighting>,
    power: Option<PowerPlan>,
    power_refreshed: bool,
    stop_monitor: bool,
}
impl Completed {
    fn message(message: impl Into<String>) -> Self {
        Self {
            status: None,
            observation: None,
            status_failed: false,
            passive: false,
            message: message.into(),
            error: false,
            lighting: None,
            power: None,
            power_refreshed: false,
            stop_monitor: false,
        }
    }
}
fn call(args: &[String]) -> Result<serde_json::Value, String> {
    use std::os::windows::process::CommandExt;
    let path = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .with_file_name("mcc.exe");
    let mut cmd = std::process::Command::new(path);
    cmd.args(args).creation_flags(0x08000000);
    let bytes = mcc::process::run(&mut cmd, Duration::from_secs(5))
        .map_err(|e| format!("Request stopped: {e:?}. Checking current settings…"))?;
    let v: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if v["ok"] != true {
        return Err(v["error"].as_str().unwrap_or("Request failed").into());
    }
    Ok(v)
}
fn hardware_status() -> Result<mcc::hardware::HardwareStatus, String> {
    serde_json::from_value(call(&["hardware".into(), "status".into()])?["status"].clone())
        .map_err(|e| e.to_string())
}
fn fan_args(f: FanMode) -> Vec<String> {
    vec![
        "hardware".into(),
        if f == FanMode::Auto {
            "fan-auto"
        } else {
            "fan-boost"
        }
        .into(),
    ]
}
fn operation(job: &Job) -> Result<Completed, String> {
    let message = match job {
        Job::Connect => {
            let s = hardware_status()?;
            if !s.writes_allowed
                && s.ownership_reason
                    .as_deref()
                    .is_some_and(|r| r.starts_with("OEM service "))
            {
                call(&["hardware".into(), "pause-oem".into()])?;
            }
            "Controls enabled. Choose a workload preset; fan state stays synchronized."
        }
        Job::Status | Job::ReconnectPoll => "Settings refreshed.",
        Job::Poll => {
            let mut r = Completed::message("Fan state synchronized.");
            r.observation = Some(
                serde_json::from_value(
                    call(&["hardware".into(), "observe".into()])?["observation"].clone(),
                )
                .map_err(|e| e.to_string())?,
            );
            if let Some(o) = &r.observation {
                if !o.writes_allowed {
                    r.error = true;
                    r.message = o
                        .ownership_reason
                        .clone()
                        .unwrap_or("Original controller has control.".into());
                }
            }
            return Ok(r);
        }
        Job::Startup(on) => {
            mcc::startup::set(*on)?;
            if *on {
                "Starts in the tray when you sign in. Hardware control still requires Enable controls."
            } else {
                "Windows startup disabled."
            }
        }
        Job::Fan(mode) => {
            mcc::presets::apply_combination(
                Some(*mode),
                None,
                |f| call(&fan_args(f)).map(|_| ()),
                mcc::desktop::power,
            )?;
            "Fan cooling updated. Windows performance setting was preserved."
        }
        Job::Power(plan) => {
            mcc::presets::apply_combination(
                None,
                Some(*plan),
                |f| call(&fan_args(f)).map(|_| ()),
                mcc::desktop::power,
            )?;
            "Windows performance updated. Fan mode was preserved."
        }
        Job::Combination(fan, power) => {
            mcc::presets::apply_combination(
                Some(*fan),
                Some(*power),
                |f| call(&fan_args(f)).map(|_| ()),
                mcc::desktop::power,
            )?;
            "Custom fan/performance combination applied. Lighting was preserved."
        }
        Job::Preset(p) => {
            mcc::presets::apply(
                *p,
                |f| call(&fan_args(f)).map(|_| ()),
                |power| {
                    call(&[
                        "power".into(),
                        match power {
                            PowerPlan::PowerSaver => "saver",
                            PowerPlan::Balanced => "balanced",
                            PowerPlan::HighPerformance => "performance",
                        }
                        .into(),
                    ])
                    .map(|_| ())
                },
            )?;
            match p {
                Preset::Office => {
                    "Office: adaptive fans + Windows Power Saver. Less CPU power; fan speed still follows heat."
                }
                Preset::Balanced => {
                    "Balanced: adaptive fans + Windows Balanced. Cooling follows workload."
                }
                Preset::High => {
                    "High: adaptive fans + Windows High Performance. May generate more heat and noise."
                }
                Preset::Turbo => {
                    "Turbo: firmware cooling boost. Pressing the laptop button updates this app automatically."
                }
            }
        }
        Job::Switch(s, on) => {
            let name = match (s, on) {
                (Switch::WindowsKeyLock, true) => "key-lock-on",
                (Switch::WindowsKeyLock, false) => "key-lock-off",
                (Switch::UsbCharging, true) => "usb-on",
                (Switch::UsbCharging, false) => "usb-off",
                (Switch::LightBar, true) => "light-bar-on",
                (Switch::LightBar, false) => "light-bar-off",
            };
            call(&["hardware".into(), name.into()])?;
            match s {
                Switch::LightBar => {
                    if *on {
                        "Light bar is on."
                    } else {
                        "Light bar is off."
                    }
                }
                Switch::UsbCharging => {
                    if *on {
                        "USB charging is on."
                    } else {
                        "USB charging is off."
                    }
                }
                Switch::WindowsKeyLock => {
                    if *on {
                        "Windows key is locked."
                    } else {
                        "Windows key is unlocked."
                    }
                }
            }
        }
        Job::Light(c) => {
            call(&[
                "lighting".into(),
                match c.effect {
                    Effect::Off => "off",
                    Effect::Solid => "solid",
                    Effect::Breathing => "breathing",
                    Effect::Rainbow => "rainbow",
                    Effect::Wave => "wave",
                    Effect::Reactive => "reactive",
                }
                .into(),
                c.brightness.to_string(),
                c.speed.to_string(),
                format!("{:02X}{:02X}{:02X}", c.rgb[0], c.rgb[1], c.rgb[2]),
                c.direction.to_string(),
            ])?;
            let mut r = Completed::message(
                "Keyboard style sent. Check the keyboard to confirm its appearance.",
            );
            r.lighting = Some(c.clone());
            return Ok(r);
        }
        Job::Gpu => {
            let v = call(&["gpu".into()])?;
            return Ok(Completed::message(format!(
                "GPU: {} °C · one sample. Continuous GPU polling is off.",
                v["gpu_temperature_c"]
            )));
        }
        Job::Restore => {
            call(&fan_args(FanMode::Auto))?;
            call(&["hardware".into(), "resume-oem".into()])?;
            "Automatic cooling restored. The original controller can now be opened."
        }
        Job::ResponsivenessTest => {
            std::thread::sleep(Duration::from_millis(500));
            return Ok(Completed::message("Synthetic worker completed."));
        }
    };
    Ok(Completed::message(message))
}
fn worker(job: Job) -> Completed {
    let mut r = worker_with(job, operation, hardware_status);
    r.power = mcc::desktop::active_power();
    r.power_refreshed = true;
    r
}
fn worker_with(
    job: Job,
    apply: impl FnOnce(&Job) -> Result<Completed, String>,
    status: impl FnOnce() -> Result<mcc::hardware::HardwareStatus, String>,
) -> Completed {
    let needs_status = matches!(
        job,
        Job::Connect
            | Job::Status
            | Job::ReconnectPoll
            | Job::Preset(_)
            | Job::Fan(_)
            | Job::Combination(_, _)
            | Job::Switch(_, _)
            | Job::Restore
    );
    let mut r = apply(&job).unwrap_or_else(|e| {
        let mut r = Completed::message(e);
        r.error = true;
        r
    });
    r.passive = matches!(job, Job::Poll | Job::ReconnectPoll);
    if matches!(job, Job::Poll) && r.error && r.observation.is_none() {
        r.status_failed = true;
    }
    r.stop_monitor = matches!(job, Job::Restore) && !r.error;
    // Recover by reading, never by replaying a possibly completed write.
    if needs_status {
        match status() {
            Ok(s) => {
                if !s.writes_allowed && !r.error && !matches!(job, Job::Restore) {
                    r.message = s
                        .ownership_reason
                        .clone()
                        .unwrap_or("The original controller has control.".into());
                    r.error = true;
                }
                r.status = Some(s);
            }
            Err(e) => {
                r.status_failed = true;
                if !r.error {
                    r.message = format!("Couldn't verify settings: {e}");
                    r.error = true;
                }
            }
        }
    }
    r
}
fn demo_caps() -> mcc::control::Capabilities {
    mcc::control::Capabilities {
        fan_mode_raw: 0,
        boost: true,
        windows_key_lock: true,
        usb_charging: true,
        light_bar: true,
        windows_key_locked: false,
        usb_charging_enabled: false,
        light_bar_enabled: true,
    }
}
fn demo_worker(job: Job, c: &mut mcc::control::Capabilities, fail: &mut bool) -> Completed {
    let mut r = Completed::message("Preview only · no hardware changes.");
    match job {
        Job::Fan(f) => c.fan_mode_raw = f.byte(),
        Job::Power(p) => r.power = Some(p),
        Job::Combination(f, p) => {
            c.fan_mode_raw = f.byte();
            r.power = Some(p);
        }
        Job::Preset(p) => {
            c.fan_mode_raw = p.fan().byte();
            r.power = p.power();
        }
        Job::Poll => r.passive = true,
        Job::Switch(s, on) => {
            if *fail && s == Switch::LightBar {
                *fail = false;
                r.error = true;
                r.message =
                    "Switch acknowledgement timed out. Current state refreshed; you can try again."
                        .into();
            } else {
                match s {
                    Switch::LightBar => c.light_bar_enabled = on,
                    Switch::UsbCharging => c.usb_charging_enabled = on,
                    Switch::WindowsKeyLock => c.windows_key_locked = on,
                }
            }
        }
        Job::ResponsivenessTest => {
            std::thread::sleep(Duration::from_millis(500));
            r.message = "Synthetic worker completed.".into();
        }
        _ => {}
    }
    r.status = Some(mcc::hardware::HardwareStatus {
        capabilities: c.clone(),
        writes_allowed: true,
        ownership_reason: None,
    });
    r
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Cooling,
    Lighting,
    Settings,
}
struct Button {
    id: u16,
    hwnd: HWND,
    page: Option<Page>,
}
struct Benchmark {
    start: Instant,
    seconds: u64,
    before: mcc::platform::ProcessMetrics,
    path: std::path::PathBuf,
    hidden: bool,
    self_test: bool,
}
struct App {
    hwnd: HWND,
    taskbar_created: u32,
    fonts: [HFONT; 4],
    dpi: i32,
    buttons: Vec<Button>,
    page: Page,
    tx: SyncSender<Job>,
    rx: Receiver<Completed>,
    session: Session,
    monitor: Monitor,
    clock: Instant,
    visible: bool,
    startup: bool,
    message: String,
    error: bool,
    lighting: Lighting,
    power: Option<PowerPlan>,
    settings: Settings,
    previous_cpu: Option<mcc::CpuTimes>,
    stats: [String; 3],
    own_memory: String,
    tray: NOTIFYICONDATAW,
    benchmark: Option<Benchmark>,
    test_completed: bool,
    ticks: u64,
    busy_ticks: u64,
    demo: bool,
    model: String,
    pages_checked: bool,
    paints: u64,
    button_paints: u64,
    paint_us: u64,
    dashboard_us: u64,
}
thread_local! {static APP:RefCell<Option<App>>=const {RefCell::new(None)};}
impl App {
    fn font_faces(&self) -> Vec<String> {
        // SAFETY: Our window's DC is released after use; selected font restored each time.
        unsafe {
            let dc = GetDC(self.hwnd);
            if dc.is_null() {
                return vec![];
            }
            let result = self
                .fonts
                .iter()
                .map(|font| {
                    let old = SelectObject(dc, *font);
                    let mut name = [0u16; 128];
                    GetTextFaceW(dc, name.len() as i32, name.as_mut_ptr());
                    SelectObject(dc, old);
                    String::from_utf16_lossy(
                        &name[..name.iter().position(|v| *v == 0).unwrap_or(name.len())],
                    )
                })
                .collect();
            ReleaseDC(self.hwnd, dc);
            result
        }
    }
    fn check_pages(&mut self) -> bool {
        let faces = self.font_faces();
        let mut passed = faces.len() == 4 && faces.iter().all(|s| s.starts_with("Segoe UI"));
        let previous = self.lighting.effect;
        self.lighting.effect = Effect::Solid;
        for page in [Page::Cooling, Page::Lighting, Page::Settings] {
            self.page = page;
            self.refresh_controls();
            for b in &self.buttons {
                let expected =
                    b.page.is_none_or(|p| p == page) && !matches!(b.id, SPEED | DIRECTION);
                // SAFETY: Query only UI-owned control styles; root is hidden during this self-test.
                let style = unsafe { GetWindowLongW(b.hwnd, GWL_STYLE) } as u32;
                passed &= (style & WS_VISIBLE != 0) == expected;
                if matches!(
                    b.id,
                    AUTO | BOOST
                        | REGULAR
                        | SAVE_CUSTOM
                        | APPLY_CUSTOM
                        | KEY
                        | USB
                        | BAR
                        | APPLY_LIGHT
                ) {
                    passed &= style & WS_DISABLED != 0;
                }
            }
        }
        self.page = Page::Lighting;
        self.lighting.effect = Effect::Wave;
        self.refresh_controls();
        for id in [SPEED, DIRECTION] {
            // SAFETY: Query style of this UI's existing control.
            passed &=
                unsafe { GetWindowLongW(self.handle(id), GWL_STYLE) } as u32 & WS_VISIBLE != 0;
        }
        self.lighting.effect = previous;
        self.page = Page::Cooling;
        self.refresh_controls();
        passed
    }
    fn px(&self, n: i32) -> i32 {
        n * self.dpi / 96
    }
    fn rect(&self, x: i32, y: i32, w: i32, h: i32) -> RECT {
        RECT {
            left: self.px(x),
            top: self.px(y),
            right: self.px(x + w),
            bottom: self.px(y + h),
        }
    }
    fn invalidate(&self) {
        // SAFETY: UI-owned window; WM_PAINT draws an opaque background.
        unsafe {
            InvalidateRect(self.hwnd, ptr::null(), 0);
        }
    }
    fn button(&mut self, label: &str, id: u16, page: Option<Page>, r: (i32, i32, i32, i32)) {
        let (x, y, w, h) = r;
        // SAFETY: Live parent, valid class/text, integer control ID and owned font.
        let hwnd = unsafe {
            let h = CreateWindowExW(
                0,
                wide("BUTTON").as_ptr(),
                wide(label).as_ptr(),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_OWNERDRAW as u32,
                self.px(x),
                self.px(y),
                self.px(w),
                self.px(h),
                self.hwnd,
                id as usize as HMENU,
                GetModuleHandleW(ptr::null()),
                ptr::null(),
            );
            SendMessageW(h, WM_SETFONT, self.fonts[0] as usize, 0);
            h
        };
        self.buttons.push(Button { id, hwnd, page });
    }
    fn handle(&self, id: u16) -> HWND {
        self.buttons
            .iter()
            .find(|b| b.id == id)
            .map_or(ptr::null_mut(), |b| b.hwnd)
    }
    fn refresh_controls(&self) {
        let mut tray = self.tray;
        if tray.cbSize != 0 {
            let tip = wide(&format!(
                "Minimalist · {}",
                mcc::session::fan_label(self.session.capabilities.as_ref().map(|c| c.fan_mode_raw))
            ));
            tray.szTip.fill(0);
            tray.szTip[..tip.len()].copy_from_slice(&tip);
            // SAFETY: Initialized tray data copied by Windows; fixed short tooltip.
            unsafe {
                Shell_NotifyIconW(NIM_MODIFY, &tray);
            }
        }
        let c = self.session.capabilities.as_ref();
        caption(
            self.handle(CONNECT),
            if self.session.busy {
                "Working…"
            } else if self.session.allowed {
                "Refresh"
            } else {
                "Enable controls"
            },
        );
        caption(
            self.handle(STARTUP),
            if self.startup { "On" } else { "Off" },
        );
        for (id, s) in [
            (KEY, Switch::WindowsKeyLock),
            (USB, Switch::UsbCharging),
            (BAR, Switch::LightBar),
        ] {
            let v = c.and_then(|c| match s {
                Switch::WindowsKeyLock => c.windows_key_lock.then_some(c.windows_key_locked),
                Switch::UsbCharging => c.usb_charging.then_some(c.usb_charging_enabled),
                Switch::LightBar => c.light_bar.then_some(c.light_bar_enabled),
            });
            caption(
                self.handle(id),
                match v {
                    Some(true) => "On",
                    Some(false) => "Off",
                    None => "Unavailable",
                },
            );
        }
        caption(
            self.handle(EFFECT),
            ["Off", "Solid", "Breathing", "Rainbow", "Wave"][self.effect_index()],
        );
        caption(
            self.handle(BRIGHTNESS),
            ["Off", "Low", "Medium", "High", "Maximum"][self.lighting.brightness as usize],
        );
        caption(
            self.handle(SPEED),
            ["Slowest", "Slow", "Medium", "Fast", "Fastest"][self.lighting.speed as usize],
        );
        caption(
            self.handle(DIRECTION),
            [
                "Left → right",
                "Right → left",
                "Bottom → top",
                "Top → bottom",
            ][self.lighting.direction as usize - 1],
        );
        let animated = matches!(
            self.lighting.effect,
            Effect::Breathing | Effect::Rainbow | Effect::Wave
        );
        for b in &self.buttons {
            let visible = b.page.is_none_or(|p| p == self.page)
                && (!matches!(b.id, SPEED | DIRECTION) || animated);
            let enabled = match b.id {
                CONNECT => !self.session.busy,
                AUTO | APPLY_LIGHT | RESTORE => self.session.ready(),
                BOOST => self.session.ready() && c.is_some_and(|c| c.boost),
                KEY => self.session.ready() && c.is_some_and(|c| c.windows_key_lock),
                USB => self.session.ready() && c.is_some_and(|c| c.usb_charging),
                BAR => self.session.ready() && c.is_some_and(|c| c.light_bar),
                QUIET | BALANCED | PERFORMANCE => !self.session.busy,
                REGULAR | SAVE_CUSTOM => self.session.ready(),
                APPLY_CUSTOM => {
                    self.session.ready()
                        && self.settings.profiles.iter().any(|p| p.name == "Custom")
                }
                GPU_TEMP | STARTUP => !self.session.busy,
                _ => true,
            };
            // SAFETY: UI-owned child handles on their owning thread.
            unsafe {
                ShowWindow(b.hwnd, if visible { SW_SHOW } else { SW_HIDE });
                EnableWindow(b.hwnd, i32::from(enabled));
                InvalidateRect(b.hwnd, ptr::null(), 0);
            }
        }
        self.invalidate();
    }
    fn effect_index(&self) -> usize {
        match self.lighting.effect {
            Effect::Off => 0,
            Effect::Solid => 1,
            Effect::Breathing => 2,
            Effect::Rainbow => 3,
            Effect::Wave => 4,
            Effect::Reactive => 0,
        }
    }
    fn submit(&mut self, job: Job) {
        if self.session.busy {
            return;
        }
        if self.tx.try_send(job).is_ok() {
            self.session.busy = true;
            self.message = "Applying…".into();
            self.error = false;
            self.refresh_controls();
        }
    }
    fn poll(&mut self) {
        if self
            .monitor
            .due(self.clock.elapsed().as_millis() as u64, self.session.busy)
            && self
                .tx
                .try_send(if self.session.allowed {
                    Job::Poll
                } else {
                    Job::ReconnectPoll
                })
                .is_err()
        {
            self.monitor
                .completed(self.clock.elapsed().as_millis() as u64, self.visible, true);
        }
    }
    fn update(&mut self) {
        while let Ok(r) = self.rx.try_recv() {
            if r.message.starts_with("Synthetic worker") {
                self.test_completed = true;
            }
            let old_caps = self.session.capabilities.clone();
            let old_allowed = self.session.allowed;
            let old_power = self.power;
            if r.stop_monitor {
                self.monitor.enabled = false;
                // SAFETY: Cancel only our read-only observation timer.
                unsafe {
                    KillTimer(self.hwnd, 3);
                }
            } else if r.status.is_some()
                && !self.monitor.enabled
                && !self.demo
                && self.benchmark.is_none()
            {
                self.monitor.enabled = true;
                // SAFETY: UI-owned timer; activated only after explicit hardware connection.
                unsafe {
                    SetTimer(self.hwnd, 3, 1000, None);
                }
            }
            if r.passive {
                self.monitor.completed(
                    self.clock.elapsed().as_millis() as u64,
                    self.visible,
                    r.status_failed || r.error,
                );
            }
            if let Some(observation) = r.observation {
                self.session
                    .observe_mode(observation.mode, observation.writes_allowed);
                if !observation.writes_allowed {
                    self.message = observation
                        .ownership_reason
                        .unwrap_or("Original controller has control.".into());
                    self.error = true;
                }
            }
            let fresh = r.status.map(|s| (s.capabilities, s.writes_allowed));
            if r.passive {
                self.session.observe(fresh, r.status_failed);
            } else {
                self.session.complete(fresh, r.status_failed);
            }
            if !r.passive || r.error {
                self.message = r.message;
                self.error = r.error;
            } else if !self.session.busy && old_caps != self.session.capabilities {
                self.message = "Laptop controls changed. Display updated from firmware.".into();
                self.error = false;
            }
            if !self.demo {
                self.startup = mcc::startup::enabled().unwrap_or(false);
            }
            if r.power_refreshed || r.power.is_some() {
                self.power = r.power;
            }
            if let Some(c) = r.lighting {
                if let Some(p) = self
                    .settings
                    .profiles
                    .iter_mut()
                    .find(|p| p.name == "Balanced")
                {
                    p.lighting = c;
                }
                if let Ok(path) = std::env::current_exe() {
                    if let Err(e) = self.settings.save(&path.with_file_name("profiles.json")) {
                        self.message = format!("Lighting sent; couldn't save preferences: {e}");
                        self.error = true;
                    }
                }
            }
            if !r.passive
                || r.error
                || old_caps != self.session.capabilities
                || old_allowed != self.session.allowed
                || old_power != self.power
            {
                self.refresh_controls();
            }
        }
    }
    fn dashboard(&mut self) {
        let begin = Instant::now();
        let s = mcc::platform::snapshot();
        self.stats[0] = s
            .system_cpu_times
            .value
            .and_then(|now| {
                let v = self.previous_cpu.and_then(|prev| mcc::cpu_usage(prev, now));
                self.previous_cpu = Some(now);
                v
            })
            .map(|v| format!("{v:.0}%"))
            .unwrap_or("—".into());
        self.stats[1] = s
            .memory_used_percent
            .value
            .map(|v| format!("{v}%"))
            .unwrap_or("—".into());
        self.stats[2] = s
            .battery_percent
            .value
            .map(|v| {
                format!(
                    "{v}% {}",
                    if s.ac_connected.value == Some(true) {
                        "· AC"
                    } else {
                        ""
                    }
                )
            })
            .unwrap_or("—".into());
        self.own_memory = s
            .process
            .value
            .map(|v| format!("{:.1} MiB", v.working_set_bytes as f64 / 1048576.))
            .unwrap_or("—".into());
        let r = self.rect(32, 576, 656, 74);
        // SAFETY: Stack RECT copied by Windows; no background erase.
        unsafe {
            InvalidateRect(self.hwnd, &r, 0);
        }
        self.dashboard_us += begin.elapsed().as_micros() as u64;
    }
    fn connect(&mut self) {
        if self.demo || self.session.allowed {
            self.submit(Job::Status);
            return;
        }
        // SAFETY: Reads the current process token only.
        if unsafe { IsUserAnAdmin() } != 0 {
            self.submit(Job::Connect);
            return;
        }
        match mcc::elevation::launch(self.hwnd) {
            Ok(()) => {
                // SAFETY: Close our old UI only after the synchronous elevated launch succeeds.
                unsafe {
                    PostMessageW(self.hwnd, WM_CLOSE, 1, 0);
                }
            }
            Err(error) => {
                self.message = error;
                self.error = true;
                self.invalidate();
                // SAFETY: Show the complete launch diagnostic; the short footer may clip it.
                unsafe {
                    MessageBoxW(
                        self.hwnd,
                        wide(&self.message).as_ptr(),
                        wide("Enable controls").as_ptr(),
                        MB_OK | MB_ICONERROR,
                    );
                }
            }
        }
    }

    fn choose(&self, id: u16, items: &[&str], current: usize) -> Option<usize> {
        let mut r = RECT::default();
        // SAFETY: Live child HWND, scoped popup menu and literal labels copied by Windows.
        unsafe {
            GetWindowRect(self.handle(id), &mut r);
            let menu = CreatePopupMenu();
            if menu.is_null() {
                return None;
            }
            for (i, s) in items.iter().enumerate() {
                AppendMenuW(
                    menu,
                    MF_STRING | if i == current { MF_CHECKED } else { 0 },
                    1000 + i,
                    wide(s).as_ptr(),
                );
            }
            let n = TrackPopupMenu(
                menu,
                TPM_RETURNCMD | TPM_LEFTALIGN,
                r.left,
                r.bottom,
                0,
                self.hwnd,
                ptr::null(),
            );
            DestroyMenu(menu);
            (n >= 1000 && (n as usize) < 1000 + items.len()).then(|| n as usize - 1000)
        }
    }
    fn command(&mut self, id: u16) {
        match id {
            COOLING | LIGHTING | SETTINGS => {
                self.page = match id {
                    LIGHTING => Page::Lighting,
                    SETTINGS => Page::Settings,
                    _ => Page::Cooling,
                };
                self.refresh_controls();
            }
            CONNECT => self.connect(),
            AUTO | BOOST => {
                if self.session.ready() {
                    self.submit(Job::Fan(if id == BOOST {
                        FanMode::Boost
                    } else {
                        FanMode::Auto
                    }));
                }
            }
            QUIET | BALANCED | PERFORMANCE => {
                if !self.session.busy {
                    self.submit(Job::Power(match id {
                        QUIET => PowerPlan::PowerSaver,
                        PERFORMANCE => PowerPlan::HighPerformance,
                        _ => PowerPlan::Balanced,
                    }));
                }
            }
            REGULAR => {
                if self.session.ready() {
                    if let Some(i) = self.choose(
                        id,
                        &[
                            "Office: Adaptive + Power Saver",
                            "Balanced: Adaptive + Balanced",
                            "High: Adaptive + Performance",
                            "Turbo: Boost, keep performance",
                        ],
                        1,
                    ) {
                        self.submit(Job::Preset(
                            [
                                Preset::Office,
                                Preset::Balanced,
                                Preset::High,
                                Preset::Turbo,
                            ][i],
                        ));
                    }
                }
            }
            SAVE_CUSTOM => {
                if self.session.ready() {
                    let fan =
                        self.session
                            .capabilities
                            .as_ref()
                            .and_then(|c| match c.fan_mode_raw {
                                0 => Some(FanMode::Auto),
                                64 => Some(FanMode::Boost),
                                _ => None,
                            });
                    if let (Some(fan), Some(power)) = (fan, self.power) {
                        let mut settings = self.settings.clone();
                        settings.profiles.retain(|p| p.name != "Custom");
                        settings.profiles.push(mcc::settings::Profile {
                            name: "Custom".into(),
                            fan,
                            power,
                            lighting: self.lighting.clone(),
                        });
                        let result = if self.demo {
                            Ok(())
                        } else {
                            std::env::current_exe()
                                .map_err(|e| e.to_string())
                                .and_then(|p| settings.save(&p.with_file_name("profiles.json")))
                        };
                        match result {
                            Ok(()) => {
                                self.settings = settings;
                                self.message = "Saved the current fan + performance combination. It will not apply automatically.".into();
                                self.error = false;
                            }
                            Err(e) => {
                                self.message = format!("Could not save custom combination: {e}");
                                self.error = true;
                            }
                        }
                    } else {
                        self.message = "Refresh first: a verified fan mode and Windows power plan are required to save.".into();
                        self.error = true;
                    }
                    self.refresh_controls();
                }
            }
            APPLY_CUSTOM => {
                if self.session.ready() {
                    if let Some(p) = self.settings.profiles.iter().find(|p| p.name == "Custom") {
                        self.submit(Job::Combination(p.fan, p.power));
                    }
                }
            }
            STARTUP => {
                if self.demo {
                    self.startup = !self.startup;
                    self.refresh_controls();
                } else {
                    self.submit(Job::Startup(!self.startup));
                }
            }
            CUSTOM => {
                self.message = "Minimum-noise / custom fan speeds need verified sensors and firmware overheat recovery. Manual low speeds remain locked; workload presets never pin fans low.".into();
                self.error = false;
                self.invalidate();
            }
            KEY | USB | BAR => {
                let s = match id {
                    KEY => Switch::WindowsKeyLock,
                    USB => Switch::UsbCharging,
                    _ => Switch::LightBar,
                };
                if let Some(on) = self.session.toggle_target(s) {
                    self.submit(Job::Switch(s, on));
                }
            }
            APPLY_LIGHT => {
                if self.session.ready() {
                    self.submit(Job::Light(self.lighting.clone()));
                }
            }
            EFFECT => {
                if let Some(i) = self.choose(
                    id,
                    &["Off", "Solid", "Breathing", "Rainbow", "Wave"],
                    self.effect_index(),
                ) {
                    self.lighting.effect = [
                        Effect::Off,
                        Effect::Solid,
                        Effect::Breathing,
                        Effect::Rainbow,
                        Effect::Wave,
                    ][i];
                    self.refresh_controls();
                }
            }
            BRIGHTNESS => {
                if let Some(i) = self.choose(
                    id,
                    &["Off", "Low", "Medium", "High", "Maximum"],
                    self.lighting.brightness as usize,
                ) {
                    self.lighting.brightness = i as u8;
                    self.refresh_controls();
                }
            }
            SPEED => {
                if let Some(i) = self.choose(
                    id,
                    &["Slowest", "Slow", "Medium", "Fast", "Fastest"],
                    self.lighting.speed as usize,
                ) {
                    self.lighting.speed = i as u8;
                    self.refresh_controls();
                }
            }
            DIRECTION => {
                if let Some(i) = self.choose(
                    id,
                    &[
                        "Left → right",
                        "Right → left",
                        "Bottom → top",
                        "Top → bottom",
                    ],
                    self.lighting.direction as usize - 1,
                ) {
                    self.lighting.direction = i as u8 + 1;
                    self.refresh_controls();
                }
            }
            SWATCH..=225 => {
                self.lighting.rgb = PALETTE[(id - SWATCH) as usize];
                self.refresh_controls();
            }
            COLOR => {
                let mut colors = [0u32; 16];
                let mut c = CHOOSECOLORW {
                    lStructSize: std::mem::size_of::<CHOOSECOLORW>() as u32,
                    hwndOwner: self.hwnd,
                    rgbResult: rgb(
                        self.lighting.rgb[0],
                        self.lighting.rgb[1],
                        self.lighting.rgb[2],
                    ),
                    lpCustColors: colors.as_mut_ptr(),
                    Flags: CC_RGBINIT | CC_FULLOPEN,
                    ..Default::default()
                };
                // SAFETY: Correct structure size, live owner, sixteen writable colors for modal lifetime.
                if unsafe { ChooseColorW(&mut c) } != 0 {
                    self.lighting.rgb = [
                        c.rgbResult as u8,
                        (c.rgbResult >> 8) as u8,
                        (c.rgbResult >> 16) as u8,
                    ];
                    self.refresh_controls();
                }
            }
            GPU_TEMP => self.submit(Job::Gpu),
            RESTORE => {
                if self.session.ready() {
                    self.submit(Job::Restore);
                }
            }
            SCREEN_OFF => {
                if !self.demo {
                    if let Err(e) = mcc::desktop::screen_off() {
                        self.message = e;
                        self.error = true;
                        self.invalidate();
                    }
                }
            }
            GPU => {
                if !self.demo {
                    // SAFETY: Fixed OS Settings URI; no external path or shell command.
                    unsafe {
                        ShellExecuteW(
                            self.hwnd,
                            wide("open").as_ptr(),
                            wide("ms-settings:display-advancedgraphics").as_ptr(),
                            ptr::null(),
                            ptr::null(),
                            SW_SHOWNORMAL,
                        );
                    }
                }
            }
            QUIT => {
                // SAFETY: Firmware-managed modes remain active; no software fan loop to terminate.
                unsafe {
                    PostMessageW(self.hwnd, WM_CLOSE, 1, 0);
                }
            }
            _ => {}
        }
    }
    fn hide(&mut self) {
        self.visible = false;
        // SAFETY: UI-owned HWND and timer; hidden dashboard polling stops.
        unsafe {
            ShowWindow(self.hwnd, SW_HIDE);
            KillTimer(self.hwnd, 1);
        }
    }
    fn show(&mut self) {
        self.visible = true;
        self.monitor.wake();
        // SAFETY: Live HWND, documented restore and one-second OS-only timer.
        unsafe {
            ShowWindow(self.hwnd, SW_RESTORE);
            SetForegroundWindow(self.hwnd);
            SetTimer(self.hwnd, 1, 1000, None);
        }
        self.previous_cpu = None;
        if !self.demo {
            self.power = mcc::desktop::active_power();
        }
        self.dashboard();
        if self.session.capabilities.is_some() {
            self.submit(Job::Status);
        }
    }
    fn draw_text(&self, dc: HDC, s: &str, r: RECT, font: usize, color: COLORREF, flags: u32) {
        let mut r = r;
        let t = wide(s);
        // SAFETY: Paint DC and owned font live; string count excludes NUL; RECT valid for call.
        unsafe {
            SelectObject(dc, self.fonts[font]);
            SetBkMode(dc, TRANSPARENT as i32);
            SetTextColor(dc, color);
            DrawTextW(
                dc,
                t.as_ptr(),
                t.len() as i32 - 1,
                &mut r,
                flags | DT_NOPREFIX,
            );
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn label(
        &self,
        dc: HDC,
        s: &str,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        font: usize,
        color: COLORREF,
    ) {
        self.draw_text(
            dc,
            s,
            self.rect(x, y, w, h),
            font,
            color,
            DT_LEFT | DT_WORDBREAK,
        );
    }
    fn round(&self, dc: HDC, r: RECT, fill: COLORREF, border: COLORREF, radius: i32) {
        // SAFETY: Scoped GDI objects; restore previous selections before deleting.
        unsafe {
            let brush = CreateSolidBrush(fill);
            let pen = CreatePen(PS_SOLID, self.px(1).max(1), border);
            let ob = SelectObject(dc, brush);
            let op = SelectObject(dc, pen);
            RoundRect(
                dc,
                r.left,
                r.top,
                r.right,
                r.bottom,
                self.px(radius),
                self.px(radius),
            );
            SelectObject(dc, ob);
            SelectObject(dc, op);
            DeleteObject(brush);
            DeleteObject(pen);
        }
    }
    fn paint(&self, dc: HDC) {
        self.round(dc, self.rect(0, 0, 720, 708), BG, BG, 0);
        self.label(dc, "Minimalist", 32, 26, 420, 34, 2, INK);
        self.label(
            dc,
            &format!("{} · v{}", self.model, env!("CARGO_PKG_VERSION")),
            32,
            65,
            420,
            24,
            3,
            MUTED,
        );
        self.label(
            dc,
            if self.demo {
                "Preview · simulated hardware"
            } else if self.session.allowed {
                "Controls ready"
            } else {
                "Monitor only"
            },
            482,
            75,
            206,
            24,
            3,
            if self.demo { MUTED } else { GREEN },
        );
        match self.page {
            Page::Cooling => {
                self.label(dc, "Cooling & performance", 32, 171, 640, 34, 2, INK);
                self.round(dc, self.rect(32, 219, 656, 186), WHITE, BORDER, 16);
                self.label(dc, "FAN COOLING", 52, 233, 250, 24, 3, MUTED);
                self.label(
                    dc,
                    mcc::session::fan_label(
                        self.session.capabilities.as_ref().map(|c| c.fan_mode_raw),
                    ),
                    412,
                    233,
                    256,
                    24,
                    1,
                    GREEN,
                );
                self.label(
                    dc,
                    "WINDOWS PERFORMANCE · independent of fan mode",
                    52,
                    315,
                    616,
                    24,
                    3,
                    MUTED,
                );
                self.label(dc, "Presets & your combination", 32, 420, 656, 24, 1, INK);
                self.label(
                    dc,
                    "Choose fan + performance above, then save your combination.",
                    32,
                    477,
                    656,
                    22,
                    3,
                    MUTED,
                );
            }
            Page::Lighting => {
                self.label(dc, "Lighting", 32, 171, 380, 34, 2, INK);
                self.round(dc, self.rect(32, 219, 656, 282), WHITE, BORDER, 16);
                self.label(dc, "Keyboard", 52, 238, 320, 30, 1, INK);
                self.label(dc, "Effect", 52, 282, 280, 22, 3, MUTED);
                self.label(dc, "Brightness", 364, 282, 280, 22, 3, MUTED);
                self.label(dc, "Color", 52, 353, 280, 22, 3, MUTED);
                if matches!(
                    self.lighting.effect,
                    Effect::Breathing | Effect::Rainbow | Effect::Wave
                ) {
                    self.label(dc, "Animation speed", 52, 426, 280, 22, 3, MUTED);
                    self.label(dc, "Direction", 364, 426, 280, 22, 3, MUTED);
                } else {
                    self.label(
                        dc,
                        "The keyboard runs effects itself. No background animation loop.",
                        52,
                        442,
                        590,
                        34,
                        3,
                        MUTED,
                    );
                }
                self.round(dc, self.rect(32, 513, 656, 50), WHITE, BORDER, 14);
                self.label(dc, "Light bar", 52, 527, 380, 26, 0, INK);
            }
            Page::Settings => {
                self.label(dc, "Settings", 32, 171, 380, 34, 2, INK);
                self.round(dc, self.rect(32, 219, 656, 161), WHITE, BORDER, 16);
                self.label(dc, "Lock Windows key", 52, 241, 460, 28, 1, INK);
                self.label(
                    dc,
                    "Prevent accidental presses while playing.",
                    52,
                    270,
                    460,
                    25,
                    3,
                    MUTED,
                );
                self.label(dc, "USB charging while asleep", 52, 313, 460, 28, 1, INK);
                self.label(
                    dc,
                    "Keep the supported USB port powered when hibernating.",
                    52,
                    342,
                    460,
                    25,
                    3,
                    MUTED,
                );
                self.round(dc, self.rect(32, 388, 656, 158), WHITE, BORDER, 16);
                self.label(dc, "Start with Windows", 52, 399, 420, 28, 1, INK);
                self.label(
                    dc,
                    "Opens in the tray. Enable controls requests permission after sign-in.",
                    52,
                    431,
                    616,
                    20,
                    3,
                    MUTED,
                );
                self.label(
                    dc,
                    &format!(
                        "v{} · {} app memory · Close the window to keep it in the tray.",
                        env!("CARGO_PKG_VERSION"),
                        self.own_memory
                    ),
                    32,
                    550,
                    656,
                    22,
                    3,
                    MUTED,
                );
            }
        }
        self.paint_stats(dc);
        self.label(
            dc,
            &self.message,
            32,
            663,
            656,
            40,
            3,
            if self.error { ERROR } else { MUTED },
        );
    }
    fn paint_stats(&self, dc: HDC) {
        self.round(dc, self.rect(32, 579, 656, 66), WHITE, BORDER, 14);
        for (i, name) in ["CPU usage", "Memory used", "Battery"].iter().enumerate() {
            let x = 52 + i as i32 * 212;
            self.label(dc, &self.stats[i], x, 590, 195, 28, 1, INK);
            self.label(dc, name, x, 621, 195, 20, 3, MUTED);
        }
    }
    fn draw_button(&self, d: &DRAWITEMSTRUCT) {
        let id = d.CtlID as u16;
        let mut r = d.rcItem;
        let disabled = d.itemState & ODS_DISABLED != 0;
        let pressed = d.itemState & ODS_SELECTED != 0;
        let selected = match id {
            COOLING => self.page == Page::Cooling,
            LIGHTING => self.page == Page::Lighting,
            SETTINGS => self.page == Page::Settings,
            AUTO => self
                .session
                .capabilities
                .as_ref()
                .is_some_and(|c| c.fan_mode_raw == 0),
            BOOST => self
                .session
                .capabilities
                .as_ref()
                .is_some_and(|c| c.fan_mode_raw == 64),
            QUIET => self.power == Some(PowerPlan::PowerSaver),
            BALANCED => self.power == Some(PowerPlan::Balanced),
            PERFORMANCE => self.power == Some(PowerPlan::HighPerformance),
            STARTUP => self.startup,
            KEY => self
                .session
                .capabilities
                .as_ref()
                .is_some_and(|c| c.windows_key_locked),
            USB => self
                .session
                .capabilities
                .as_ref()
                .is_some_and(|c| c.usb_charging_enabled),
            BAR => self
                .session
                .capabilities
                .as_ref()
                .is_some_and(|c| c.light_bar_enabled),
            _ => false,
        };
        // SAFETY: Windows supplies valid DC/RECT; save and restore isolate drawing state.
        let saved = unsafe { SaveDC(d.hDC) };
        let nav = matches!(id, COOLING | LIGHTING | SETTINGS);
        let primary = id == APPLY_LIGHT || id == CONNECT && !self.session.allowed;
        let fill = if disabled {
            BG
        } else if primary {
            GREEN
        } else if selected || pressed {
            MINT
        } else if nav {
            BG
        } else {
            WHITE
        };
        let edge = if selected {
            GREEN
        } else if nav {
            fill
        } else {
            BORDER
        };
        self.round(d.hDC, r, fill, edge, 12);
        let mut buffer = [0u16; 160];
        // SAFETY: Owned child handle and correctly sized writable character buffer.
        let n = unsafe { GetWindowTextW(d.hwndItem, buffer.as_mut_ptr(), buffer.len() as i32) };
        let s = String::from_utf16_lossy(&buffer[..n.max(0) as usize]);
        let color = if disabled {
            MUTED
        } else if primary {
            WHITE
        } else if selected {
            GREEN
        } else {
            INK
        };
        if (SWATCH..=225).contains(&id) {
            let c = PALETTE[(id - SWATCH) as usize];
            let active = c == self.lighting.rgb;
            let p = self.px(5);
            r.left += p;
            r.top += p;
            r.right -= p;
            r.bottom -= p;
            self.round(
                d.hDC,
                r,
                rgb(c[0], c[1], c[2]),
                if active { GREEN } else { BORDER },
                10,
            );
            if active {
                self.draw_text(
                    d.hDC,
                    "✓",
                    r,
                    1,
                    if id == 225 { INK } else { WHITE },
                    DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                );
            }
        } else {
            let choice = matches!(id, EFFECT | BRIGHTNESS | SPEED | DIRECTION);
            if choice {
                r.left += self.px(16);
                r.right -= self.px(34);
            }
            self.draw_text(
                d.hDC,
                &s,
                r,
                if nav || primary { 1 } else { 0 },
                color,
                (if choice { DT_LEFT } else { DT_CENTER }) | DT_VCENTER | DT_SINGLELINE,
            );
            if choice {
                let mut arrow = d.rcItem;
                arrow.left = arrow.right - self.px(32);
                self.draw_text(
                    d.hDC,
                    "⌄",
                    arrow,
                    0,
                    MUTED,
                    DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                );
            }
        }
        if d.itemState & ODS_FOCUS != 0 && d.itemState & ODS_NOFOCUSRECT == 0 {
            // SAFETY: Focus rectangle inside this item, using its live DC.
            unsafe {
                InflateRect(&mut r, -self.px(4), -self.px(4));
                DrawFocusRect(d.hDC, &r);
            }
        }
        // SAFETY: Restore DC state saved on entry.
        unsafe {
            RestoreDC(d.hDC, saved);
        }
    }
}
unsafe extern "system" fn procedure(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if msg == WM_DESTROY {
        // SAFETY: Windows invokes callback on its UI thread.
        unsafe {
            PostQuitMessage(0);
        }
        return 0;
    }
    let handled=APP.with(|slot|{
        let Ok(mut slot)=slot.try_borrow_mut()else{return false;};let Some(a)=slot.as_mut()else{return false;};
        if msg == a.taskbar_created && msg != 0 {
            // SAFETY: Re-add our initialized tray icon after Explorer restarts.
            unsafe { Shell_NotifyIconW(NIM_ADD, &a.tray); }
            a.refresh_controls();
            return true;
        }
        match msg {
            WM_SHOWWINDOW if w != 0 => { a.visible=true; a.monitor.wake();
                // SAFETY: UI-owned dashboard timer when another instance restores this window.
                unsafe { SetTimer(hwnd, 1, 1000, None); } true }
            WM_ERASEBKGND=>true,
            WM_PAINT=>{
                a.paints += 1;
                // SAFETY: Paired paint calls with initialized PAINTSTRUCT and live HWND.
                unsafe{let mut p=PAINTSTRUCT::default();let dc=BeginPaint(hwnd,&mut p);let saved=SaveDC(dc);let begin=Instant::now();if p.rcPaint.top >= a.px(576) && p.rcPaint.bottom <= a.px(650) {a.paint_stats(dc);} else {a.paint(dc);} a.paint_us+=begin.elapsed().as_micros() as u64;RestoreDC(dc,saved);EndPaint(hwnd,&p);}true
            }
            WM_DRAWITEM if l!=0=>{
                a.button_paints += 1;
                // SAFETY: Windows provides DRAWITEMSTRUCT for synchronous WM_DRAWITEM.
                a.draw_button(unsafe{&*(l as *const DRAWITEMSTRUCT)});true
            }
            WM_COMMAND=>{a.command((w&65535)as u16);true}
            WM_KEYDOWN if w==0x74=>{if a.session.capabilities.is_some(){a.submit(Job::Status);}true}
            UPDATE=>{a.update();true}
            WM_POWERBROADCAST if w == 18 || w == 7 => { a.monitor.wake(); a.poll(); true }
            WM_TIMER if w==3=>{a.poll();true}
            WM_TIMER=>{
                a.ticks+=1;if a.session.busy{a.busy_ticks+=1;}
                if w==2 {
                    if let Some(b)=&a.benchmark{if b.start.elapsed()>=Duration::from_secs(b.seconds){
                        let after=mcc::platform::metrics().value;
                        let passed=!b.self_test || (a.test_completed && a.busy_ticks>=3 && a.pages_checked && a.buttons.iter().all(|b|!b.hwnd.is_null()));
                        let report=serde_json::json!({"ok":passed,"benchmark":"native_ui_os_counters_only","hidden":b.hidden,"wall_ms":b.start.elapsed().as_secs_f64()*1000.,"cpu_ms":after.map(|a|a.cpu_time_100ns.saturating_sub(b.before.cpu_time_100ns)as f64/10000.),"working_set_bytes":after.map(|a|a.working_set_bytes),"peak_working_set_bytes":after.map(|a|a.peak_working_set_bytes),"private_commit_bytes":after.map(|a|a.private_commit_bytes),"message_ticks":a.ticks,"ticks_during_worker_delay":a.busy_ticks,"worker_responsive":a.test_completed,"hardware_accessed":false,"pages":3,"font_faces":a.font_faces(),"paints":a.paints,"button_paints":a.button_paints,"paint_us":a.paint_us,"dashboard_us":a.dashboard_us,"pages_isolated_and_startup_writes_disabled":a.pages_checked,"note":"Includes startup. Windows CPU counters are quantized. No game FPS, thermal or OEM comparison."});
                        let _=std::fs::write(&b.path,serde_json::to_vec_pretty(&report).unwrap());
                        // SAFETY: Exit this UI after its bounded test/benchmark.
                        unsafe{PostMessageW(hwnd,WM_CLOSE,1,0);}
                    }}
                }else{a.update();a.dashboard();}true
            }
            TRAY if l as u32==WM_LBUTTONUP || l as u32==WM_LBUTTONDBLCLK=>{a.show();true}
            TRAY if l as u32==WM_RBUTTONUP=>{
                // SAFETY: Scoped menu and POINT with fixed commands; destroy after use.
                unsafe{let mut p=POINT::default();GetCursorPos(&mut p);let menu=CreatePopupMenu();AppendMenuW(menu,MF_STRING,CONNECT as usize,wide("Refresh settings").as_ptr());AppendMenuW(menu,MF_STRING,QUIT as usize,wide("Exit").as_ptr());SetForegroundWindow(hwnd);let id=TrackPopupMenu(menu,TPM_RETURNCMD|TPM_RIGHTBUTTON,p.x,p.y,0,hwnd,ptr::null());DestroyMenu(menu);if id!=0{a.command(id as u16);}}true
            }
            WM_CLOSE=>{
                if w==1{
                    // SAFETY: Destroy on its owning thread.
                    unsafe{DestroyWindow(hwnd);}
                }else{a.hide();}true
            }
            WM_SIZE if w==SIZE_MINIMIZED as usize=>{a.hide();true}
            _=>false,
        }
    });
    if handled {
        if msg == WM_ERASEBKGND || msg == WM_DRAWITEM {
            1
        } else {
            0
        }
    } else {
        // SAFETY: Forward unhandled Windows message unchanged.
        unsafe { DefWindowProcW(hwnd, msg, w, l) }
    }
}
pub fn run() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args == ["--enable-startup"] || args == ["--disable-startup"] {
        if let Err(e) = mcc::startup::set(args == ["--enable-startup"]) {
            // SAFETY: Error display uses static title and bounded generated message.
            unsafe {
                MessageBoxW(
                    ptr::null_mut(),
                    wide(&e).as_ptr(),
                    wide("Minimalist startup").as_ptr(),
                    MB_OK | MB_ICONERROR,
                );
            }
            std::process::exit(1);
        }
        return;
    }
    let demo = args == ["--demo"] || args == ["--demo-failure"];
    let mode = match args.as_slice() {
        [] => None,
        [a] if a == "--connect" || a == "--tray" || a == "--demo" || a == "--demo-failure" => None,
        [a, path] if a == "--self-test" => Some((2, std::path::PathBuf::from(path), true, true)),
        [a, seconds, path] if a == "--benchmark" || a == "--benchmark-hidden" => {
            match seconds.parse::<u64>() {
                Ok(n) if (1..=3600).contains(&n) => Some((
                    n,
                    std::path::PathBuf::from(path),
                    a == "--benchmark-hidden",
                    false,
                )),
                _ => return,
            }
        }
        _ => return,
    };
    let _instance = if !demo && mode.is_none() {
        match mcc::instance::acquire(args == ["--tray"]) {
            Ok(Some(guard)) => Some(guard),
            Ok(None) => return,
            Err(e) => {
                // SAFETY: Error display contains no action or executable command.
                unsafe {
                    MessageBoxW(
                        ptr::null_mut(),
                        wide(&e).as_ptr(),
                        wide("Minimalist").as_ptr(),
                        MB_OK | MB_ICONERROR,
                    );
                }
                return;
            }
        }
    } else {
        None
    };
    let _helper_lease = if !demo && mode.is_none() {
        let lease = std::env::current_exe()
            .and_then(|p| mcc::process::lock_helper(&p.with_file_name("mcc.exe")));
        match lease {
            Ok(lease) => Some(lease),
            Err(e) => {
                // SAFETY: Informational error only; no control or recovery writes occur.
                unsafe {
                    MessageBoxW(ptr::null_mut(), wide(&format!("Keep mcc.exe alongside this app and close any updater before opening it. Helper could not be secured: {e}")).as_ptr(), wide("Minimalist").as_ptr(), MB_OK | MB_ICONERROR);
                }
                return;
            }
        }
    } else {
        None
    };
    let before = mcc::platform::metrics().value;
    // SAFETY: Set DPI awareness before creating windows; geometry/fonts scale consistently.
    unsafe {
        SetProcessDPIAware();
    }
    // SAFETY: Register Win32 callback and create own window with valid strings and handles.
    let hwnd = unsafe {
        let instance = GetModuleHandleW(ptr::null());
        let class = wide(if demo || mode.is_some() {
            "MinimalistControlCenter.Native.Test"
        } else {
            mcc::instance::class()
        });
        let wc = WNDCLASSW {
            lpfnWndProc: Some(procedure),
            hInstance: instance,
            lpszClassName: class.as_ptr(),
            hCursor: LoadCursorW(ptr::null_mut(), IDC_ARROW),
            ..Default::default()
        };
        if RegisterClassW(&wc) == 0 {
            return;
        }
        CreateWindowExW(
            0,
            class.as_ptr(),
            wide("Minimalist Control Center").as_ptr(),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            736,
            748,
            ptr::null_mut(),
            ptr::null_mut(),
            instance,
            ptr::null(),
        )
    };
    if hwnd.is_null() {
        return;
    }
    // SAFETY: Live window, DPI query, font creation and resize affect only this UI.
    let (dpi, fonts) = unsafe {
        let dpi = GetDpiForWindow(hwnd).max(96) as i32;
        let fonts = [(16, 400), (16, 600), (25, 600), (13, 400)].map(|(n, w)| {
            CreateFontW(
                -n * dpi / 96,
                0,
                0,
                0,
                w,
                0,
                0,
                0,
                DEFAULT_CHARSET as u32,
                OUT_DEFAULT_PRECIS as u32,
                CLIP_DEFAULT_PRECIS as u32,
                CLEARTYPE_QUALITY as u32,
                DEFAULT_PITCH as u32,
                wide("Segoe UI").as_ptr(),
            )
        });
        let mut r = RECT {
            left: 0,
            top: 0,
            right: 720 * dpi / 96,
            bottom: 708 * dpi / 96,
        };
        AdjustWindowRectEx(
            &mut r,
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
            0,
            0,
        );
        SetWindowPos(
            hwnd,
            ptr::null_mut(),
            0,
            0,
            r.right - r.left,
            r.bottom - r.top,
            SWP_NOMOVE | SWP_NOZORDER,
        );
        (dpi, fonts)
    };
    let (tx, jobs) = mpsc::sync_channel(1);
    let (result_tx, rx) = mpsc::channel();
    let window_id = hwnd as usize;
    let fail_demo = args == ["--demo-failure"];
    std::thread::spawn(move || {
        let mut caps = demo_caps();
        let mut fail = fail_demo;
        while let Ok(job) = jobs.recv() {
            let r = if demo {
                demo_worker(job, &mut caps, &mut fail)
            } else {
                worker(job)
            };
            if result_tx.send(r).is_err() {
                break;
            }
            // SAFETY: Integer notification only; failure after window close harmless.
            unsafe {
                PostMessageW(window_id as HWND, UPDATE, 0, 0);
            }
        }
    });
    let settings = std::env::current_exe()
        .ok()
        .and_then(|p| Settings::load(&p.with_file_name("profiles.json")).ok())
        .unwrap_or_default();
    let lighting = settings
        .profiles
        .iter()
        .find(|p| p.name == "Balanced")
        .map(|p| p.lighting.clone())
        .unwrap_or_default();
    let mut a = App {
        hwnd,
        // SAFETY: Fixed registered Windows notification name copied by Windows.
        taskbar_created: unsafe { RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()) },
        fonts,
        dpi,
        buttons: vec![],
        page: Page::Cooling,
        tx,
        rx,
        session: Session::default(),
        monitor: Monitor::default(),
        clock: Instant::now(),
        visible: true,
        startup: !demo && mcc::startup::enabled().unwrap_or(false),
        message: "Choose Enable controls to use fans and lighting. Nothing is applied on startup."
            .into(),
        error: false,
        lighting,
        power: if demo {
            Some(PowerPlan::Balanced)
        } else {
            mcc::desktop::active_power()
        },
        settings,
        previous_cpu: None,
        stats: Default::default(),
        own_memory: String::new(),
        tray: NOTIFYICONDATAW::default(),
        benchmark: mode.and_then(|(seconds, path, hidden, self_test)| {
            before.map(|before| Benchmark {
                start: Instant::now(),
                seconds,
                path,
                hidden,
                self_test,
                before,
            })
        }),
        test_completed: false,
        ticks: 0,
        busy_ticks: 0,
        demo,
        model: mcc::platform::identity()
            .model
            .value
            .unwrap_or("Laptop".into()),
        pages_checked: false,
        paints: 0,
        button_paints: 0,
        paint_us: 0,
        dashboard_us: 0,
    };
    a.button("Enable controls", CONNECT, None, (500, 28, 188, 38));
    a.button("Cooling", COOLING, None, (32, 112, 128, 38));
    a.button("Lighting", LIGHTING, None, (172, 112, 128, 38));
    a.button("Settings", SETTINGS, None, (312, 112, 128, 38));
    let c = Some(Page::Cooling);
    let l = Some(Page::Lighting);
    let s = Some(Page::Settings);
    a.button(
        "Adaptive · firmware controlled",
        AUTO,
        c,
        (52, 268, 298, 36),
    );
    a.button("Turbo · cooling boost", BOOST, c, (370, 268, 298, 36));
    a.button("Power Saver", QUIET, c, (52, 349, 194, 36));
    a.button("Balanced", BALANCED, c, (262, 349, 194, 36));
    a.button("Performance", PERFORMANCE, c, (474, 349, 194, 36));
    a.button(
        "Regular presets: Office / Balanced / High / Turbo",
        REGULAR,
        c,
        (32, 447, 656, 28),
    );
    a.button("Save current as Custom", SAVE_CUSTOM, c, (32, 505, 318, 32));
    a.button("Apply saved Custom", APPLY_CUSTOM, c, (370, 505, 318, 32));
    a.button(
        "Custom fan curves: awaiting thermal validation",
        CUSTOM,
        c,
        (32, 542, 656, 28),
    );
    a.button("Apply keyboard", APPLY_LIGHT, l, (496, 236, 172, 36));
    a.button("Solid", EFFECT, l, (52, 307, 286, 34));
    a.button("High", BRIGHTNESS, l, (364, 307, 304, 34));
    for (i, name) in ["Blue", "Purple", "Green", "Red", "Amber", "White"]
        .iter()
        .enumerate()
    {
        a.button(
            name,
            SWATCH + i as u16,
            l,
            (52 + i as i32 * 50, 378, 40, 36),
        );
    }
    a.button("Custom color…", COLOR, l, (364, 378, 304, 36));
    a.button("Medium", SPEED, l, (52, 451, 286, 34));
    a.button("Left → right", DIRECTION, l, (364, 451, 304, 34));
    a.button("Unavailable", BAR, l, (548, 522, 120, 32));
    a.button("Unavailable", KEY, s, (548, 247, 120, 34));
    a.button("Unavailable", USB, s, (548, 319, 120, 34));
    a.button("Off", STARTUP, s, (548, 396, 120, 34));
    a.button("Turn screen off", SCREEN_OFF, s, (52, 455, 194, 36));
    a.button("Graphics settings", GPU, s, (262, 455, 194, 36));
    a.button("Read GPU temp", GPU_TEMP, s, (474, 455, 194, 36));
    a.button("Use original controller", RESTORE, s, (52, 501, 300, 36));
    a.button("Exit app", QUIT, s, (474, 501, 194, 36));
    if demo {
        a.session.complete(Some((demo_caps(), true)), false);
        a.message = "Preview only · all hardware actions are simulated.".into();
    }
    a.refresh_controls();
    if a.benchmark.as_ref().is_some_and(|b| b.self_test) {
        a.pages_checked = a.check_pages();
    }
    a.dashboard();
    a.tray.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    a.tray.hWnd = hwnd;
    a.tray.uID = 1;
    a.tray.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    a.tray.uCallbackMessage = TRAY;
    // SAFETY: Shared system icon; correct tray structure and live callback window.
    unsafe {
        a.tray.hIcon = LoadIconW(ptr::null_mut(), IDI_APPLICATION);
    }
    let tip = wide("Minimalist — click to open");
    a.tray.szTip[..tip.len()].copy_from_slice(&tip);
    // SAFETY: Fully initialized tray structure copied by Windows.
    unsafe {
        Shell_NotifyIconW(NIM_ADD, &a.tray);
    }
    if args == ["--connect"] {
        a.submit(Job::Connect);
    }
    let hidden = args == ["--tray"] || a.benchmark.as_ref().is_some_and(|b| b.hidden);
    a.visible = !hidden;
    let test = a.benchmark.as_ref().is_some_and(|b| b.self_test);
    let benchmark = a.benchmark.is_some();
    if test {
        a.submit(Job::ResponsivenessTest);
    }
    APP.with(|slot| *slot.borrow_mut() = Some(a));
    // SAFETY: Standard message loop; hidden tests never initiate hardware access.
    unsafe {
        if !hidden {
            ShowWindow(hwnd, SW_SHOWNORMAL);
            SetTimer(hwnd, 1, 1000, None);
        }
        if benchmark {
            SetTimer(hwnd, 2, if test { 100 } else { 1000 }, None);
        }
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
            let keyboard = (WM_KEYFIRST..=WM_KEYLAST).contains(&msg.message);
            if !keyboard || IsDialogMessageW(hwnd, &msg) == 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
    APP.with(|slot| {
        if let Some(a) = slot.borrow_mut().take() {
            // SAFETY: Remove tray icon and fonts once after all windows are destroyed.
            unsafe {
                Shell_NotifyIconW(NIM_DELETE, &a.tray);
                for font in a.fonts {
                    DeleteObject(font);
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ready() -> mcc::hardware::HardwareStatus {
        mcc::hardware::HardwareStatus {
            capabilities: demo_caps(),
            writes_allowed: true,
            ownership_reason: None,
        }
    }
    #[test]
    fn failed_toggle_refreshes_once_and_never_replays_the_action() {
        let attempts = std::cell::Cell::new(0);
        let reads = std::cell::Cell::new(0);
        let r = worker_with(
            Job::Switch(Switch::LightBar, false),
            |_| {
                attempts.set(attempts.get() + 1);
                Err("late acknowledgement".into())
            },
            || {
                reads.set(reads.get() + 1);
                Ok(ready())
            },
        );
        assert_eq!(attempts.get(), 1);
        assert_eq!(reads.get(), 1);
        assert!(r.error);
        assert!(!r.status_failed);
        let mut s = Session::default();
        s.complete(
            r.status.map(|s| (s.capabilities, s.writes_allowed)),
            r.status_failed,
        );
        assert!(s.ready());
        assert_eq!(s.toggle_target(Switch::LightBar), Some(false));
    }
    #[test]
    fn lost_readback_disables_controls_instead_of_using_cached_state() {
        let r = worker_with(
            Job::Preset(Preset::Turbo),
            |_| Ok(Completed::message("sent")),
            || Err("transport gone".into()),
        );
        assert!(r.error && r.status_failed);
        assert!(r.status.is_none());
    }
    #[test]
    fn unrelated_startup_error_does_not_poll_or_disable_hardware() {
        let r = worker_with(
            Job::Startup(true),
            |_| Err("plan unavailable".into()),
            || panic!("unexpected firmware poll"),
        );
        assert!(r.error);
        assert!(!r.status_failed);
    }
    #[test]
    fn independent_performance_changes_never_request_firmware_status() {
        for plan in [
            PowerPlan::PowerSaver,
            PowerPlan::Balanced,
            PowerPlan::HighPerformance,
        ] {
            let r = worker_with(
                Job::Power(plan),
                |_| Ok(Completed::message("power changed")),
                || panic!("Independent performance must not access firmware"),
            );
            assert!(!r.error && !r.status_failed);
            assert!(r.status.is_none());
        }
    }
    #[test]
    fn independent_fan_and_performance_preserve_the_other_setting() {
        let mut c = demo_caps();
        c.fan_mode_raw = 64;
        let r = demo_worker(Job::Power(PowerPlan::PowerSaver), &mut c, &mut false);
        assert_eq!(c.fan_mode_raw, 64);
        assert_eq!(r.power, Some(PowerPlan::PowerSaver));
        let r = demo_worker(Job::Fan(FanMode::Auto), &mut c, &mut false);
        assert_eq!(c.fan_mode_raw, 0);
        assert!(r.power.is_none());
        for fan in [FanMode::Auto, FanMode::Boost] {
            for plan in [
                PowerPlan::PowerSaver,
                PowerPlan::Balanced,
                PowerPlan::HighPerformance,
            ] {
                let r = demo_worker(Job::Combination(fan, plan), &mut c, &mut false);
                assert_eq!(c.fan_mode_raw, fan.byte());
                assert_eq!(r.power, Some(plan));
            }
        }
    }
    #[test]
    fn passive_full_recovery_reads_once_and_failed_narrow_poll_discards_state() {
        let r = worker_with(Job::ReconnectPoll, operation, || Ok(ready()));
        assert!(r.passive && !r.error);
        assert!(r.status.is_some());
        let r = worker_with(
            Job::Poll,
            |_| Err("read failed".into()),
            || panic!("no redundant full read"),
        );
        assert!(r.passive && r.status_failed && r.error);
    }
    #[test]
    fn restoring_original_controller_is_success_when_it_owns_hardware() {
        let r = worker_with(
            Job::Restore,
            |_| Ok(Completed::message("restored")),
            || {
                let mut s = ready();
                s.writes_allowed = false;
                s.ownership_reason = Some("OEM service active".into());
                Ok(s)
            },
        );
        assert!(!r.error);
        assert!(r.stop_monitor);
        assert!(!r.status.unwrap().writes_allowed);
    }
}
