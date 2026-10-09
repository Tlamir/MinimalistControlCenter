//! Native Win32 widgets and an isolated, bounded hardware worker. No web runtime.
use mcc::{
    control::{FanMode, Switch},
    lighting::{Effect, Lighting},
    settings::{PowerPlan, Profile, Settings},
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
    UI::{Input::KeyboardAndMouse::EnableWindow, Shell::*, WindowsAndMessaging::*},
};

const UPDATE: u32 = WM_APP + 1;
const TRAY: u32 = WM_APP + 2;
const CONNECT: u16 = 101;
const ADMIN: u16 = 102;
const AUTO: u16 = 103;
const BOOST: u16 = 104;
const QUIET: u16 = 105;
const BALANCED: u16 = 106;
const PERFORMANCE: u16 = 107;
const APPLY_LIGHT: u16 = 108;
const SCREEN_OFF: u16 = 109;
const GPU: u16 = 110;
const KEY_ON: u16 = 111;
const KEY_OFF: u16 = 112;
const USB_ON: u16 = 113;
const USB_OFF: u16 = 114;
const BAR_ON: u16 = 115;
const BAR_OFF: u16 = 116;
const APPLY_PROFILE: u16 = 117;
const SAVE_PROFILE: u16 = 118;
const HIDE: u16 = 119;
const QUIT: u16 = 120;
const GPU_TEMP: u16 = 121;
const PAUSE_OEM: u16 = 122;
const RESUME_OEM: u16 = 123;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
#[derive(Clone)]
enum Job {
    Status,
    Fan(FanMode),
    Switch(Switch, bool),
    Light(Lighting),
    Power(PowerPlan),
    Profile(Profile),
    Gpu,
    ResponsivenessTest,
    Handover(bool),
}
struct Completed {
    status: Option<mcc::hardware::HardwareStatus>,
    message: String,
}
struct App {
    hwnd: HWND,
    font: HFONT,
    dashboard: HWND,
    status: HWND,
    fan_label: HWND,
    effect: HWND,
    brightness: HWND,
    speed: HWND,
    color: HWND,
    direction: HWND,
    profile: HWND,
    controls: Vec<(u16, HWND)>,
    tx: SyncSender<Job>,
    rx: Receiver<Completed>,
    busy: bool,
    caps: Option<mcc::control::Capabilities>,
    settings: Settings,
    previous_cpu: Option<mcc::CpuTimes>,
    tray: NOTIFYICONDATAW,
    benchmark: Option<Benchmark>,
    test_completed: bool,
    ticks: u64,
    busy_ticks: u64,
    writes_allowed: bool,
}
struct Benchmark {
    start: Instant,
    seconds: u64,
    before: mcc::platform::ProcessMetrics,
    path: std::path::PathBuf,
    hidden: bool,
    self_test: bool,
}
thread_local! { static APP:RefCell<Option<App>>=const {RefCell::new(None)}; }

