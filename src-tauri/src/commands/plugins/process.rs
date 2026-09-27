use crate::error::{AppError, AppResult};
use std::io::{self, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const MAX_OUTPUT_BYTES: usize = 200 * 1024;
const TRUNCATED_SUFFIX: &str = "\n...[output truncated]";
const CLEANUP_GRACE: Duration = Duration::from_secs(2);

struct CapturedOutput {
    bytes: Vec<u8>,
    truncated: bool,
}

#[cfg(unix)]
type RawPipe = std::os::unix::io::RawFd;
#[cfg(windows)]
type RawPipe = std::os::windows::io::RawHandle;

struct PipeReader {
    raw_pipe: RawPipe,
    handle: JoinHandle<io::Result<CapturedOutput>>,
}

#[cfg(unix)]
const NONBLOCKING_FLAG: i32 = if cfg!(target_os = "linux") {
    0x800
} else {
    0x0004
};

#[cfg(unix)]
extern "C" {
    fn fcntl(fd: i32, command: i32, ...) -> i32;
    fn kill(pid: i32, signal: i32) -> i32;
}

#[cfg(unix)]
fn set_nonblocking(fd: RawPipe) -> io::Result<()> {
    const F_GETFL: i32 = 3;
    const F_SETFL: i32 = 4;
    let flags = unsafe { fcntl(fd, F_GETFL) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }
    if unsafe { fcntl(fd, F_SETFL, flags | NONBLOCKING_FLAG) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(unix)]
fn terminate_process_tree(pid: u32) {
    const SIGKILL: i32 = 9;
    if let Ok(pid) = i32::try_from(pid) {
        unsafe {
            kill(-pid, SIGKILL);
        }
    }
}

#[cfg(windows)]
fn terminate_process_tree(pid: u32) {
    let mut taskkill = match Command::new("taskkill.exe")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            tracing::warn!(
                "Could not start taskkill for plugin process {}: {}",
                pid,
                error
            );
            return;
        }
    };
    let deadline = Instant::now() + CLEANUP_GRACE;
    loop {
        match taskkill.try_wait() {
            Ok(Some(_)) => return,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = taskkill.kill();
                let _ = taskkill.wait();
                return;
            }
        }
    }
}

#[cfg(not(any(unix, windows)))]
fn terminate_process_tree(_pid: u32) {}

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn CancelIoEx(file: std::os::windows::io::RawHandle, overlapped: *mut std::ffi::c_void) -> i32;
    fn CancelSynchronousIo(thread: std::os::windows::io::RawHandle) -> i32;
}

#[cfg(windows)]
fn cancel_pipe_read(pipe: RawPipe, reader: &JoinHandle<io::Result<CapturedOutput>>) {
    unsafe {
        CancelIoEx(pipe, std::ptr::null_mut());
        CancelSynchronousIo(std::os::windows::io::AsRawHandle::as_raw_handle(reader));
    }
}

#[cfg(unix)]
fn cancel_pipe_read(_pipe: RawPipe, _reader: &JoinHandle<io::Result<CapturedOutput>>) {}

#[cfg(not(any(unix, windows)))]
fn cancel_pipe_read(_pipe: RawPipe, _reader: &JoinHandle<io::Result<CapturedOutput>>) {}

fn drain_pipe<R: Read>(mut reader: R, deadline: Instant) -> io::Result<CapturedOutput> {
    let mut bytes = Vec::with_capacity(MAX_OUTPUT_BYTES);
    let mut truncated = false;
    let mut chunk = [0u8; 8192];
    loop {
        match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(count) => {
                let remaining = MAX_OUTPUT_BYTES.saturating_sub(bytes.len());
                let kept = count.min(remaining);
                bytes.extend_from_slice(&chunk[..kept]);
                truncated |= kept < count;
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    truncated = true;
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
            #[cfg(windows)]
            Err(error) if error.raw_os_error() == Some(995) => {
                truncated = true;
                break;
            }
            Err(error) => return Err(error),
        }
    }
    Ok(CapturedOutput { bytes, truncated })
}

