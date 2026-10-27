//! Console tools the panel runs: fixed arguments, no shell string built from data, no window, a deadline.

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// How a tool ended: its exit code and what it printed.
#[derive(Debug)]
pub struct Output {
    pub code: i32,
    pub stdout: String,
}

#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error("the program is not installed")]
    Missing,
    #[error("no answer before the deadline")]
    Timeout,
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

/// Runs a program with a deadline. Its output is read on a thread of its own: a tool that prints more than the pipe
/// holds (winget's progress) would otherwise block on a full pipe until the deadline.
pub fn output(program: &str, args: &[&str], timeout: Duration) -> Result<Output, RunError> {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: a console tool must not flash a window over the panel
        command.creation_flags(0x0800_0000);
    }
    let mut child = command.spawn().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            RunError::Missing
        } else {
            RunError::Io(error)
        }
    })?;
    let reader = child.stdout.take().map(|mut stdout| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            // A read error leaves what was read; the exit code still tells how the tool ended
            let _ = stdout.read_to_end(&mut bytes);
            bytes
        })
    });
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if started.elapsed() > timeout {
            if let Err(error) = child.kill() {
                tracing::warn!("could not stop {program} past its deadline: {error}");
            }
            return Err(RunError::Timeout);
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let bytes = reader
        .and_then(|reader| reader.join().ok())
        .unwrap_or_default();
    Ok(Output {
        code: status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&bytes).into_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_code_and_the_output() {
        let done = output(
            "cmd",
            &["/D", "/C", "echo hello& exit 3"],
            Duration::from_secs(10),
        )
        .expect("cmd runs");
        assert_eq!(done.code, 3);
        assert_eq!(done.stdout.trim(), "hello");
    }

    #[test]
    fn a_missing_program_is_told_apart() {
        assert!(matches!(
            output("no-such-tool-of-vitals", &[], Duration::from_secs(5)),
            Err(RunError::Missing)
        ));
    }

    #[test]
    fn more_output_than_a_pipe_holds_does_not_hang() {
        let done = output(
            "cmd",
            &["/D", "/C", "for /L %i in (1,1,20000) do @echo line %i"],
            Duration::from_secs(60),
        )
        .expect("cmd runs");
        assert_eq!(done.code, 0);
        assert_eq!(done.stdout.lines().count(), 20000);
    }
}
