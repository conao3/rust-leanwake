use crate::error::{Error, Result};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};

pub static DEBUG: AtomicBool = AtomicBool::new(false);

struct ChildGuard(Child, Option<UnixStream>);

impl ChildGuard {
    fn stop(&mut self) {
        self.1.take();
        let _ = self.0.wait();
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        self.stop();
    }
}

pub fn run(command: &mut Command, input: Option<&[u8]>) -> Result<Output> {
    if DEBUG.load(Ordering::Relaxed) {
        eprintln!("leanwake: {command:?}");
    }
    let helper = std::env::current_exe()?
        .parent()
        .ok_or_else(|| Error::internal("host has no executable directory"))?
        .join("leanwake-supervisor");
    let (control, lifetime) = UnixStream::pair()?;
    let fd = control.as_raw_fd();
    let mut supervised = Command::new(helper);
    supervised
        .arg("--control-fd")
        .arg(fd.to_string())
        .arg("--")
        .arg(command.get_program())
        .args(command.get_args());
    if let Some(directory) = command.get_current_dir() {
        supervised.current_dir(directory);
    }
    for (key, value) in command.get_envs() {
        if let Some(value) = value {
            supervised.env(key, value);
        } else {
            supervised.env_remove(key);
        }
    }
    unsafe {
        supervised.process_group(0).pre_exec(move || {
            if libc::fcntl(fd, libc::F_SETFD, 0) == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = ChildGuard(
        supervised
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?,
        Some(lifetime),
    );
    drop(control);
    let mut stdout = child
        .0
        .stdout
        .take()
        .ok_or_else(|| Error::internal("missing child stdout"))?;
    let mut stderr = child
        .0
        .stderr
        .take()
        .ok_or_else(|| Error::internal("missing child stderr"))?;
    std::thread::scope(|scope| {
        let out = scope.spawn(move || {
            let mut bytes = Vec::new();
            stdout.read_to_end(&mut bytes).map(|_| bytes)
        });
        let err = scope.spawn(move || {
            let mut bytes = Vec::new();
            stderr.read_to_end(&mut bytes).map(|_| bytes)
        });
        let status = (|| -> std::io::Result<_> {
            if let Some(mut stdin) = child.0.stdin.take()
                && let Some(input) = input
            {
                stdin.write_all(input)?;
            }
            child.0.wait()
        })();
        if status.is_err() {
            child.stop();
        }
        let stdout = out
            .join()
            .map_err(|_| Error::internal("stdout reader panicked"))??;
        let stderr = err
            .join()
            .map_err(|_| Error::internal("stderr reader panicked"))??;
        if DEBUG.load(Ordering::Relaxed) && !stderr.is_empty() {
            eprint!("{}", String::from_utf8_lossy(&stderr));
        }
        Ok(Output {
            status: status?,
            stdout,
            stderr,
        })
    })
}

pub fn checked(command: &mut Command) -> Result<Vec<u8>> {
    let output = run(command, None)?;
    if !output.status.success() {
        return Err(Error::target(format!(
            "{command:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(output.stdout)
}