#[cfg(unix)]
fn spawn_pipe_reader<R: Read + Send + 'static>(
    pipe: R,
    raw_pipe: RawPipe,
    deadline: Instant,
) -> PipeReader {
    PipeReader {
        raw_pipe,
        handle: thread::spawn(move || drain_pipe(pipe, deadline)),
    }
}

#[cfg(windows)]
fn spawn_pipe_reader<R: Read + Send + 'static>(
    pipe: R,
    raw_pipe: RawPipe,
    _deadline: Instant,
) -> PipeReader {
    PipeReader {
        raw_pipe,
        handle: thread::spawn(move || {
            drain_pipe(
                pipe,
                Instant::now() + Duration::from_secs(365 * 24 * 60 * 60),
            )
        }),
    }
}

#[cfg(not(any(unix, windows)))]
fn spawn_pipe_reader<R: Read + Send + 'static>(
    pipe: R,
    raw_pipe: RawPipe,
    deadline: Instant,
) -> PipeReader {
    PipeReader {
        raw_pipe,
        handle: thread::spawn(move || drain_pipe(pipe, deadline)),
    }
}

fn finish_pipe_reader(reader: PipeReader, deadline: Instant) -> io::Result<CapturedOutput> {
    while !reader.handle.is_finished() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    if !reader.handle.is_finished() {
        cancel_pipe_read(reader.raw_pipe, &reader.handle);
    }
    let cancel_deadline = Instant::now() + Duration::from_millis(500);
    while !reader.handle.is_finished() && Instant::now() < cancel_deadline {
        thread::sleep(Duration::from_millis(10));
    }
    if !reader.handle.is_finished() {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "Plugin output pipe did not close during cleanup",
        ));
    }
    reader
        .handle
        .join()
        .map_err(|_| io::Error::other("Plugin output reader thread failed"))?
}

fn format_output(output: CapturedOutput) -> String {
    let mut text = String::from_utf8_lossy(&output.bytes).into_owned();
    let truncated = output.truncated || text.len() > MAX_OUTPUT_BYTES;
    let content_budget = if truncated {
        MAX_OUTPUT_BYTES - TRUNCATED_SUFFIX.len()
    } else {
        MAX_OUTPUT_BYTES
    };
    if text.len() > content_budget {
        let mut boundary = content_budget;
        while !text.is_char_boundary(boundary) {
            boundary -= 1;
        }
        text.truncate(boundary);
    }
    if truncated {
        text.push_str(TRUNCATED_SUFFIX);
    }
    text
}

fn stop_child(child: &mut std::process::Child, pid: u32) {
    terminate_process_tree(pid);
    let _ = child.kill();
}