fn call(args: &[String]) -> Result<serde_json::Value, String> {
    use std::os::windows::process::CommandExt;
    let path = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .with_file_name("mcc.exe");
    let mut command = std::process::Command::new(path);
    command.args(args).creation_flags(0x08000000);
    let bytes = mcc::process::run(&mut command, Duration::from_secs(5)).map_err(|e| {
        format!("Hardware helper: {e:?}. Request stopped; settings may need checking")
    })?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if value["ok"] != true {
        return Err(value["error"]
            .as_str()
            .unwrap_or("Unknown helper failure")
            .to_owned());
    }
    Ok(value)
}
fn hardware_status() -> Result<mcc::hardware::HardwareStatus, String> {
    serde_json::from_value(call(&["hardware".into(), "status".into()])?["status"].clone())
        .map_err(|e| e.to_string())
}
fn fan_args(f: FanMode) -> Vec<String> {
    vec![
        "hardware".into(),
        match f {
            FanMode::Auto => "fan-auto",
            FanMode::Boost => "fan-boost",
        }
        .into(),
    ]
}
fn power_args(p: PowerPlan) -> Vec<String> {
    vec![
        "power".into(),
        match p {
            PowerPlan::Balanced => "balanced",
            PowerPlan::PowerSaver => "saver",
            PowerPlan::HighPerformance => "performance",
        }
        .into(),
    ]
}
fn light_args(c: &Lighting) -> Vec<String> {
    vec![
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
    ]
}
fn worker(job: Job) -> Result<Completed, String> {
    match job {
        Job::Handover(start) => {
            call(&[
                "hardware".into(),
                if start { "resume-oem" } else { "pause-oem" }.into(),
            ])?;
            Ok(Completed{status:Some(hardware_status()?),message:if start {"OEM launcher restored. Exit this app before using the original controller."}else{"OEM launcher paused for this session. Its startup configuration is unchanged. Firmware remains in automatic mode."}.into()})
        }
        Job::ResponsivenessTest => {
            std::thread::sleep(Duration::from_millis(500));
            Ok(Completed {
                status: None,
                message: "Synthetic worker delay completed; no hardware was accessed.".into(),
            })
        }
        Job::Gpu => {
            let v = call(&["gpu".into()])?;
            Ok(Completed {
                status: None,
                message: format!(
                    "NVIDIA GPU temperature: {} °C (one sample). GPU reads are opt-in because they may wake the discrete GPU.",
                    v["gpu_temperature_c"]
                ),
            })
        }
        Job::Status => {
            let s = hardware_status()?;
            let message = if s.writes_allowed {
                "Connected. Firmware controls ready; temperature and RPM calibration is still required.".into()
            } else {
                s.ownership_reason.clone().unwrap_or_default()
            };
            Ok(Completed {
                status: Some(s),
                message,
            })
        }
        Job::Fan(f) => {
            call(&fan_args(f))?;
            Ok(Completed {
                status: Some(hardware_status()?),
                message: format!(
                    "Fan mode: {f:?}. Mode register readback confirmed; cooling remains managed by firmware."
                ),
            })
        }
        Job::Switch(s, on) => {
            let a = match (s, on) {
                (Switch::WindowsKeyLock, true) => "key-lock-on",
                (Switch::WindowsKeyLock, false) => "key-lock-off",
                (Switch::UsbCharging, true) => "usb-on",
                (Switch::UsbCharging, false) => "usb-off",
                (Switch::LightBar, true) => "light-bar-on",
                (Switch::LightBar, false) => "light-bar-off",
            };
            call(&["hardware".into(), a.into()])?;
            Ok(Completed {
                status: Some(hardware_status()?),
                message: "Setting applied and readback confirmed.".into(),
            })
        }
        Job::Light(c) => {
            call(&light_args(&c))?;
            Ok(Completed{status:None,message:"Keyboard effect submitted to firmware. Observe the keyboard to confirm the result.".into()})
        }
        Job::Power(p) => {
            call(&power_args(p))?;
            Ok(Completed {
                status: None,
                message: format!("Windows power plan: {p:?}. Fan mode was not changed."),
            })
        }
        Job::Profile(p) => {
            call(&fan_args(p.fan))?;
            call(&power_args(p.power))
                .map_err(|e| format!("Fan mode applied; power plan failed: {e}"))?;
            call(&light_args(&p.lighting))
                .map_err(|e| format!("Fan mode and power plan applied; keyboard failed: {e}"))?;
            Ok(Completed {
                status: Some(hardware_status()?),
                message: format!(
                    "Profile '{}' applied. Firmware manages cooling; no background policy loop is needed.",
                    p.name
                ),
            })
        }
    }
}

