//! Bounded subprocess runner for optional, one-shot metadata discovery.
use std::{
    io::Read,
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

pub const OUTPUT_LIMIT: usize = 64 * 1024;

/// Keep the helper and its immediate directory stable while an elevated UI can launch it.
/// No write/delete sharing for the helper, and no directory rename sharing.
#[cfg(windows)]
pub fn lock_helper(path: &std::path::Path) -> std::io::Result<(std::fs::File, std::fs::File)> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_READ, FILE_SHARE_WRITE,
    };
    let directory = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(path.parent().ok_or(std::io::ErrorKind::InvalidInput)?)?;
    let helper = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(path)?;
    Ok((directory, helper))
}

#[derive(Debug, PartialEq, Eq)]
pub enum RunError {
    Spawn,
    Timeout,
    OutputLimit,
    Io,
    Exit,
}

/// Closing this lease terminates descendants too, including startup's one-shot PowerShell.
#[cfg(windows)]
struct ChildJob(windows_sys::Win32::Foundation::HANDLE);
#[cfg(windows)]
impl Drop for ChildJob {
    fn drop(&mut self) {
        // SAFETY: Owns exactly one job handle, configured to kill only its assigned children.
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}
#[cfg(windows)]
impl ChildJob {
    fn attach(child: &std::process::Child) -> Result<Self, RunError> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::System::JobObjects::*;
        // SAFETY: Unnamed job with default security and a valid configuration structure.
        unsafe {
            let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if handle.is_null() {
                return Err(RunError::Spawn);
            }
            let job = Self(handle);
            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of_val(&limits) as u32,
            ) == 0
                || AssignProcessToJobObject(handle, child.as_raw_handle()) == 0
            {
                return Err(RunError::Spawn);
            }
            Ok(job)
        }
    }
}

fn reader(stream: impl Read + Send + 'static, tx: mpsc::Sender<Result<Vec<u8>, RunError>>) {
    thread::spawn(move || {
        let mut data = Vec::new();
        let result = match stream
            .take((OUTPUT_LIMIT + 1) as u64)
            .read_to_end(&mut data)
        {
            Err(_) => Err(RunError::Io),
            Ok(_) if data.len() > OUTPUT_LIMIT => Err(RunError::OutputLimit),
            Ok(_) => Ok(data),
        };
        let _ = tx.send(result);
    });
}