pub(super) fn run_process(
    program: &str,
    args: &[String],
    cwd: &Path,
    timeout: Duration,
) -> AppResult<super::PluginRunResult> {
    let started = Instant::now();
    let deadline = started
        .checked_add(timeout)
        .unwrap_or(started + Duration::from_secs(315_360_000));
    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command.spawn().map_err(|error| {
        AppError::Config(format!(
            "Failed to start plugin command '{}': {}",
            program, error
        ))
    })?;
    let pid = child.id();
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::Config("Plugin stdout pipe was not available".into()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| AppError::Config("Plugin stderr pipe was not available".into()))?;

    #[cfg(unix)]
    if let Err(error) = set_nonblocking(std::os::unix::io::AsRawFd::as_raw_fd(&stdout))
        .and_then(|_| set_nonblocking(std::os::unix::io::AsRawFd::as_raw_fd(&stderr)))
    {
        stop_child(&mut child, pid);
        let _ = child.wait();
        return Err(AppError::Config(format!(
            "Cannot prepare plugin output pipes: {}",
            error
        )));
    }

    #[cfg(unix)]
    let stdout_raw = std::os::unix::io::AsRawFd::as_raw_fd(&stdout);
    #[cfg(unix)]
    let stderr_raw = std::os::unix::io::AsRawFd::as_raw_fd(&stderr);
    #[cfg(windows)]
    let stdout_raw = std::os::windows::io::AsRawHandle::as_raw_handle(&stdout);
    #[cfg(windows)]
    let stderr_raw = std::os::windows::io::AsRawHandle::as_raw_handle(&stderr);
    #[cfg(not(any(unix, windows)))]
    let (stdout_raw, stderr_raw) = (0, 0);

    let reader_deadline = deadline.checked_add(CLEANUP_GRACE).unwrap_or(deadline);
    let stdout_reader = spawn_pipe_reader(stdout, stdout_raw, reader_deadline);
    let stderr_reader = spawn_pipe_reader(stderr, stderr_raw, reader_deadline);

    let mut timed_out = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
            Ok(None) => {
                timed_out = true;
                stop_child(&mut child, pid);
                let _ = child.wait();
                break None;
            }
            Err(error) => {
                stop_child(&mut child, pid);
                let _ = child.wait();
                let _ = finish_pipe_reader(stdout_reader, Instant::now() + CLEANUP_GRACE);
                let _ = finish_pipe_reader(stderr_reader, Instant::now() + CLEANUP_GRACE);
                return Err(AppError::Config(format!("Plugin process error: {}", error)));
            }
        }
    };

    terminate_process_tree(pid);
    let reader_cleanup_deadline = Instant::now() + CLEANUP_GRACE;
    let stdout = finish_pipe_reader(stdout_reader, reader_cleanup_deadline);
    let stderr = finish_pipe_reader(stderr_reader, reader_cleanup_deadline);
    let stdout = stdout.map_err(|error| {
        AppError::Config(format!("Cannot finish plugin stdout capture: {}", error))
    })?;
    let stderr = stderr.map_err(|error| {
        AppError::Config(format!("Cannot finish plugin stderr capture: {}", error))
    })?;
    timed_out |= Instant::now() >= deadline;

    Ok(super::PluginRunResult {
        exit_code: if timed_out {
            None
        } else {
            status.and_then(|status| status.code())
        },
        stdout: format_output(stdout),
        stderr: format_output(stderr),
        timed_out,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_is_utf8_bounded_and_marks_truncation() {
        let mut bytes = "你".repeat(70_000).into_bytes();
        bytes.extend_from_slice(&[0xf0, 0x9f]);
        let rendered = format_output(CapturedOutput {
            bytes,
            truncated: true,
        });
        assert!(rendered.ends_with(TRUNCATED_SUFFIX));
        assert!(rendered.len() <= MAX_OUTPUT_BYTES);
        assert!(rendered.is_char_boundary(rendered.len()));

        let invalid_utf8 = format_output(CapturedOutput {
            bytes: vec![0xff, b'x'],
            truncated: false,
        });
        assert_eq!(invalid_utf8, "\u{fffd}x");
    }

    #[cfg(unix)]
    #[test]
    fn large_stdout_and_stderr_are_drained_and_bounded() {
        let script =
            "python3 -c \"import sys; sys.stdout.write('你'*70000); sys.stderr.write('e'*500000)\"";
        let result = run_process(
            "/bin/sh",
            &["-c".into(), script.into()],
            Path::new("/tmp"),
            Duration::from_secs(10),
        )
        .unwrap();
        assert_eq!(result.exit_code, Some(0));
        assert!(result.stdout.ends_with(TRUNCATED_SUFFIX));
        assert!(result.stderr.ends_with(TRUNCATED_SUFFIX));
        assert!(result.stdout.len() <= MAX_OUTPUT_BYTES);
        assert!(result.stderr.len() <= MAX_OUTPUT_BYTES);
    }

    #[cfg(unix)]
    #[test]
    fn timeout_and_exited_parent_close_descendant_pipes() {
        let started = Instant::now();
        let result = run_process(
            "/bin/sh",
            &["-c".into(), "(sleep 15) & exit 0".into()],
            Path::new("/tmp"),
            Duration::from_secs(5),
        )
        .unwrap();
        assert_eq!(result.exit_code, Some(0));
        assert!(started.elapsed() < Duration::from_secs(3));

        let result = run_process(
            "/bin/sleep",
            &["15".into()],
            Path::new("/tmp"),
            Duration::from_millis(100),
        )
        .unwrap();
        assert!(result.timed_out);
        assert_eq!(result.exit_code, None);
    }
}
