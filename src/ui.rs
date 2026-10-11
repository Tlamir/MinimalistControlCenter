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
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
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
const PRESET_DAILY: u16 = 132;
const PRESET_HIGH: u16 = 133;
const PRESET_BOOST: u16 = 134;
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
const HIDE: u16 = 129;
const DETAILS: u16 = 130;
const SHOW: u16 = 131;
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
    StartupConnect,
    Status,
    Poll,
    StopObserver,
    ReconnectPoll,
    Preset(Preset),
    Fan(FanMode),
    Power(PowerPlan),
    Combination(FanMode, PowerPlan),
    Startup(bool),
    StartupStatus,
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
    gpu: Option<mcc::gpu::Sensors>,
    stop_monitor: bool,
    startup: Option<mcc::startup::Status>,
    startup_refreshed: bool,
    startup_attempt: bool,
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
            gpu: None,
            stop_monitor: false,
            startup: None,
            startup_refreshed: false,
            startup_attempt: false,
        }
    }
}
fn call(args: &[String]) -> Result<serde_json::Value, String> {
    let timeout = if args.first().is_some_and(|a| a == "startup") {
        15
    } else {
        5
    };
    call_with_timeout(args, timeout)
}
fn call_with_timeout(args: &[String], timeout: u64) -> Result<serde_json::Value, String> {
    use std::os::windows::process::CommandExt;
    let path = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .with_file_name("mcc.exe");
    let mut cmd = std::process::Command::new(path);
    // DETACHED_PROCESS avoids allocating a hidden console host for each request.
    // The runner supplies stdout/stderr pipes explicitly; job containment is unchanged.
    cmd.args(args).creation_flags(0x00000008);
    let bytes = mcc::process::run(&mut cmd, Duration::from_secs(timeout))
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
fn boot_hardware_status() -> Result<mcc::hardware::HardwareStatus, String> {
    serde_json::from_value(
        call_with_timeout(&["hardware".into(), "status".into()], 20)?["status"].clone(),
    )
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
        Job::Connect | Job::StartupConnect => {
            let startup = matches!(job, Job::StartupConnect);
            let s = if startup {
                boot_hardware_status()?
            } else {
                hardware_status()?
            };
            if !s.writes_allowed
                && s.ownership_reason
                    .as_deref()
                    .is_some_and(|r| r.starts_with("OEM service "))
            {
                call_with_timeout(
                    &["hardware".into(), "pause-oem".into()],
                    if startup { 20 } else { 5 },
                )?;
            }
            if startup {
                // Fresh desired-state command checks pending pulses and readback before writing.
                // Boot recovery must never replay a toggle or apply a fan/keyboard profile.
                call_with_timeout(&["hardware".into(), "light-bar-off".into()], 20)?;
                "Connected · light bar off."
            } else {
                "Connected."
            }
        }
        Job::Status | Job::ReconnectPoll => "Settings refreshed.",
        Job::Poll => {
            return Err("Observation reader unavailable".into());
        }
        Job::StopObserver => return Ok(Completed::message("")),
        Job::Startup(_) | Job::StartupStatus => {
            let action = match job {
                Job::Startup(true) => "on",
                Job::Startup(false) => "off",
                _ => "status",
            };
            let v = call(&["startup".into(), action.into()])?;
            let startup: mcc::startup::Status =
                serde_json::from_value(v["startup"].clone()).map_err(|e| e.to_string())?;
            let mut r = Completed::message(if !startup.enabled {
                "Windows startup is off."
            } else if startup.managed {
                "Windows startup on."
            } else {
                "Portable startup on."
            });
            r.startup = Some(startup);
            r.startup_refreshed = true;
            return Ok(r);
        }
        Job::Fan(mode) => {
            mcc::presets::apply_combination(
                Some(*mode),
                None,
                |f| call(&fan_args(f)).map(|_| ()),
                mcc::desktop::power,
            )?;
            "Fan mode updated."
        }
        Job::Power(plan) => {
            mcc::presets::apply_combination(
                None,
                Some(*plan),
                |f| call(&fan_args(f)).map(|_| ()),
                mcc::desktop::power,
            )?;
            "Power plan updated."
        }
        Job::Combination(fan, power) => {
            mcc::presets::apply_combination(
                Some(*fan),
                Some(*power),
                |f| call(&fan_args(f)).map(|_| ()),
                mcc::desktop::power,
            )?;
            "Custom preset applied."
        }
        Job::Preset(p) => {
            mcc::presets::apply(*p, |f| call(&fan_args(f)).map(|_| ()), mcc::desktop::power)?;
            match p {
                Preset::Office => "Office preset applied.",
                Preset::Balanced => "Balanced preset applied.",
                Preset::High => "Performance preset applied.",
                Preset::Turbo => "Boost preset applied.",
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
            let mut r = Completed::message("GPU sample updated.");
            r.gpu = Some(serde_json::from_value(v["sensors"].clone()).map_err(|e| e.to_string())?);
            return Ok(r);
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
fn worker(job: Job, observer: &mut Option<mcc::observer::Observer>) -> Completed {
    let startup = matches!(job, Job::StartupConnect);
    let mut r = worker_with(
        job,
        |job| {
            if matches!(job, Job::Poll) {
                if observer.is_none() {
                    let path = std::env::current_exe()
                        .map_err(|e| e.to_string())?
                        .with_file_name("mcc.exe");
                    *observer = Some(
                        mcc::observer::Observer::start(&path)
                            .map_err(|e| format!("Observation reader stopped: {e:?}"))?,
                    );
                }
                let result = observer
                    .as_mut()
                    .ok_or("Observation reader unavailable")?
                    .read(Duration::from_secs(5));
                let observation = match result {
                    Ok(o) => o,
                    Err(e) => {
                        *observer = None;
                        return Err(e);
                    }
                };
                let mut r = Completed::message("Fan state synchronized.");
                if !observation.writes_allowed {
                    r.error = true;
                    r.message = observation
                        .ownership_reason
                        .clone()
                        .unwrap_or("Original controller has control.".into());
                }
                r.observation = Some(observation);
                Ok(r)
            } else {
                operation(job)
            }
        },
        || {
            if startup {
                boot_hardware_status()
            } else {
                hardware_status()
            }
        },
    );
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
            | Job::StartupConnect
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
    r.startup_refreshed |= matches!(job, Job::Startup(_) | Job::StartupStatus);
    r.startup_attempt = matches!(job, Job::StartupConnect);
    if matches!(job, Job::StartupStatus) {
        r.passive = true;
    }
    r.passive |= matches!(job, Job::Poll | Job::ReconnectPoll);
    if matches!(job, Job::Poll) && r.error && r.observation.is_none() {
        r.status_failed = true;
    }
    r.stop_monitor = matches!(job, Job::Restore) && !r.error;
    // Recover by reading, never by replaying a possibly completed write.
    if needs_status {
        match status() {
            Ok(s) => {
                if matches!(job, Job::StartupConnect)
                    && s.writes_allowed
                    && s.capabilities.light_bar
                    && !s.capabilities.light_bar_enabled
                {
                    // A timed-out request may have completed. Verified desired state ends recovery.
                    r.error = false;
                    r.message = "Connected · light bar off.".into();
                }
                if matches!(job, Job::StartupConnect)
                    && s.capabilities.light_bar_enabled
                    && !r.error
                {
                    r.message = "Startup connected, but the light bar still reports on. Check Lighting before retrying.".into();
                    r.error = true;
                }
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
    r.startup_attempt = matches!(job, Job::StartupConnect);
    match job {
        Job::StartupConnect => c.light_bar_enabled = false,
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
        Job::Light(c) => {
            r.lighting = Some(c);
            r.message = "Preview only · keyboard selection simulated.".into();
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
    startup_test: bool,
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
    observation_scope: Arc<AtomicBool>,
    clock: Instant,
    visible: bool,
    startup: Option<mcc::startup::Status>,
    startup_recovery: Option<mcc::startup::Recovery>,
    startup_attempts: u8,
    message: String,
    error: bool,
    lighting: Lighting,
    last_lighting_sent: Option<Lighting>,
    power: Option<PowerPlan>,
    settings: Settings,
    previous_cpu: Option<mcc::CpuTimes>,
    stats: [String; 3],
    cpu_clock: String,
    gpu: Option<(mcc::gpu::Sensors, Instant)>,
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
    /// Render only our own drawing and controls into a bitmap. No screen capture or hardware calls.
    fn render_preview(&self, path: &std::path::Path) -> Result<(), String> {
        let width = self.px(720);
        let height = self.px(708);
        let mut info = BITMAPINFO::default();
        info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = width;
        info.bmiHeader.biHeight = height;
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        let mut bits = ptr::null_mut();
        // SAFETY: Fixed bounded dimensions, owned memory DC and bitmap. Restore selections before
        // releasing objects; copy pixels only after GDI has finished drawing into the DIB.
        let pixels = unsafe {
            let root = GetDC(self.hwnd);
            let dc = CreateCompatibleDC(root);
            ReleaseDC(self.hwnd, root);
            if dc.is_null() {
                return Err("Could not create preview DC".into());
            }
            let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, ptr::null_mut(), 0);
            if bitmap.is_null() || bits.is_null() {
                DeleteDC(dc);
                return Err("Could not create preview bitmap".into());
            }
            let old = SelectObject(dc, bitmap);
            self.paint(dc);
            for b in &self.buttons {
                let style = GetWindowLongW(b.hwnd, GWL_STYLE) as u32;
                if style & WS_VISIBLE == 0 {
                    continue;
                }
                let mut r = RECT::default();
                GetWindowRect(b.hwnd, &mut r);
                let mut origin = POINT {
                    x: r.left,
                    y: r.top,
                };
                ScreenToClient(self.hwnd, &mut origin);
                let saved = SaveDC(dc);
                SetViewportOrgEx(dc, origin.x, origin.y, ptr::null_mut());
                self.draw_button(&DRAWITEMSTRUCT {
                    CtlID: b.id as u32,
                    hwndItem: b.hwnd,
                    hDC: dc,
                    itemState: if style & WS_DISABLED != 0 {
                        ODS_DISABLED
                    } else {
                        0
                    },
                    rcItem: RECT {
                        left: 0,
                        top: 0,
                        right: r.right - r.left,
                        bottom: r.bottom - r.top,
                    },
                    ..Default::default()
                });
                RestoreDC(dc, saved);
            }
            GdiFlush();
            let pixels =
                std::slice::from_raw_parts(bits as *const u8, (width * height * 4) as usize)
                    .to_vec();
            SelectObject(dc, old);
            DeleteObject(bitmap);
            DeleteDC(dc);
            pixels
        };
        let mut bmp = Vec::with_capacity(54 + pixels.len());
        bmp.extend_from_slice(b"BM");
        bmp.extend_from_slice(&((54 + pixels.len()) as u32).to_le_bytes());
        bmp.extend_from_slice(&[0; 4]);
        bmp.extend_from_slice(&54u32.to_le_bytes());
        bmp.extend_from_slice(&40u32.to_le_bytes());
        bmp.extend_from_slice(&width.to_le_bytes());
        bmp.extend_from_slice(&height.to_le_bytes());
        bmp.extend_from_slice(&1u16.to_le_bytes());
        bmp.extend_from_slice(&32u16.to_le_bytes());
        bmp.extend_from_slice(&[0; 24]);
        bmp.extend_from_slice(&pixels);
        std::fs::write(path, bmp).map_err(|e| e.to_string())
    }
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
                        | PRESET_DAILY
                        | PRESET_HIGH
                        | PRESET_BOOST
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
            match self.startup {
                Some(s) if s.enabled => "On",
                Some(_) => "Off",
                None => "Check",
            },
        );
        caption(
            self.handle(GPU_TEMP),
            &self
                .gpu
                .as_ref()
                .and_then(|(s, _)| s.clock_mhz)
                .map(|v| format!("{v} MHz"))
                .unwrap_or("Read GPU".into()),
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
                REGULAR | PRESET_DAILY | PRESET_HIGH | SAVE_CUSTOM => self.session.ready(),
                PRESET_BOOST => self.session.ready() && c.is_some_and(|c| c.boost),
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
        if !matches!(job, Job::StartupConnect | Job::StartupStatus)
            && self.startup_recovery.is_some()
        {
            self.startup_recovery = None;
            // SAFETY: An explicit user action supersedes pending boot recovery.
            unsafe {
                KillTimer(self.hwnd, 3);
            }
        }
        let startup_check = matches!(job, Job::StartupStatus);
        let gpu_sample = matches!(job, Job::Gpu);
        if self.tx.try_send(job).is_ok() {
            if gpu_sample {
                self.gpu = None;
            }
            self.session.busy = true;
            self.message = if startup_check {
                "Checking Windows startup…"
            } else {
                "Applying…"
            }
            .into();
            self.error = false;
            self.refresh_controls();
        }
    }
    fn poll(&mut self) {
        if !self.visible || self.page != Page::Cooling {
            return;
        }
        self.sync_observer_scope();
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
            if let Some(gpu) = r.gpu.filter(|_| self.visible) {
                self.gpu = Some((gpu, Instant::now()));
            }
            let mut startup_exhausted = false;
            if r.startup_attempt {
                self.startup_attempts = self.startup_attempts.saturating_add(1);
                if let Some(recovery) = self.startup_recovery.as_mut() {
                    match recovery.completed(
                        self.clock.elapsed().as_millis() as u64,
                        !r.error && !r.status_failed,
                    ) {
                        mcc::startup::RecoveryOutcome::RetryAfter(delay) => {
                            // SAFETY: A sparse boot-only timer works while hidden; no dashboard sampling.
                            unsafe {
                                SetTimer(self.hwnd, 3, delay, None);
                            }
                        }
                        outcome => {
                            startup_exhausted =
                                matches!(outcome, mcc::startup::RecoveryOutcome::Exhausted);
                            self.startup_recovery = None;
                            // SAFETY: Cancel the boot-only timer on this live UI window.
                            unsafe {
                                KillTimer(self.hwnd, 3);
                            }
                        }
                    }
                }
            }
            if startup_exhausted && self.tray.cbSize != 0 {
                let mut tray = self.tray;
                tray.uFlags = NIF_INFO;
                tray.dwInfoFlags = NIIF_WARNING;
                let title = wide("Minimalist needs attention");
                tray.szInfoTitle[..title.len()].copy_from_slice(&title);
                let detail = wide(
                    "Couldn't connect after startup. Open Minimalist and choose Enable controls.",
                );
                tray.szInfo[..detail.len()].copy_from_slice(&detail);
                // SAFETY: Bounded UTF16 strings, initialized tray ID and callback window.
                unsafe {
                    Shell_NotifyIconW(NIM_MODIFY, &tray);
                }
            }
            if r.stop_monitor {
                self.monitor.enabled = false;
            } else if r.status.is_some()
                && !self.monitor.enabled
                && !self.demo
                && self.benchmark.is_none()
            {
                self.monitor.enabled = true;
            }
            if (r.passive && !r.startup_refreshed) || r.status.is_some() {
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
            if !r.passive || r.error || r.startup_refreshed {
                self.message = r.message;
                self.error = r.error;
            } else if !self.session.busy && old_caps != self.session.capabilities {
                self.message = "Settings updated.".into();
                self.error = false;
            }
            if r.startup_refreshed {
                self.startup = r.startup;
                // A passive startup check still occupies the worker queue.
                self.session.busy = false;
            }
            if r.power_refreshed || r.power.is_some() {
                self.power = r.power;
            }
            if let Some(c) = r.lighting {
                self.last_lighting_sent = Some(c.clone());
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
                || r.startup_refreshed
                || r.error
                || old_caps != self.session.capabilities
                || old_allowed != self.session.allowed
                || old_power != self.power
            {
                self.refresh_controls();
            }
        }
        self.sync_observer_scope();
    }
    fn dashboard(&mut self) {
        if !self.visible {
            return;
        }
        let begin = Instant::now();
        if self
            .gpu
            .as_ref()
            .is_some_and(|(_, t)| t.elapsed() >= Duration::from_secs(30))
        {
            self.gpu = None;
            self.refresh_controls();
        }
        let s = mcc::platform::dashboard_snapshot();
        let cpu = s
            .system_cpu_times
            .value
            .and_then(|now| {
                let v = self.previous_cpu.and_then(|prev| mcc::cpu_usage(prev, now));
                self.previous_cpu = Some(now);
                v
            })
            .map(|v| format!("{v:.0}%"))
            .unwrap_or("—".into());
        let memory = s
            .memory_used_percent
            .value
            .map(|v| format!("{v}%"))
            .unwrap_or("—".into());
        let battery = s
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
        let clock = s
            .cpu_clock_mhz
            .value
            .map(|v| format!("{:.2} GHz · OS", v as f64 / 1000.))
            .unwrap_or("—".into());
        let stats = [cpu, memory, battery];
        if self.stats != stats || self.cpu_clock != clock {
            self.stats = stats;
            self.cpu_clock = clock;
            let r = self.rect(32, 555, 656, 96);
            // SAFETY: Stack RECT copied by Windows; repaint only changed displayed values.
            unsafe {
                InvalidateRect(self.hwnd, &r, 0);
            }
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
                self.sync_observer_scope();
                self.refresh_controls();
                if id == COOLING && !self.demo && self.benchmark.is_none() {
                    self.monitor.wake();
                    self.poll();
                }
                if id == SETTINGS && !self.demo && self.benchmark.is_none() && !self.session.busy {
                    self.submit(Job::StartupStatus);
                }
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
            REGULAR | PRESET_DAILY | PRESET_HIGH | PRESET_BOOST => {
                if self.session.ready() {
                    self.submit(Job::Preset(match id {
                        REGULAR => Preset::Office,
                        PRESET_HIGH => Preset::High,
                        PRESET_BOOST => Preset::Turbo,
                        _ => Preset::Balanced,
                    }));
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
                    self.startup = Some(mcc::startup::Status {
                        enabled: !self.startup.is_some_and(|s| s.enabled),
                        managed: false,
                    });
                    self.refresh_controls();
                } else if self.startup.is_none() {
                    self.submit(Job::StartupStatus);
                } else {
                    self.submit(Job::Startup(!self.startup.is_some_and(|s| s.enabled)));
                }
            }
            CUSTOM => {
                self.message = "This laptop currently supports verified Automatic and Boost modes. Custom curves need reliable temperature readings and proven overheat recovery if the app stops. They remain unavailable until that protection is verified.".into();
                self.error = false;
                self.command(DETAILS);
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
            HIDE => self.hide(),
            SHOW => self.show(),
            DETAILS => {
                // SAFETY: Show the complete status as plain text in an owned native dialog.
                unsafe {
                    MessageBoxW(
                        self.hwnd,
                        wide(&self.message).as_ptr(),
                        wide(if self.error {
                            "Minimalist · Action failed"
                        } else {
                            "Minimalist · Status"
                        })
                        .as_ptr(),
                        MB_OK
                            | if self.error {
                                MB_ICONERROR
                            } else {
                                MB_ICONINFORMATION
                            },
                    );
                }
                self.invalidate();
            }
            _ => {}
        }
    }
    fn sync_observer_scope(&self) {
        let enabled = self.visible
            && self.page == Page::Cooling
            && self.monitor.enabled
            && self.session.allowed;
        if self.observation_scope.swap(enabled, Ordering::Relaxed) && !enabled {
            let _ = self.tx.try_send(Job::StopObserver);
        }
    }
    fn observation_visibility(&mut self, visible: bool) {
        if self.visible == visible {
            return;
        }
        self.visible = visible;
        self.sync_observer_scope();
        self.previous_cpu = None;
        if visible {
            self.monitor.wake();
            // SAFETY: One shared observation timer on the live UI window.
            unsafe {
                SetTimer(self.hwnd, 1, 1000, None);
            }
            self.dashboard();
        } else {
            self.gpu = None;
            caption(self.handle(GPU_TEMP), "Read GPU");
            // SAFETY: Suspend OS and firmware observations in tray and when minimized.
            unsafe {
                KillTimer(self.hwnd, 1);
            }
        }
    }
    fn hide(&mut self) {
        self.observation_visibility(false);
        // SAFETY: UI-owned HWND and timer; hidden dashboard polling stops.
        unsafe {
            ShowWindow(self.hwnd, SW_HIDE);
            KillTimer(self.hwnd, 1);
        }
    }
    fn show(&mut self) {
        self.visible = true;
        self.sync_observer_scope();
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
        self.refresh_controls();
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
        self.draw_text(
            dc,
            &self.model,
            self.rect(32, 24, 656, 34),
            2,
            INK,
            DT_SINGLELINE | DT_END_ELLIPSIS,
        );
        self.label(
            dc,
            &format!("Minimalist Control Center · {}", env!("CARGO_PKG_VERSION")),
            32,
            71,
            300,
            24,
            3,
            MUTED,
        );
        match self.page {
            Page::Cooling => {
                self.label(dc, "Quick presets", 32, 182, 400, 24, 1, INK);
                self.round(dc, self.rect(32, 328, 656, 163), WHITE, BORDER, 16);
                self.label(dc, "Fans", 52, 342, 400, 24, 1, INK);
                self.label(dc, "Windows power", 52, 417, 616, 24, 1, INK);
            }
            Page::Lighting => {
                self.label(dc, "Lighting", 32, 169, 380, 34, 2, INK);

                self.round(dc, self.rect(32, 238, 656, 263), WHITE, BORDER, 16);
                self.label(dc, "Keyboard", 52, 252, 320, 30, 1, INK);
                self.label(
                    dc,
                    if self.last_lighting_sent.as_ref() == Some(&self.lighting) {
                        "Sent"
                    } else {
                        "Pending"
                    },
                    200,
                    257,
                    280,
                    22,
                    3,
                    MUTED,
                );
                self.label(dc, "Effect", 52, 292, 280, 22, 3, MUTED);
                self.label(dc, "Brightness", 364, 292, 280, 22, 3, MUTED);
                self.label(dc, "Color", 52, 363, 280, 22, 3, MUTED);
                if matches!(
                    self.lighting.effect,
                    Effect::Breathing | Effect::Rainbow | Effect::Wave
                ) {
                    self.label(dc, "Animation speed", 52, 436, 280, 22, 3, MUTED);
                    self.label(dc, "Direction", 364, 436, 280, 22, 3, MUTED);
                }
                self.round(dc, self.rect(32, 497, 656, 54), WHITE, BORDER, 14);
                self.label(dc, "Light bar", 52, 505, 380, 24, 1, INK);
                self.label(dc, "Off at sign-in", 52, 530, 460, 18, 3, MUTED);
            }
            Page::Settings => {
                self.label(dc, "Settings", 32, 169, 380, 34, 2, INK);

                self.round(dc, self.rect(32, 238, 656, 140), WHITE, BORDER, 16);
                self.label(dc, "Lock Windows key", 52, 251, 460, 28, 1, INK);

                self.label(dc, "USB charging in hibernation", 52, 316, 460, 28, 1, INK);
                self.label(dc, "Supported USB port", 52, 343, 460, 25, 3, MUTED);
                self.round(dc, self.rect(32, 390, 656, 87), WHITE, BORDER, 16);
                self.label(dc, "Start with Windows", 52, 402, 420, 28, 1, INK);
                self.label(
                    dc,
                    if self.startup.is_some_and(|s| s.managed) {
                        "Start in tray · light bar off"
                    } else {
                        "Install to enable startup controls"
                    },
                    52,
                    441,
                    616,
                    20,
                    3,
                    MUTED,
                );
                self.label(dc, "Windows tools", 32, 491, 656, 24, 1, INK);
            }
        }
        self.paint_stats(dc);
        self.draw_text(
            dc,
            self.message.lines().next().unwrap_or(""),
            self.rect(32, 663, 554, 28),
            3,
            if self.error { ERROR } else { MUTED },
            DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS,
        );
    }
    fn paint_stats(&self, dc: HDC) {
        self.round(dc, self.rect(32, 558, 656, 90), WHITE, BORDER, 14);
        let sample = self
            .gpu
            .as_ref()
            .filter(|(_, time)| time.elapsed() < Duration::from_secs(30))
            .map(|(s, _)| s);
        let gpu_clock = sample
            .and_then(|s| s.clock_mhz)
            .map(|v| format!("{v} MHz"))
            .unwrap_or("—".into());
        let gpu_temp = sample
            .and_then(|s| s.temperature_c)
            .map(|v| format!("{v} °C · sample"))
            .unwrap_or("On demand".into());
        let gpu_fan = sample
            .and_then(|s| s.fan_rpm)
            .map(|v| format!("{v} RPM"))
            .unwrap_or("—".into());
        let values = [
            &self.stats[0],
            &gpu_clock,
            &gpu_fan,
            &self.stats[1],
            &self.stats[2],
        ];
        let notes = [
            self.cpu_clock.as_str(),
            gpu_temp.as_str(),
            if sample.is_some_and(|s| s.fan_rpm.is_some()) {
                "Sample"
            } else {
                "Unavailable"
            },
            "",
            "",
        ];
        for (i, name) in ["CPU", "GPU", "GPU fan", "Memory", "Battery"]
            .iter()
            .enumerate()
        {
            let x = 48 + i as i32 * 130;
            if i != 1 {
                self.label(dc, values[i], x, 568, 123, 26, 1, INK);
            }
            self.label(dc, notes[i], x, 595, 123, 20, 3, MUTED);
            self.label(dc, name, x, 622, 123, 20, 3, MUTED);
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
            STARTUP => self.startup.is_some_and(|s| s.enabled),
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
            REGULAR | PRESET_DAILY | PRESET_HIGH | PRESET_BOOST => {
                let p = Preset::observed(
                    self.session.capabilities.as_ref().map(|c| c.fan_mode_raw),
                    self.power,
                );
                p == Some(match id {
                    REGULAR => Preset::Office,
                    PRESET_HIGH => Preset::High,
                    PRESET_BOOST => Preset::Turbo,
                    _ => Preset::Balanced,
                })
            }
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
        } else if matches!(id, REGULAR | PRESET_DAILY | PRESET_HIGH | PRESET_BOOST) {
            let mut title = r;
            title.top += self.px(10);
            title.bottom = title.top + self.px(25);
            self.draw_text(d.hDC, &s, title, 1, color, DT_CENTER | DT_SINGLELINE);
            let mut detail = r;
            detail.top += self.px(39);
            self.draw_text(
                d.hDC,
                match id {
                    REGULAR => "Auto · Saver",
                    PRESET_HIGH => "Auto · Performance",
                    PRESET_BOOST => "Boost · Keep power",
                    _ => "Auto · Balanced",
                },
                detail,
                3,
                MUTED,
                DT_CENTER | DT_SINGLELINE,
            );
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
                if nav || primary {
                    1
                } else if id == HIDE || id == CUSTOM {
                    3
                } else {
                    0
                },
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
            WM_SHOWWINDOW => { a.observation_visibility(w != 0); true }
            WM_ERASEBKGND=>true,
            WM_PAINT=>{
                a.paints += 1;
                // SAFETY: Paired paint calls with initialized PAINTSTRUCT and live HWND.
                unsafe{let mut p=PAINTSTRUCT::default();let dc=BeginPaint(hwnd,&mut p);let saved=SaveDC(dc);let begin=Instant::now();if p.rcPaint.top >= a.px(555) && p.rcPaint.bottom <= a.px(651) {a.paint_stats(dc);} else {a.paint(dc);} a.paint_us+=begin.elapsed().as_micros() as u64;RestoreDC(dc,saved);EndPaint(hwnd,&p);}true
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
            WM_TIMER=>{
                a.ticks+=1;if a.session.busy{a.busy_ticks+=1;}
                if w==3 {
                    if a.startup_recovery.as_mut().is_some_and(|r| r.due(a.clock.elapsed().as_millis() as u64, a.session.busy)) {
                        // SAFETY: Cancel our boot timer before queueing one bounded retry.
                        unsafe { KillTimer(hwnd, 3); }
                        a.submit(Job::StartupConnect);
                    } else if a.startup_recovery.is_some() {
                        // SAFETY: Defer while a UI action is busy; no overlapping startup request.
                        unsafe { SetTimer(hwnd, 3, 1000, None); }
                    } else {
                        // SAFETY: Recovery ended; remove our timer from the live UI window.
                        unsafe { KillTimer(hwnd, 3); }
                    }
                } else if w==2 {
                    if let Some(b)=&a.benchmark{if b.start.elapsed()>=Duration::from_secs(b.seconds){
                        let after=mcc::platform::metrics().value;
                        let startup_passed=!b.startup_test || (a.startup_attempts==2 && !a.visible && a.startup_recovery.is_none() && a.session.allowed && a.session.capabilities.as_ref().is_some_and(|c| !c.light_bar_enabled && c.fan_mode_raw==64));
                        let passed=startup_passed && (!b.self_test || (a.test_completed && a.busy_ticks>=3 && a.pages_checked && a.buttons.iter().all(|b|!b.hwnd.is_null())));
                        let report=serde_json::json!({"ok":passed,"benchmark":"native_ui_os_counters_only","hidden":b.hidden,"startup_recovery_test":b.startup_test,"startup_attempts":a.startup_attempts,"wall_ms":b.start.elapsed().as_secs_f64()*1000.,"cpu_ms":after.map(|a|a.cpu_time_100ns.saturating_sub(b.before.cpu_time_100ns)as f64/10000.),"working_set_bytes":after.map(|a|a.working_set_bytes),"peak_working_set_bytes":after.map(|a|a.peak_working_set_bytes),"private_commit_bytes":after.map(|a|a.private_commit_bytes),"message_ticks":a.ticks,"ticks_during_worker_delay":a.busy_ticks,"worker_responsive":a.test_completed,"hardware_accessed":false,"pages":3,"font_faces":a.font_faces(),"paints":a.paints,"button_paints":a.button_paints,"paint_us":a.paint_us,"dashboard_us":a.dashboard_us,"pages_isolated_and_startup_writes_disabled":a.pages_checked,"note":"Includes startup. Windows CPU counters are quantized. No game FPS, thermal or OEM comparison."});
                        let _=std::fs::write(&b.path,serde_json::to_vec_pretty(&report).unwrap());
                        // SAFETY: Exit this UI after its bounded test/benchmark.
                        unsafe{PostMessageW(hwnd,WM_CLOSE,1,0);}
                    }}
                }else{a.dashboard();a.poll();}true
            }
            TRAY if l as u32==WM_LBUTTONUP || l as u32==WM_LBUTTONDBLCLK=>{a.show();true}
            TRAY if l as u32==WM_RBUTTONUP=>{
                // SAFETY: Scoped menu and POINT with fixed commands; destroy after use.
                unsafe{let mut p=POINT::default();GetCursorPos(&mut p);let menu=CreatePopupMenu();AppendMenuW(menu,MF_STRING,SHOW as usize,wide("Open").as_ptr());AppendMenuW(menu,MF_STRING,CONNECT as usize,wide(if a.session.allowed { "Refresh settings" } else { "Enable controls" }).as_ptr());AppendMenuW(menu,MF_SEPARATOR,0,ptr::null());AppendMenuW(menu,MF_STRING,QUIT as usize,wide("Exit").as_ptr());SetForegroundWindow(hwnd);let id=TrackPopupMenu(menu,TPM_RETURNCMD|TPM_RIGHTBUTTON,p.x,p.y,0,hwnd,ptr::null());DestroyMenu(menu);if id!=0{a.command(id as u16);}}true
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
    if args == ["--open-installed"] {
        if mcc::instance::show_existing() {
            return;
        }
        if let Err(e) = mcc::startup::launch() {
            // SAFETY: Fixed title and diagnostic only; task action cannot be supplied by callers.
            unsafe {
                MessageBoxW(
                    ptr::null_mut(),
                    wide(&e).as_ptr(),
                    wide("Minimalist startup").as_ptr(),
                    MB_OK | MB_ICONERROR,
                );
            }
            return;
        }
        let start = Instant::now();
        while !mcc::instance::show_existing() && start.elapsed() < Duration::from_secs(10) {
            std::thread::sleep(Duration::from_millis(100));
        }
        return;
    }
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
    let preview = match args.as_slice() {
        [flag, path] if flag == "--render-preview" => Some(std::path::PathBuf::from(path)),
        _ => None,
    };
    let startup_test = matches!(args.as_slice(), [flag, _] if flag == "--self-test-startup");
    let demo =
        args == ["--demo"] || args == ["--demo-failure"] || preview.is_some() || startup_test;
    let mode = match args.as_slice() {
        [] => None,
        [a] if a == "--connect"
            || a == "--tray"
            || a == "--startup-controls"
            || a == "--demo"
            || a == "--demo-failure" =>
        {
            None
        }
        [a, path] if a == "--self-test" => Some((2, std::path::PathBuf::from(path), true, true)),
        [a, path] if a == "--self-test-startup" => {
            Some((15, std::path::PathBuf::from(path), true, false))
        }
        [a, _] if a == "--render-preview" => None,
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
        match mcc::instance::acquire(args == ["--tray"] || args == ["--startup-controls"]) {
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
            hIcon: LoadIconW(instance, ptr::without_provenance(1)),
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
    // One active request plus an observer shutdown notification.
    let (tx, jobs) = mpsc::sync_channel(2);
    let (result_tx, rx) = mpsc::channel();
    let window_id = hwnd as usize;
    let fail_demo = args == ["--demo-failure"] || startup_test;
    let observation_scope = Arc::new(AtomicBool::new(false));
    let worker_scope = observation_scope.clone();
    std::thread::spawn(move || {
        let mut caps = demo_caps();
        if startup_test {
            caps.fan_mode_raw = 64;
        }
        let mut fail = fail_demo;
        let mut observer = None;
        while let Ok(job) = jobs.recv() {
            if !worker_scope.load(Ordering::Relaxed) || matches!(job, Job::StopObserver) {
                observer = None;
            }
            if matches!(job, Job::StopObserver) {
                continue;
            }
            let r = if startup_test && matches!(job, Job::StartupConnect) && fail {
                fail = false;
                let mut r = Completed::message("Synthetic boot connection timed out.");
                r.error = true;
                r.status_failed = true;
                r.startup_attempt = true;
                r
            } else if demo {
                demo_worker(job, &mut caps, &mut fail)
            } else {
                worker(job, &mut observer)
            };
            if !worker_scope.load(Ordering::Relaxed)
                || r.stop_monitor
                || r.status_failed
                || r.observation.as_ref().is_some_and(|o| !o.writes_allowed)
            {
                observer = None;
            }
            if result_tx.send(r).is_err() {
                break;
            }
            // SAFETY: Integer notification only; failure after window close harmless.
            unsafe {
                PostMessageW(window_id as HWND, UPDATE, 0, 0);
            }
        }
    });
    let loaded = std::env::current_exe()
        .map_err(|e| e.to_string())
        .and_then(|p| {
            let path = p.with_file_name("profiles.json");
            if path.exists() {
                Settings::load(&path)
            } else {
                Ok(Settings::default())
            }
        });
    let settings_error = loaded.as_ref().err().cloned();
    let settings = loaded.unwrap_or_default();
    let lighting = settings
        .profiles
        .iter()
        .find(|p| p.name == "Balanced")
        .map(|p| p.lighting.clone())
        .unwrap_or_default();
    let initially_visible = args != ["--tray"]
        && args != ["--startup-controls"]
        && !mode.as_ref().is_some_and(|(_, _, hidden, _)| *hidden);
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
        observation_scope,
        clock: Instant::now(),
        visible: initially_visible,
        startup: None,
        startup_recovery: (args == ["--startup-controls"] || startup_test).then(mcc::startup::Recovery::default),
        startup_attempts: 0,
        message: settings_error.as_ref().map(|e| format!("Preferences could not be loaded: {e}. Defaults shown; hardware was not changed."))
            .unwrap_or("Enable controls to connect.".into()),
        error: settings_error.is_some(),
        lighting,
        last_lighting_sent: None,
        power: if demo {
            Some(PowerPlan::Balanced)
        } else {
            mcc::desktop::active_power()
        },
        settings,
        previous_cpu: None,
        stats: Default::default(),
        cpu_clock: String::new(),
        gpu: None,
        tray: NOTIFYICONDATAW::default(),
        benchmark: mode.and_then(|(seconds, path, hidden, self_test)| {
            before.map(|before| Benchmark {
                start: Instant::now(),
                seconds,
                path,
                hidden,
                self_test,
                startup_test,
                before,
            })
        }),
        test_completed: false,
        ticks: 0,
        busy_ticks: 0,
        demo,
        model: if demo { "MONSTER TULPAR T5 V17.1".into() } else { mcc::platform::identity()
            .model
            .value
            .unwrap_or("Laptop".into()) },
        pages_checked: false,
        paints: 0,
        button_paints: 0,
        paint_us: 0,
        dashboard_us: 0,
    };
    caption(hwnd, &format!("{} · Minimalist Control Center", a.model));
    a.button("Enable controls", CONNECT, None, (340, 60, 164, 38));
    a.button("Hide", HIDE, None, (516, 60, 92, 38));
    a.button("Exit", QUIT, None, (620, 60, 68, 38));
    a.button("Cooling", COOLING, None, (32, 112, 213, 38));
    a.button("Lighting", LIGHTING, None, (253, 112, 213, 38));
    a.button("Settings", SETTINGS, None, (474, 112, 214, 38));
    let c = Some(Page::Cooling);
    let l = Some(Page::Lighting);
    let s = Some(Page::Settings);
    a.button("Automatic", AUTO, c, (52, 372, 298, 32));
    a.button("Boost", BOOST, c, (370, 372, 298, 32));
    a.button("Power Saver", QUIET, c, (52, 446, 194, 32));
    a.button("Balanced", BALANCED, c, (262, 446, 194, 32));
    a.button("Performance", PERFORMANCE, c, (474, 446, 194, 32));
    a.button("Office", REGULAR, c, (32, 216, 155, 70));
    a.button("Balanced", PRESET_DAILY, c, (199, 216, 155, 70));
    a.button("Performance", PRESET_HIGH, c, (366, 216, 155, 70));
    a.button("Boost", PRESET_BOOST, c, (533, 216, 155, 70));
    a.button("Save current", SAVE_CUSTOM, c, (256, 506, 208, 34));
    a.button("Custom preset", APPLY_CUSTOM, c, (32, 506, 208, 34));
    a.button("Fan support", CUSTOM, c, (480, 506, 208, 34));
    a.button("Apply keyboard", APPLY_LIGHT, l, (496, 250, 172, 36));
    a.button("Solid", EFFECT, l, (52, 317, 286, 34));
    a.button("High", BRIGHTNESS, l, (364, 317, 304, 34));
    for (i, name) in ["Blue", "Purple", "Green", "Red", "Amber", "White"]
        .iter()
        .enumerate()
    {
        a.button(
            name,
            SWATCH + i as u16,
            l,
            (52 + i as i32 * 50, 388, 40, 36),
        );
    }
    a.button("Custom color…", COLOR, l, (364, 388, 304, 36));
    a.button("Medium", SPEED, l, (52, 461, 286, 34));
    a.button("Left → right", DIRECTION, l, (364, 461, 304, 34));
    a.button("Unavailable", BAR, l, (548, 506, 120, 32));
    a.button("Unavailable", KEY, s, (548, 261, 120, 34));
    a.button("Unavailable", USB, s, (548, 326, 120, 34));
    a.button("Off", STARTUP, s, (548, 405, 120, 34));
    a.button("Turn screen off", SCREEN_OFF, s, (32, 515, 316, 34));
    a.button("Graphics settings", GPU, s, (372, 515, 316, 34));
    a.button("Read GPU", GPU_TEMP, None, (178, 565, 123, 30));
    a.button("Use original controller", RESTORE, s, (466, 171, 222, 34));
    a.button("Details", DETAILS, None, (596, 663, 92, 32));
    if demo {
        a.session.complete(Some((demo_caps(), true)), false);
        a.message = "Preview only · all hardware actions are simulated.".into();
    }
    a.refresh_controls();
    if a.benchmark.as_ref().is_some_and(|b| b.self_test) {
        a.pages_checked = a.check_pages();
    }
    a.dashboard();
    if let Some(path) = preview {
        if std::fs::create_dir_all(&path).is_ok() {
            a.stats = ["4%".into(), "40%".into(), "84% · AC".into()];
            a.startup = Some(mcc::startup::Status {
                enabled: true,
                managed: true,
            });
            for (page, name) in [
                (Page::Cooling, "cooling"),
                (Page::Lighting, "lighting"),
                (Page::Settings, "settings"),
            ] {
                a.page = page;
                a.refresh_controls();
                if let Err(e) = a.render_preview(&path.join(format!("{name}.bmp"))) {
                    eprintln!("Preview: {e}");
                }
            }
        }
        // SAFETY: Preview never shows a window or adds a tray icon. Release only owned objects.
        unsafe {
            DestroyWindow(hwnd);
            for font in a.fonts {
                DeleteObject(font);
            }
        }
        return;
    }
    a.tray.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    a.tray.hWnd = hwnd;
    a.tray.uID = 1;
    a.tray.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    a.tray.uCallbackMessage = TRAY;
    // SAFETY: Shared system icon; correct tray structure and live callback window.
    unsafe {
        a.tray.hIcon = LoadIconW(GetModuleHandleW(ptr::null()), ptr::without_provenance(1));
    }
    let tip = wide("Minimalist — click to open");
    a.tray.szTip[..tip.len()].copy_from_slice(&tip);
    // SAFETY: Fully initialized tray structure copied by Windows.
    unsafe {
        Shell_NotifyIconW(NIM_ADD, &a.tray);
    }
    if args == ["--connect"] || args == ["--startup-controls"] || startup_test {
        a.submit(if args == ["--startup-controls"] || startup_test {
            Job::StartupConnect
        } else {
            Job::Connect
        });
    }
    let hidden = args == ["--tray"]
        || args == ["--startup-controls"]
        || a.benchmark.as_ref().is_some_and(|b| b.hidden);
    a.visible = !hidden;
    let test = a.benchmark.as_ref().is_some_and(|b| b.self_test);
    let benchmark = a.benchmark.is_some();
    if test {
        a.submit(Job::ResponsivenessTest);
    } else if !demo && !benchmark && !a.session.busy {
        a.submit(Job::StartupStatus);
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
    fn startup_status_never_accesses_hardware_and_reports_unknown_on_failure() {
        let r = worker_with(
            Job::StartupStatus,
            |_| Err("provider unavailable".into()),
            || panic!("Startup status cannot access firmware"),
        );
        assert!(r.passive && r.error && r.startup_refreshed);
        assert!(r.startup.is_none());
        assert!(!r.status_failed);
    }
    #[test]
    fn startup_light_bar_failure_is_read_back_without_replaying_the_write() {
        let attempts = std::cell::Cell::new(0);
        let r = worker_with(
            Job::StartupConnect,
            |_| {
                attempts.set(attempts.get() + 1);
                Err("light-bar acknowledgement timed out".into())
            },
            || Ok(ready()),
        );
        assert_eq!(attempts.get(), 1);
        assert!(r.error && r.startup_attempt);
        assert!(!r.status_failed);
        assert!(r.status.is_some());
        let r = worker_with(
            Job::StartupConnect,
            |_| Ok(Completed::message("submitted")),
            || Ok(ready()),
        );
        assert!(r.error);
        assert!(r.message.contains("still reports on"));
    }
    #[test]
    fn late_boot_acknowledgement_finishes_once_fresh_status_confirms_bar_off() {
        let attempts = std::cell::Cell::new(0);
        let r = worker_with(
            Job::StartupConnect,
            |_| {
                attempts.set(attempts.get() + 1);
                Err("late acknowledgement".into())
            },
            || {
                let mut s = ready();
                s.capabilities.light_bar_enabled = false;
                Ok(s)
            },
        );
        assert_eq!(attempts.get(), 1);
        assert!(!r.error && !r.status_failed);
        assert!(r.message.contains("light bar off"));
        let r = worker_with(
            Job::StartupConnect,
            |_| Err("timed out".into()),
            || {
                let mut s = ready();
                s.capabilities.light_bar_enabled = false;
                s.writes_allowed = false;
                Ok(s)
            },
        );
        assert!(r.error);
    }
    #[test]
    fn installed_logon_switch_preserves_physical_fan_mode_and_never_applies_keyboard_or_power() {
        let mut caps = demo_caps();
        caps.fan_mode_raw = 64;
        let r = demo_worker(Job::StartupConnect, &mut caps, &mut false);
        assert!(!caps.light_bar_enabled);
        assert_eq!(caps.fan_mode_raw, 64);
        assert!(r.lighting.is_none() && r.power.is_none());
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
