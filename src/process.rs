use crate::error::{Error, Result};
use std::io::{Read, Write};
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};

pub static DEBUG: AtomicBool = AtomicBool::new(false);

struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

pub fn run(command: &mut Command, input: Option<&[u8]>) -> Result<Output> {
    if DEBUG.load(Ordering::Relaxed) {
        eprintln!("leanwake: {command:?}");
    }
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            command.pre_exec(|| {
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::getppid() == 1 {
                    libc::raise(libc::SIGKILL);
                }
                Ok(())
            });
        }
    }
    let mut child = ChildGuard(
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?,
    );
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
            let _ = child.0.kill();
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
