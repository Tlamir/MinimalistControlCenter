//! Persistent hardware agent IPC. One elevated helper keeps the firmware connection warm and
//! serves typed requests over stdin/stdout. The parent contains it in a kill-on-close job, kills
//! it on timeout, and never replays a write: recovery always re-reads status instead.
use crate::{
    control::{FanMode, Switch},
    hardware::{HardwareStatus, Observation},
    process::{ChildJob, OUTPUT_LIMIT, RunError},
};
use serde::{Deserialize, Serialize};
use std::{
    io::{BufRead, BufReader, Read, Write},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant},
};

/// The complete set of operations the agent accepts. Anything else is rejected unparsed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Request {
    Observe,
    Status,
    Fan(FanMode),
    Switch(Switch, bool),
}
const VERBS: [(&str, Request); 10] = [
    ("read", Request::Observe),
    ("status", Request::Status),
    ("fan-auto", Request::Fan(FanMode::Auto)),
    ("fan-boost", Request::Fan(FanMode::Boost)),
    ("key-lock-on", Request::Switch(Switch::WindowsKeyLock, true)),
    (
        "key-lock-off",
        Request::Switch(Switch::WindowsKeyLock, false),
    ),
    ("usb-on", Request::Switch(Switch::UsbCharging, true)),
    ("usb-off", Request::Switch(Switch::UsbCharging, false)),
    ("light-bar-on", Request::Switch(Switch::LightBar, true)),
    ("light-bar-off", Request::Switch(Switch::LightBar, false)),
];
impl Request {
    fn verb(self) -> &'static str {
        VERBS
            .iter()
            .find(|(_, r)| *r == self)
            .map_or("read", |(v, _)| v)
    }
    fn parse(verb: &str) -> Option<Self> {
        VERBS.iter().find(|(v, _)| *v == verb).map(|(_, r)| *r)
    }
}
pub enum Response {
    Observation(Observation),
    Status(HardwareStatus),
    Done,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Reply {
    request: u64,
    ok: bool,
    observation: Option<Observation>,
    status: Option<HardwareStatus>,
    error: Option<String>,
}
/// Longest valid line: "light-bar-off " + 20 digits + newline.
const LINE_LIMIT: usize = 48;

/// Accept only a bounded `<verb> <sequence>` line. Malformed input never calls the adapter.
/// The agent exits after any failed request so the next one starts from a fresh connection.
pub(crate) fn serve(
    mut input: impl BufRead,
    mut output: impl Write,
    mut handle: impl FnMut(Request) -> Result<Response, String>,
) -> Result<(), String> {
    loop {
        let mut line = Vec::new();
        let count = input
            .by_ref()
            .take(LINE_LIMIT as u64 + 1)
            .read_until(b'\n', &mut line)
            .map_err(|e| e.to_string())?;
        if count == 0 {
            return Ok(());
        }
        let parsed = (|| {
            if count > LINE_LIMIT || line.last() != Some(&b'\n') {
                return None;
            }
            let mut fields = std::str::from_utf8(&line).ok()?.split_whitespace();
            let request = Request::parse(fields.next()?)?;
            let id = fields.next()?.parse::<u64>().ok()?;
            (id != 0 && fields.next().is_none()).then_some((request, id))
        })();
        let (id, result) = match parsed {
            Some((request, id)) => (id, handle(request)),
            None => (0, Err("Invalid hardware agent request".into())),
        };
        let reply = match result {
            Ok(response) => {
                let (observation, status) = match response {
                    Response::Observation(o) => (Some(o), None),
                    Response::Status(s) => (None, Some(s)),
                    Response::Done => (None, None),
                };
                Reply {
                    request: id,
                    ok: true,
                    observation,
                    status,
                    error: None,
                }
            }
            Err(error) => Reply {
                request: id,
                ok: false,
                observation: None,
                status: None,
                error: Some(error),
            },
        };
        serde_json::to_writer(&mut output, &reply).map_err(|e| e.to_string())?;
        output
            .write_all(b"\n")
            .and_then(|_| output.flush())
            .map_err(|e| e.to_string())?;
        if !reply.ok {
            return Ok(());
        }
    }
}

pub struct Agent {
    child: Child,
    input: Option<ChildStdin>,
    job: Option<ChildJob>,
    replies: Receiver<Result<Vec<u8>, RunError>>,
    sequence: u64,
}
impl Agent {
    pub fn start(helper: &std::path::Path) -> Result<Self, RunError> {
        use std::os::windows::process::CommandExt;
        let mut command = Command::new(helper);
        command
            .args(["hardware", "agent"])
            .creation_flags(0x00000008);
        Self::spawn(&mut command)
    }
    fn spawn(command: &mut Command) -> Result<Self, RunError> {
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|_| RunError::Spawn)?;
        let job = match ChildJob::attach(&child) {
            Ok(job) => job,
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e);
            }
        };
        let input = child.stdin.take();
        let stdout = child.stdout.take().ok_or(RunError::Io)?;
        let stderr = child.stderr.take().ok_or(RunError::Io)?;
        let (tx, replies) = mpsc::sync_channel(2);
        let errors = tx.clone();
        thread::spawn(move || {
            let mut stdout = BufReader::new(stdout);
            loop {
                let mut line = Vec::new();
                let result = match stdout
                    .by_ref()
                    .take((OUTPUT_LIMIT + 1) as u64)
                    .read_until(b'\n', &mut line)
                {
                    Err(_) => Err(RunError::Io),
                    Ok(n) if n > OUTPUT_LIMIT => Err(RunError::OutputLimit),
                    Ok(_) if line.last() != Some(&b'\n') => Err(RunError::Io),
                    Ok(_) => Ok(line),
                };
                let failed = result.is_err();
                if tx.send(result).is_err() || failed {
                    break;
                }
            }
        });
        thread::spawn(move || {
            let mut data = Vec::new();
            let error = match stderr
                .take((OUTPUT_LIMIT + 1) as u64)
                .read_to_end(&mut data)
            {
                Err(_) => Some(RunError::Io),
                Ok(n) if n > OUTPUT_LIMIT => Some(RunError::OutputLimit),
                _ => None,
            };
            if let Some(e) = error {
                let _ = errors.send(Err(e));
            }
        });
        Ok(Self {
            child,
            input,
            job: Some(job),
            replies,
            sequence: 0,
        })
    }
    /// Send one request and wait for its reply. Any error means the caller must drop this agent:
    /// its state is unknown, and the request is never resent.
    pub fn request(&mut self, request: Request, timeout: Duration) -> Result<Response, String> {
        let start = Instant::now();
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or("Hardware agent sequence exhausted")?;
        writeln!(
            self.input.as_mut().ok_or("Hardware agent closed")?,
            "{} {}",
            request.verb(),
            self.sequence
        )
        .map_err(|e| e.to_string())?;
        let remaining = timeout
            .checked_sub(start.elapsed())
            .ok_or("Hardware request timed out")?;
        let bytes = self
            .replies
            .recv_timeout(remaining)
            .map_err(|e| format!("Hardware request stopped: {e}"))?
            .map_err(|e| format!("Hardware request stopped: {e:?}"))?;
        let reply: Reply = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if reply.request != self.sequence {
            return Err("Unexpected hardware reply sequence".into());
        }
        match (reply.ok, reply.observation, reply.status, reply.error) {
            (true, Some(o), None, None) if request == Request::Observe => {
                Ok(Response::Observation(o))
            }
            (true, None, Some(s), None) if request == Request::Status => Ok(Response::Status(s)),
            (true, None, None, None)
                if matches!(request, Request::Fan(_) | Request::Switch(_, _)) =>
            {
                Ok(Response::Done)
            }
            (false, None, None, Some(error)) => Err(error),
            _ => Err("Invalid hardware agent reply".into()),
        }
    }
}
impl Drop for Agent {
    fn drop(&mut self) {
        use std::os::windows::io::AsRawHandle;
        // EOF lets an idle reader release COM normally. A blocked/misbehaving one is contained.
        self.input.take();
        // SAFETY: Wait briefly on our own live child; this worker owns no windows/COM apartment.
        unsafe {
            windows_sys::Win32::System::Threading::WaitForSingleObject(
                self.child.as_raw_handle(),
                100,
            );
        }
        self.job.take();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn observation(mode: u8) -> Observation {
        Observation {
            mode,
            writes_allowed: true,
            ownership_reason: None,
        }
    }
    #[test]
    fn protocol_rejects_unknown_verbs_and_overlong_lines_before_adapter_access() {
        for input in [
            b"fan-max 1\nread 2\n".as_slice(),
            b"write 1\n",
            &[b'x'; 60],
            b"read 1 extra\n",
            b"read 0\n",
            b"fan-boost\n",
            b"light-bar-off 123456789012345678901234567890123\n",
        ] {
            let mut calls = 0;
            let mut output = Vec::new();
            serve(input, &mut output, |_| {
                calls += 1;
                Ok(Response::Observation(observation(0)))
            })
            .unwrap();
            assert_eq!(calls, 0);
            let reply: Reply = serde_json::from_slice(output.trim_ascii()).unwrap();
            assert!(!reply.ok);
        }
    }
    #[test]
    fn every_allowlisted_verb_round_trips_to_one_typed_request() {
        for (verb, request) in VERBS {
            assert_eq!(Request::parse(verb), Some(request));
            assert_eq!(request.verb(), verb);
            let mut seen = None;
            let mut output = Vec::new();
            serve(
                format!("{verb} 18446744073709551615\n").as_bytes(),
                &mut output,
                |r| {
                    seen = Some(r);
                    Ok(Response::Done)
                },
            )
            .unwrap();
            assert_eq!(seen, Some(request));
        }
    }
    #[test]
    fn multiple_requests_keep_sequence_and_stop_after_adapter_failure() {
        let mut calls = 0;
        let mut output = Vec::new();
        serve(
            b"read 1\nfan-boost 2\nread 3\nread 4\n".as_slice(),
            &mut output,
            |r| {
                calls += 1;
                match (calls, r) {
                    (1, Request::Observe) => Ok(Response::Observation(observation(64))),
                    (2, Request::Fan(FanMode::Boost)) => Ok(Response::Done),
                    _ => Err("disconnected".into()),
                }
            },
        )
        .unwrap();
        let lines: Vec<Reply> = output
            .split(|c| *c == b'\n')
            .filter(|s| !s.is_empty())
            .map(|s| serde_json::from_slice(s).unwrap())
            .collect();
        assert_eq!(calls, 3);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].request, 1);
        assert_eq!(lines[0].observation.as_ref().unwrap().mode, 64);
        assert!(lines[1].ok && lines[1].observation.is_none() && lines[1].status.is_none());
        assert_eq!(lines[2].request, 3);
        assert!(!lines[2].ok);
    }
    fn fixture(script: &str) -> Agent {
        // EncodedCommand preserves literal JSON quotes through PowerShell's command-line parser.
        let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
        let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut encoded = String::new();
        for part in bytes.chunks(3) {
            let word = ((part[0] as u32) << 16)
                | ((part.get(1).copied().unwrap_or(0) as u32) << 8)
                | part.get(2).copied().unwrap_or(0) as u32;
            encoded.push(alphabet[(word >> 18) as usize] as char);
            encoded.push(alphabet[((word >> 12) & 63) as usize] as char);
            encoded.push(if part.len() > 1 {
                alphabet[((word >> 6) & 63) as usize] as char
            } else {
                '='
            });
            encoded.push(if part.len() > 2 {
                alphabet[(word & 63) as usize] as char
            } else {
                '='
            });
        }
        let mut command = Command::new(crate::platform::powershell_path().unwrap());
        use std::os::windows::process::CommandExt;
        // PowerShell requires a console association; production uses detached native Rust IO.
        command.creation_flags(0x08000000);
        command.args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-EncodedCommand",
            &encoded,
        ]);
        Agent::spawn(&mut command).unwrap()
    }
    #[test]
    fn pipe_reuses_one_child_and_drop_reaps_it() {
        let mut reader = fixture(
            r#"while ($line=[Console]::ReadLine()) {
            $id=[uint64]($line.Split(' ')[1]);
            [Console]::WriteLine('{"request":'+$id+',"ok":true,"observation":{"mode":64,"writes_allowed":true,"ownership_reason":null},"status":null,"error":null}')
        }"#,
        );
        let id = reader.child.id();
        for _ in 0..3 {
            assert!(matches!(
                reader.request(Request::Observe, Duration::from_secs(10)),
                Ok(Response::Observation(o)) if o.mode == 64
            ));
            assert_eq!(reader.child.id(), id);
        }
        assert_eq!(reader.sequence, 3);
        use std::os::windows::io::AsRawHandle;
        // SAFETY: Open a synchronization-only handle to our own live child for cleanup verification.
        let process = unsafe {
            windows_sys::Win32::System::Threading::OpenProcess(
                windows_sys::Win32::System::Threading::PROCESS_SYNCHRONIZE,
                0,
                id,
            )
        };
        assert!(!process.is_null());
        assert!(!reader.child.as_raw_handle().is_null());
        drop(reader);
        // SAFETY: Independently owned process handle remains valid after dropping the agent.
        assert_eq!(
            // SAFETY: Wait only on our independently owned process handle.
            unsafe { windows_sys::Win32::System::Threading::WaitForSingleObject(process, 1000) },
            0
        );
        // SAFETY: Close this independently acquired handle exactly once.
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(process);
        }
    }
    #[test]
    fn pipe_bounds_output_rejects_stale_sequences_and_times_out() {
        let mut stale = fixture(
            r#"[Console]::ReadLine() | Out-Null; [Console]::WriteLine('{"request":0,"ok":true,"observation":{"mode":0,"writes_allowed":true,"ownership_reason":null},"status":null,"error":null}'); Start-Sleep 30"#,
        );
        assert!(
            stale
                .request(Request::Observe, Duration::from_secs(10))
                .err()
                .unwrap()
                .contains("sequence")
        );
        drop(stale);
        let mut overflow = fixture(
            "[Console]::ReadLine() | Out-Null; [Console]::WriteLine(('x'*70000)); Start-Sleep 30",
        );
        assert!(
            overflow
                .request(Request::Observe, Duration::from_secs(10))
                .err()
                .unwrap()
                .contains("OutputLimit")
        );
        drop(overflow);
        let mut hanging = fixture("Start-Sleep 30");
        let start = Instant::now();
        assert!(
            hanging
                .request(Request::Status, Duration::from_millis(200))
                .is_err()
        );
        drop(hanging);
        assert!(start.elapsed() < Duration::from_secs(3));
    }
}