fn text(hwnd: HWND, value: &str) {
    // SAFETY: The UI owns this live control handle; UTF-16 remains valid throughout the call.
    unsafe {
        SetWindowTextW(hwnd, wide(value).as_ptr());
    }
}
fn read_text(hwnd: HWND) -> String {
    let mut buffer = [0u16; 64];
    // SAFETY: Writable buffer size matches supplied count; handle belongs to this UI.
    let count = unsafe { GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32) };
    String::from_utf16_lossy(&buffer[..count.max(0) as usize])
}
fn selection(hwnd: HWND) -> usize {
    // SAFETY: Documented combo selection message without pointer arguments.
    unsafe { SendMessageW(hwnd, CB_GETCURSEL, 0, 0).max(0) as usize }
}
fn select(hwnd: HWND, index: usize) {
    // SAFETY: Documented combo selection message, index bound by the caller's list.
    unsafe {
        SendMessageW(hwnd, CB_SETCURSEL, index, 0);
    }
}
impl App {
    // Mirrors CreateWindowExW's control geometry and style arguments.
    #[allow(clippy::too_many_arguments)]
    fn child(
        &mut self,
        class: &str,
        label: &str,
        id: u16,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        style: u32,
    ) -> HWND {
        // SAFETY: Parent HWND is live; strings are NUL terminated, control ID is an integer HMENU.
        let control = unsafe {
            CreateWindowExW(
                0,
                wide(class).as_ptr(),
                wide(label).as_ptr(),
                WS_CHILD | WS_VISIBLE | style,
                x,
                y,
                w,
                h,
                self.hwnd,
                id as usize as HMENU,
                GetModuleHandleW(ptr::null()),
                ptr::null(),
            )
        };
        // SAFETY: WM_SETFONT receives a live GDI font owned for the window lifetime.
        unsafe {
            SendMessageW(control, WM_SETFONT, self.font as usize, 1);
        }
        self.controls.push((id, control));
        control
    }
    fn label(&mut self, t: &str, x: i32, y: i32, w: i32, h: i32) -> HWND {
        self.child("STATIC", t, 0, x, y, w, h, 0)
    }
    fn button(&mut self, t: &str, id: u16, x: i32, y: i32, w: i32) -> HWND {
        self.child(
            "BUTTON",
            t,
            id,
            x,
            y,
            w,
            32,
            WS_TABSTOP | BS_PUSHBUTTON as u32,
        )
    }
    fn combo(&mut self, items: &[&str], x: i32, y: i32, w: i32, index: usize) -> HWND {
        let control = self.child(
            "COMBOBOX",
            "",
            0,
            x,
            y,
            w,
            220,
            WS_TABSTOP | CBS_DROPDOWNLIST as u32 | WS_VSCROLL,
        );
        for item in items {
            // SAFETY: CB_ADDSTRING copies the NUL terminated text before returning.
            unsafe {
                SendMessageW(control, CB_ADDSTRING, 0, wide(item).as_ptr() as isize);
            }
        }
        select(control, index);
        control
    }
    fn enable_controls(&self, allowed: bool) {
        for &(id, h) in &self.controls {
            let enabled = match id {
                CONNECT | ADMIN | QUIET | BALANCED | PERFORMANCE | APPLY_LIGHT | APPLY_PROFILE
                | PAUSE_OEM | RESUME_OEM => !self.busy,
                AUTO => allowed && !self.busy,
                BOOST => allowed && !self.busy && self.caps.as_ref().is_some_and(|c| c.boost),
                KEY_ON | KEY_OFF => {
                    allowed && !self.busy && self.caps.as_ref().is_some_and(|c| c.windows_key_lock)
                }
                USB_ON | USB_OFF => {
                    allowed && !self.busy && self.caps.as_ref().is_some_and(|c| c.usb_charging)
                }
                BAR_ON | BAR_OFF => {
                    allowed && !self.busy && self.caps.as_ref().is_some_and(|c| c.light_bar)
                }
                _ => true,
            };
            // SAFETY: Only UI-owned HWNDs are used, on their owning thread.
            unsafe {
                EnableWindow(h, i32::from(enabled));
            }
        }
    }
    fn submit(&mut self, job: Job) {
        if self.busy {
            return;
        }
        if self.tx.try_send(job).is_ok() {
            self.busy = true;
            self.enable_controls(false);
            text(
                self.status,
                "Working… The UI stays responsive while the hardware helper runs.",
            );
        }
    }
    fn lighting(&self) -> Result<Lighting, String> {
        let hex = read_text(self.color);
        let args = vec![
            "lighting".into(),
            ["off", "solid", "breathing", "rainbow", "wave"][selection(self.effect).min(4)].into(),
            selection(self.brightness).to_string(),
            selection(self.speed).to_string(),
            hex,
            (selection(self.direction) + 1).to_string(),
        ];
        match mcc::cli::parse(&args)? {
            mcc::cli::Command::Lighting { config } => Ok(config),
            _ => unreachable!(),
        }
    }
    fn show_lighting(&self, c: &Lighting) {
        select(
            self.effect,
            match c.effect {
                Effect::Off => 0,
                Effect::Solid => 1,
                Effect::Breathing => 2,
                Effect::Rainbow => 3,
                Effect::Wave => 4,
                Effect::Reactive => 5,
            },
        );
        select(self.brightness, c.brightness as usize);
        select(self.speed, c.speed as usize);
        select(self.direction, c.direction as usize - 1);
        text(
            self.color,
            &format!("{:02X}{:02X}{:02X}", c.rgb[0], c.rgb[1], c.rgb[2]),
        );
    }
    fn update(&mut self) {
        while let Ok(result) = self.rx.try_recv() {
            if result.message.starts_with("Synthetic worker") {
                self.test_completed = true;
            }
            self.busy = false;
            if let Some(s) = result.status {
                text(
                    self.fan_label,
                    &format!(
                        "Mode byte: 0x{:02X}   •   Windows key: {}   •   USB charging: {}   •   Light bar: {}",
                        s.capabilities.fan_mode_raw,
                        if s.capabilities.windows_key_locked {
                            "locked"
                        } else {
                            "enabled"
                        },
                        if s.capabilities.usb_charging_enabled {
                            "on"
                        } else {
                            "off"
                        },
                        if s.capabilities.light_bar_enabled {
                            "on"
                        } else {
                            "off"
                        }
                    ),
                );
                self.caps = Some(s.capabilities);
                self.writes_allowed = s.writes_allowed;
            }
            if result.message.starts_with("Request failed:") {
                self.writes_allowed = false;
            }
            text(self.status, &result.message);
            self.enable_controls(self.writes_allowed);
        }
    }
    fn dashboard(&mut self) {
        let s = mcc::platform::snapshot();
        let cpu = s.system_cpu_times.value.and_then(|current| {
            let value = self
                .previous_cpu
                .and_then(|prev| mcc::cpu_usage(prev, current));
            self.previous_cpu = Some(current);
            value
        });
        let cpu = cpu
            .map(|v| format!("{v:.1}%"))
            .unwrap_or("sampling…".into());
        let mem = s
            .memory_used_percent
            .value
            .map(|v| format!("{v}%"))
            .unwrap_or("unknown".into());
        let bat = s
            .battery_percent
            .value
            .map(|v| format!("{v}%"))
            .unwrap_or("unknown".into());
        let power = match s.ac_connected.value {
            Some(true) => "AC",
            Some(false) => "Battery",
            None => "Unknown",
        };
        let own = s
            .process
            .value
            .map(|v| format!("{:.1} MiB", v.working_set_bytes as f64 / 1048576.))
            .unwrap_or("unknown".into());
        text(
            self.dashboard,
            &format!(
                "CPU {cpu}     Memory {mem}     Battery {bat} / {power}     App memory {own}\r\nCPU temperature / fan RPM: awaiting calibration. GPU polling is opt-in to preserve battery."
            ),
        );
    }
    fn command(&mut self, id: u16) {
        match id {
            CONNECT => self.submit(Job::Status),
            PAUSE_OEM => self.submit(Job::Handover(false)),
            RESUME_OEM => self.submit(Job::Handover(true)),
            GPU_TEMP => self.submit(Job::Gpu),
            ADMIN => {
                if let Ok(path) = std::env::current_exe() {
                    // SAFETY: Absolute current executable, fixed parameters and verb; no shell string evaluation.
                    let r = unsafe {
                        ShellExecuteW(
                            self.hwnd,
                            wide("runas").as_ptr(),
                            wide(&path.to_string_lossy()).as_ptr(),
                            wide("--connect").as_ptr(),
                            ptr::null(),
                            SW_SHOWNORMAL,
                        )
                    };
                    if r as usize > 32 {
                        // SAFETY: Request close of this live window after successfully launching the new instance.
                        unsafe {
                            PostMessageW(self.hwnd, WM_CLOSE, 1, 0);
                        }
                    } else {
                        text(
                            self.status,
                            "Administrator launch canceled or denied. No hardware changes were made.",
                        );
                    }
                }
            }
            AUTO => self.submit(Job::Fan(FanMode::Auto)),
            BOOST => self.submit(Job::Fan(FanMode::Boost)),
            QUIET => self.submit(Job::Power(PowerPlan::PowerSaver)),
            BALANCED => self.submit(Job::Power(PowerPlan::Balanced)),
            PERFORMANCE => self.submit(Job::Power(PowerPlan::HighPerformance)),
            KEY_ON => self.submit(Job::Switch(Switch::WindowsKeyLock, true)),
            KEY_OFF => self.submit(Job::Switch(Switch::WindowsKeyLock, false)),
            USB_ON => self.submit(Job::Switch(Switch::UsbCharging, true)),
            USB_OFF => self.submit(Job::Switch(Switch::UsbCharging, false)),
            BAR_ON => self.submit(Job::Switch(Switch::LightBar, true)),
            BAR_OFF => self.submit(Job::Switch(Switch::LightBar, false)),
            APPLY_LIGHT => match self.lighting() {
                Ok(c) => self.submit(Job::Light(c)),
                Err(e) => text(self.status, &e),
            },
            SCREEN_OFF => {
                if let Err(e) = mcc::desktop::screen_off() {
                    text(self.status, &e);
                }
            }
            GPU => {
                // SAFETY: Opens a fixed Windows Settings URI; no external executable path or command is supplied.
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
            APPLY_PROFILE => {
                if let Some(p) = self.settings.profiles.get(selection(self.profile)).cloned() {
                    self.show_lighting(&p.lighting);
                    self.submit(Job::Profile(p));
                }
            }
            SAVE_PROFILE => match self.lighting() {
                Ok(c) => {
                    let index = selection(self.profile);
                    if let Some(p) = self.settings.profiles.get_mut(index) {
                        p.lighting = c;
                    }
                    let result = std::env::current_exe()
                        .map_err(|e| e.to_string())
                        .and_then(|p| self.settings.save(&p.with_file_name("profiles.json")));
                    text(self.status,&match result {Ok(())=>"Profile saved beside the app. Saved profiles never apply automatically.".into(),Err(e)=>format!("Cannot save profile: {e}")});
                }
                Err(e) => text(self.status, &e),
            },
            HIDE => self.hide(),
            QUIT => {
                // SAFETY: Closing the UI does not need a running controller; all modes are firmware managed.
                unsafe {
                    PostMessageW(self.hwnd, WM_CLOSE, 1, 0);
                }
            }
            _ => {}
        }
    }
    fn hide(&self) {
        // SAFETY: UI-owned window and timer. No polling continues while hidden.
        unsafe {
            ShowWindow(self.hwnd, SW_HIDE);
            KillTimer(self.hwnd, 1);
        }
    }
    fn show(&mut self) {
        // SAFETY: UI-owned window and timer; one second updates of documented OS counters.
        unsafe {
            ShowWindow(self.hwnd, SW_RESTORE);
            SetForegroundWindow(self.hwnd);
            SetTimer(self.hwnd, 1, 1000, None);
        }
        self.previous_cpu = None;
        self.dashboard();
    }
}

unsafe extern "system" fn procedure(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if msg == WM_DESTROY {
        // SAFETY: Called by Windows on the owning UI thread.
        unsafe {
            PostQuitMessage(0);
        }
        return 0;
    }
    let handled = APP.with(|slot| {
        let Ok(mut a) = slot.try_borrow_mut() else {
            return false;
        };
        let Some(app) = a.as_mut() else { return false };
        match msg {
            WM_COMMAND => {
                app.command((w & 65535) as u16);
                true
            }
            UPDATE => {
                app.update();
                true
            }
            WM_TIMER => {
                app.ticks+=1;
                if app.busy {app.busy_ticks+=1;}
                if w==2 {
                    if let Some(b)=&app.benchmark {
                        if b.start.elapsed()>=Duration::from_secs(b.seconds) {
                            let after=mcc::platform::metrics().value;
                            let passed=!b.self_test || (app.test_completed && app.busy_ticks>=3 && app.controls.iter().all(|(_,h)|!h.is_null()));
                            let report=serde_json::json!({"ok":passed,"benchmark":"native_ui_os_counters_only","hidden":b.hidden,"wall_ms":b.start.elapsed().as_secs_f64()*1000.,"cpu_ms":after.map(|a|a.cpu_time_100ns.saturating_sub(b.before.cpu_time_100ns) as f64/10000.),"working_set_bytes":after.map(|a|a.working_set_bytes),"peak_working_set_bytes":after.map(|a|a.peak_working_set_bytes),"private_commit_bytes":after.map(|a|a.private_commit_bytes),"message_ticks":app.ticks,"ticks_during_worker_delay":app.busy_ticks,"worker_responsive":app.test_completed,"hardware_accessed":false,"note":"CPU time is quantized by Windows counters. This does not measure game FPS, cooling or improvement over OEM software."});
                            let _=std::fs::write(&b.path,serde_json::to_vec_pretty(&report).unwrap());
                            // SAFETY: Request final close after this test/benchmark deadline.
                            unsafe {PostMessageW(hwnd,WM_CLOSE,1,0);}
                        }
                    }
                    return true;
                }
                app.update();
                // SAFETY: Windows delivers this callback for this live HWND.
                if unsafe { IsIconic(hwnd) } == 0 {
                    app.dashboard();
                }
                true
            }
            TRAY if l as u32 == WM_LBUTTONUP || l as u32 == WM_LBUTTONDBLCLK => {
                app.show();
                true
            }
            TRAY if l as u32 == WM_RBUTTONUP => {
                let mut p = POINT::default();
                // SAFETY: Owned popup menu and live HWND; no pointer data crosses the event loop.
                unsafe {
                    GetCursorPos(&mut p);
                    let menu = CreatePopupMenu();
                    AppendMenuW(menu, MF_STRING, HIDE as usize, wide("Hide").as_ptr());
                    AppendMenuW(
                        menu,
                        MF_STRING,
                        CONNECT as usize,
                        wide("Refresh hardware").as_ptr(),
                    );
                    AppendMenuW(menu, MF_STRING, QUIT as usize, wide("Quit").as_ptr());
                    SetForegroundWindow(hwnd);
                    let id = TrackPopupMenu(
                        menu,
                        TPM_RETURNCMD | TPM_RIGHTBUTTON,
                        p.x,
                        p.y,
                        0,
                        hwnd,
                        ptr::null(),
                    );
                    DestroyMenu(menu);
                    if id != 0 {
                        app.command(id as u16);
                    }
                }
                true
            }
            WM_CLOSE => {
                if w == 1 {
                    // SAFETY: Final close on this window's own thread.
                    unsafe {
                        DestroyWindow(hwnd);
                    }
                } else {
                    app.hide();
                }
                true
            }
            WM_SIZE if w == SIZE_MINIMIZED as usize => {
                app.hide();
                true
            }
            _ => false,
        }
    });
    if handled {
        0
    } else {
        // SAFETY: Unhandled message is forwarded unchanged to the Windows default procedure.
        unsafe { DefWindowProcW(hwnd, msg, w, l) }
    }
}

pub fn run() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mode = match args.as_slice() {
        [] => None,
        [a] if a == "--connect" => None,
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
    let before = mcc::platform::metrics().value;
    // SAFETY: Module handle stays live for process lifetime; registered callback has the Win32 ABI.
    let (hwnd, font) = unsafe {
        let instance = GetModuleHandleW(ptr::null());
        let class = wide("MinimalistControlCenter.Native");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(procedure),
            hInstance: instance,
            lpszClassName: class.as_ptr(),
            hCursor: LoadCursorW(ptr::null_mut(), IDC_ARROW),
            hbrBackground: (COLOR_WINDOW + 1) as HBRUSH,
            ..Default::default()
        };
        if RegisterClassW(&wc) == 0 {
            return;
        }
        let hwnd = CreateWindowExW(
            0,
            class.as_ptr(),
            wide("Minimalist Control Center").as_ptr(),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            870,
            720,
            ptr::null_mut(),
            ptr::null_mut(),
            instance,
            ptr::null(),
        );
        let font = CreateFontW(
            -16,
            0,
            0,
            0,
            400,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            OUT_DEFAULT_PRECIS as u32,
            CLIP_DEFAULT_PRECIS as u32,
            CLEARTYPE_QUALITY as u32,
            DEFAULT_PITCH as u32,
            wide("Segoe UI").as_ptr(),
        );
        (hwnd, font)
    };
    if hwnd.is_null() {
        return;
    }
    let (tx, jobs) = mpsc::sync_channel(1);
    let (result_tx, rx) = mpsc::channel();
    let window_id = hwnd as usize;
    std::thread::spawn(move || {
        while let Ok(job) = jobs.recv() {
            let result = worker(job).unwrap_or_else(|e| Completed {
                status: None,
                message: format!("Request failed: {e}"),
            });
            if result_tx.send(result).is_err() {
                break;
            }
            // SAFETY: PostMessage queues an integer notification only; failure after window close is harmless.
            unsafe {
                PostMessageW(window_id as HWND, UPDATE, 0, 0);
            }
        }
    });
    let settings = std::env::current_exe()
        .ok()
        .and_then(|p| Settings::load(&p.with_file_name("profiles.json")).ok())
        .unwrap_or_default();
    let mut a = App {
        hwnd,
        font,
        dashboard: ptr::null_mut(),
        status: ptr::null_mut(),
        fan_label: ptr::null_mut(),
        effect: ptr::null_mut(),
        brightness: ptr::null_mut(),
        speed: ptr::null_mut(),
        color: ptr::null_mut(),
        direction: ptr::null_mut(),
        profile: ptr::null_mut(),
        controls: vec![],
        tx,
        rx,
        busy: false,
        caps: None,
        settings,
        previous_cpu: None,
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
        writes_allowed: false,
    };
    a.label("Minimalist Control Center", 24, 18, 450, 30);
    let id = mcc::platform::identity();
    a.label(
        &format!(
            "{}   /   {}   /   Native Rust + Win32",
            id.model.value.unwrap_or("Unknown model".into()),
            id.bios_version.value.unwrap_or("Unknown BIOS".into())
        ),
        24,
        50,
        800,
        24,
    );
    a.dashboard = a.label("Sampling OS counters…", 24, 82, 805, 52);
    a.label("Cooling", 24, 143, 600, 24);
    a.button("Connect / refresh", CONNECT, 24, 172, 160);
    a.button("Connect as admin", ADMIN, 196, 172, 160);
    a.button("Automatic fans", AUTO, 368, 172, 160);
    a.button("Cooling boost", BOOST, 540, 172, 160);
    a.button("Take control", PAUSE_OEM, 712, 172, 110);
    a.fan_label = a.label(
        "No EC access on startup. Connect to read firmware capabilities.",
        24,
        211,
        805,
        24,
    );
    a.label("Power plan", 24, 246, 650, 24);
    a.button("Quiet / Power Saver", QUIET, 24, 275, 190);
    a.button("Balanced", BALANCED, 226, 275, 150);
    a.button("High performance", PERFORMANCE, 388, 275, 190);
    a.button("Restore OEM controller", RESUME_OEM, 590, 275, 232);
    a.label(
        "Power Saver reduces CPU power. Select Automatic fans to leave boost mode.",
        24,
        313,
        800,
        24,
    );
    a.label("Keyboard • firmware effects", 24, 348, 760, 24);
    a.label("Effect", 24, 376, 160, 22);
    a.label("Brightness", 210, 376, 130, 22);
    a.label("Speed", 354, 376, 130, 22);
    a.label("RGB hex", 498, 376, 120, 22);
    a.label("Direction", 636, 376, 150, 22);
    a.effect = a.combo(
        &["Off", "Solid", "Breathing", "Rainbow", "Wave"],
        24,
        402,
        172,
        1,
    );
    a.brightness = a.combo(
        &["Off", "Low", "Medium", "High", "Maximum"],
        210,
        402,
        130,
        3,
    );
    a.speed = a.combo(
        &["Slowest", "Slow", "Medium", "Fast", "Fastest"],
        354,
        402,
        130,
        2,
    );
    a.color = a.child(
        "EDIT",
        "46A0FF",
        0,
        498,
        402,
        120,
        26,
        WS_BORDER | WS_TABSTOP | ES_AUTOHSCROLL as u32,
    );
    a.direction = a.combo(
        &[
            "Left → right",
            "Right → left",
            "Bottom → top",
            "Top → bottom",
        ],
        636,
        402,
        166,
        0,
    );
    a.button("Apply lighting", APPLY_LIGHT, 24, 441, 172);
    let names: Vec<String> = a.settings.profiles.iter().map(|p| p.name.clone()).collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    a.profile = a.combo(&names, 210, 441, 172, 0);
    a.button("Apply profile", APPLY_PROFILE, 394, 441, 150);
    a.button("Save lighting to profile", SAVE_PROFILE, 556, 441, 246);
    a.label("Convenience", 24, 489, 740, 24);
    a.button("Lock Win key", KEY_ON, 24, 518, 124);
    a.button("Unlock", KEY_OFF, 158, 518, 80);
    a.button("USB charge on", USB_ON, 250, 518, 135);
    a.button("Off", USB_OFF, 395, 518, 64);
    a.button("Light bar on", BAR_ON, 471, 518, 130);
    a.button("Off", BAR_OFF, 611, 518, 64);
    a.button("Screen off", SCREEN_OFF, 24, 558, 124);
    a.button("GPU preference", GPU, 158, 558, 170);
    a.button("GPU temperature", GPU_TEMP, 340, 558, 160);
    a.button("Hide to tray", HIDE, 512, 558, 140);
    a.button("Quit", QUIT, 664, 558, 138);
    a.status=a.label("Ready. No settings have been applied. No driver installation, undervolting or firmware writes.",24,611,805,70);
    a.enable_controls(false);
    a.dashboard();
    a.tray.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    a.tray.hWnd = hwnd;
    a.tray.uID = 1;
    a.tray.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    a.tray.uCallbackMessage = TRAY;
    // SAFETY: Shared system icon is not owned; tray structure remains valid through each call.
    unsafe {
        a.tray.hIcon = LoadIconW(ptr::null_mut(), IDI_APPLICATION);
    }
    let tip = wide("Minimalist Control Center — click to open");
    a.tray.szTip[..tip.len()].copy_from_slice(&tip);
    // SAFETY: Correct NOTIFYICONDATAW size and live HWND.
    unsafe {
        Shell_NotifyIconW(NIM_ADD, &a.tray);
    }
    let connect = args == ["--connect"];
    if connect {
        a.submit(Job::Status);
    }
    let hidden = a.benchmark.as_ref().is_some_and(|b| b.hidden);
    let test = a.benchmark.as_ref().is_some_and(|b| b.self_test);
    if test {
        a.submit(Job::ResponsivenessTest);
    }
    let benchmark = a.benchmark.is_some();
    APP.with(|slot| *slot.borrow_mut() = Some(a));
    // SAFETY: Standard Windows message loop and HWND; MSG is initialized before use.
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
            if IsDialogMessageW(hwnd, &msg) == 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
    APP.with(|slot| {
        if let Some(a) = slot.borrow_mut().take() {
            // SAFETY: Remove owned tray icon and font once after the message loop exits.
            unsafe {
                Shell_NotifyIconW(NIM_DELETE, &a.tray);
                DeleteObject(a.font);
            }
        }
    });
}
