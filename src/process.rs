//! Bounded subprocess runner for optional, one-shot metadata discovery.
use std::{
    io::Read,
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

pub const OUTPUT_LIMIT: usize = 64 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub enum RunError {
    Spawn,
    Timeout,
    OutputLimit,
    Io,
    Exit,
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
}