pub fn run(command: &mut Command, timeout: Duration) -> Result<Vec<u8>, RunError> {
    let start = Instant::now();
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| RunError::Spawn)?;
    #[cfg(windows)]
    let _job = match ChildJob::attach(&child) {
        Ok(job) => job,
        Err(e) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(e);
        }
    };
    let (out_tx, out_rx) = mpsc::channel();
    let (err_tx, err_rx) = mpsc::channel();
    reader(child.stdout.take().expect("piped stdout"), out_tx);
    reader(child.stderr.take().expect("piped stderr"), err_tx);
    let mut stdout = None;
    let mut stderr = None;
    let result = loop {
        if start.elapsed() >= timeout {
            break Err(RunError::Timeout);
        }
        if stdout.is_none() {
            match out_rx.try_recv() {
                Ok(Ok(v)) => stdout = Some(v),
                Ok(Err(e)) => break Err(e),
                Err(mpsc::TryRecvError::Disconnected) => break Err(RunError::Io),
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if stderr.is_none() {
            match err_rx.try_recv() {
                Ok(Ok(v)) => stderr = Some(v),
                Ok(Err(e)) => break Err(e),
                Err(mpsc::TryRecvError::Disconnected) => break Err(RunError::Io),
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        match child.try_wait() {
            Ok(Some(status)) if !status.success() => break Err(RunError::Exit),
            Ok(Some(_)) if stdout.is_some() && stderr.is_some() => {
                break Ok(stdout.take().unwrap());
            }
            Err(_) => break Err(RunError::Io),
            _ => {}
        }
        thread::sleep(Duration::from_millis(10));
    };
    if result.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn subprocess_fixture() {
        match std::env::var("MCC_TEST_CHILD").as_deref() {
            Ok("hang") => thread::sleep(Duration::from_secs(30)),
            Ok("overflow") => {
                use std::io::Write;
                let _ = std::io::stdout().write_all(&vec![b'x'; OUTPUT_LIMIT * 2]);
            }
            Ok("stderr_overflow") => {
                use std::io::Write;
                let _ = std::io::stderr().write_all(&vec![b'x'; OUTPUT_LIMIT * 2]);
            }
            Ok("exit") => std::process::exit(7),
            #[cfg(windows)]
            Ok("tree") => {
                let mut command = child("hang");
                command.stdout(Stdio::inherit()).stderr(Stdio::inherit());
                let mut descendant = command.spawn().unwrap();
                std::fs::write(
                    std::env::var_os("MCC_TEST_GRANDCHILD_PID").unwrap(),
                    descendant.id().to_string(),
                )
                .unwrap();
                let _ = descendant.wait();
            }
            Ok("ok") => println!("fixture works"),
            _ => {}
        }
    }
    fn child(mode: &str) -> Command {
        let mut c = Command::new(std::env::current_exe().unwrap());
        c.args([
            "--exact",
            "process::tests::subprocess_fixture",
            "--nocapture",
        ])
        .env("MCC_TEST_CHILD", mode);
        c
    }
    #[test]
    fn timeout_kills_hung_child() {
        let now = Instant::now();
        assert_eq!(
            run(&mut child("hang"), Duration::from_millis(150)),
            Err(RunError::Timeout)
        );
        assert!(now.elapsed() < Duration::from_secs(3));
    }
    #[cfg(windows)]
    #[test]
    fn timeout_also_terminates_descendants_that_hold_the_output_pipe() {
        use windows_sys::Win32::{Foundation::*, System::Threading::*};
        let file = std::env::temp_dir().join(format!("mcc-child-tree-{}.txt", std::process::id()));
        let _ = std::fs::remove_file(&file);
        let mut command = child("tree");
        command.env("MCC_TEST_GRANDCHILD_PID", &file);
        assert_eq!(
            run(&mut command, Duration::from_secs(1)),
            Err(RunError::Timeout)
        );
        let pid: u32 = std::fs::read_to_string(&file).unwrap().parse().unwrap();
        let _ = std::fs::remove_file(file);
        // SAFETY: Query only the disposable test descendant; close the acquired handle once.
        unsafe {
            let handle = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
            if handle.is_null() {
                assert_eq!(GetLastError(), ERROR_INVALID_PARAMETER);
            } else {
                assert_eq!(WaitForSingleObject(handle, 1000), WAIT_OBJECT_0);
                CloseHandle(handle);
            }
        }
    }
    #[test]
    fn stdout_is_bounded() {
        assert_eq!(
            run(&mut child("overflow"), Duration::from_secs(5)),
            Err(RunError::OutputLimit)
        );
    }
    #[test]
    fn stderr_is_bounded() {
        assert_eq!(
            run(&mut child("stderr_overflow"), Duration::from_secs(5)),
            Err(RunError::OutputLimit)
        );
    }
    #[test]
    fn nonzero_exit_is_not_success() {
        assert_eq!(
            run(&mut child("exit"), Duration::from_secs(5)),
            Err(RunError::Exit)
        );
    }
    #[test]
    fn captures_normal_output() {
        assert!(
            String::from_utf8(run(&mut child("ok"), Duration::from_secs(5)).unwrap())
                .unwrap()
                .contains("fixture works")
        );
    }
    #[cfg(windows)]
    #[test]
    fn helper_lease_blocks_replacement_while_allowing_readers() {
        let directory =
            std::env::temp_dir().join(format!("mcc-helper-lease-{}", std::process::id()));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("helper.txt");
        std::fs::write(&path, b"trusted").unwrap();
        let lease = lock_helper(&path).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"trusted");
        assert!(std::fs::write(&path, b"changed").is_err());
        assert!(std::fs::rename(&path, directory.join("moved.txt")).is_err());
        assert!(std::fs::rename(&directory, directory.with_extension("moved")).is_err());
        drop(lease);
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
}
