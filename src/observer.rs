//! Read-only observation IPC. The parent contains the helper in a kill-on-close job.
use crate::{
    hardware::Observation,
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

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Reply {
    request: u64,
    ok: bool,
    observation: Option<Observation>,
    error: Option<String>,
}

/// Accept only a bounded `read <sequence>` line. Malformed input never calls the adapter.
pub(crate) fn serve(
    mut input: impl BufRead,
    mut output: impl Write,
    mut observe: impl FnMut() -> Result<Observation, String>,
) -> Result<(), String> {
    loop {
        let mut line = Vec::new();
        let count = input
            .by_ref()
            .take(33)
            .read_until(b'\n', &mut line)
            .map_err(|e| e.to_string())?;
        if count == 0 {
            return Ok(());
        }
        let request = (|| {
            if count > 32 || line.last() != Some(&b'\n') {
                return None;
            }
            let mut fields = std::str::from_utf8(&line).ok()?.split_whitespace();
            if fields.next()? != "read" {
                return None;
            }
            let id = fields.next()?.parse::<u64>().ok()?;
            (id != 0 && fields.next().is_none()).then_some(id)
        })();
        let result = if request.is_some() {
            observe()
        } else {
            Err("Invalid read-only observation request".into())
        };
        let reply = match result {
            Ok(observation) => Reply {
                request: request.unwrap_or(0),
                ok: true,
                observation: Some(observation),
                error: None,
            },
            Err(error) => Reply {
                request: request.unwrap_or(0),
                ok: false,
                observation: None,
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

pub struct Observer {
    child: Child,
    input: Option<ChildStdin>,
    job: Option<ChildJob>,
    replies: Receiver<Result<Vec<u8>, RunError>>,
    sequence: u64,
}
impl Observer {
    pub fn start(helper: &std::path::Path) -> Result<Self, RunError> {
        use std::os::windows::process::CommandExt;
        let mut command = Command::new(helper);
        command
            .args(["hardware", "observe-loop"])
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
    pub fn read(&mut self, timeout: Duration) -> Result<Observation, String> {
        let start = Instant::now();
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or("Observation sequence exhausted")?;
        writeln!(
            self.input.as_mut().ok_or("Observation reader closed")?,
            "read {}",
            self.sequence
        )
        .map_err(|e| e.to_string())?;
        let remaining = timeout
            .checked_sub(start.elapsed())
            .ok_or("Observation reader timed out")?;
        let bytes = self
            .replies
            .recv_timeout(remaining)
            .map_err(|e| format!("Observation reader stopped: {e}"))?
            .map_err(|e| format!("Observation reader stopped: {e:?}"))?;
        let reply: Reply = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if reply.request != self.sequence {
            return Err("Unexpected observation reply sequence".into());
        }
        match (reply.ok, reply.observation, reply.error) {
            (true, Some(observation), None) => Ok(observation),
            (false, None, Some(error)) => Err(error),
            _ => Err("Invalid observation reply".into()),
        }
    }
}
impl Drop for Observer {
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
    fn protocol_rejects_writes_and_overlong_lines_before_adapter_access() {
        for input in [
            b"fan-boost\nread 1\n".as_slice(),
            &[b'x'; 40],
            b"read 1 extra\n",
            b"read 0\n",
        ] {
            let mut calls = 0;
            let mut output = Vec::new();
            serve(input, &mut output, || {
                calls += 1;
                Ok(observation(0))
            })
            .unwrap();
            assert_eq!(calls, 0);
            let reply: Reply = serde_json::from_slice(output.trim_ascii()).unwrap();
            assert!(!reply.ok);
        }
    }
    #[test]
    fn multiple_reads_keep_sequence_and_stop_after_adapter_failure() {
        let mut calls = 0;
        let mut output = Vec::new();
        serve(b"read 1\nread 2\nread 3\n".as_slice(), &mut output, || {
            calls += 1;
            if calls == 1 {
                Ok(observation(64))
            } else {
                Err("disconnected".into())
            }
        })
        .unwrap();
        let lines: Vec<Reply> = output
            .split(|c| *c == b'\n')
            .filter(|s| !s.is_empty())
            .map(|s| serde_json::from_slice(s).unwrap())
            .collect();
        assert_eq!(calls, 2);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].request, 1);
        assert_eq!(lines[0].observation.as_ref().unwrap().mode, 64);
        assert_eq!(lines[1].request, 2);
        assert!(!lines[1].ok);
    }
    fn fixture(script: &str) -> Observer {
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
        Observer::spawn(&mut command).unwrap()
    }
    #[test]
    fn pipe_reuses_one_child_and_drop_reaps_it() {
        let mut reader = fixture(
            r#"while ($line=[Console]::ReadLine()) {
            $id=[uint64]($line.Split(' ')[1]);
            [Console]::WriteLine('{"request":'+$id+',"ok":true,"observation":{"mode":64,"writes_allowed":true,"ownership_reason":null},"error":null}')
        }"#,
        );
        let id = reader.child.id();
        for _ in 0..3 {
            assert_eq!(reader.read(Duration::from_secs(10)).unwrap().mode, 64);
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
        // SAFETY: Independently owned process handle remains valid after dropping the observer.
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
            r#"[Console]::ReadLine() | Out-Null; [Console]::WriteLine('{"request":0,"ok":true,"observation":{"mode":0,"writes_allowed":true,"ownership_reason":null},"error":null}'); Start-Sleep 30"#,
        );
        assert!(
            stale
                .read(Duration::from_secs(10))
                .unwrap_err()
                .contains("sequence")
        );
        drop(stale);
        let mut overflow = fixture(
            "[Console]::ReadLine() | Out-Null; [Console]::WriteLine(('x'*70000)); Start-Sleep 30",
        );
        assert!(
            overflow
                .read(Duration::from_secs(10))
                .unwrap_err()
                .contains("OutputLimit")
        );
        drop(overflow);
        let mut hanging = fixture("Start-Sleep 30");
        let start = Instant::now();
        assert!(hanging.read(Duration::from_millis(200)).is_err());
        drop(hanging);
        assert!(start.elapsed() < Duration::from_secs(3));
    }
}
