use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Child, Command, ExitStatus};

struct Group(Child, bool);

impl Group {
    fn stop(&mut self) -> io::Result<ExitStatus> {
        if self.1 {
            return self.0.wait();
        }
        if unsafe { libc::kill(-(self.0.id() as i32), libc::SIGKILL) } != 0
            && io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
        {
            return Err(io::Error::last_os_error());
        }
        let status = self.0.wait()?;
        self.1 = true;
        Ok(status)
    }
}

impl Drop for Group {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

fn supervise() -> io::Result<ExitStatus> {
    let invalid = || {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected --control-fd FD -- PROGRAM [ARG ...]",
        )
    };
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new("--control-fd")) {
        return Err(invalid());
    }
    let fd = args
        .next()
        .and_then(|arg| arg.to_str().and_then(|s| s.parse::<i32>().ok()))
        .filter(|fd| *fd >= 3)
        .ok_or_else(invalid)?;
    if args.next().as_deref() != Some(std::ffi::OsStr::new("--")) {
        return Err(invalid());
    }
    let program = args.next().ok_or_else(invalid)?;
    let control = unsafe { OwnedFd::from_raw_fd(fd) };
    if unsafe { libc::fcntl(control.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) } == -1 {
        return Err(io::Error::last_os_error());
    }
    let mut group = Group(
        Command::new(program).args(args).process_group(0).spawn()?,
        false,
    );
    loop {
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        if unsafe {
            libc::waitid(
                libc::P_PID,
                group.0.id() as libc::id_t,
                &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        } != 0
        {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error);
        }
        #[cfg(target_os = "linux")]
        let completed = unsafe { info.si_pid() } != 0;
        #[cfg(target_os = "macos")]
        let completed = info.si_pid != 0;
        if completed {
            return group.stop();
        }
        let mut poll = libc::pollfd {
            fd: control.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let result = unsafe { libc::poll(&mut poll, 1, 50) };
        if result < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error);
        }
        if result > 0 {
            let mut byte = 0u8;
            let read = unsafe { libc::read(control.as_raw_fd(), (&mut byte as *mut u8).cast(), 1) };
            if read == 0 {
                return group.stop();
            }
            if read < 0 {
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(error);
            }
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unexpected supervisor control data",
            ));
        }
    }
}

fn main() -> std::process::ExitCode {
    match supervise() {
        Ok(status) => (status
            .code()
            .unwrap_or_else(|| 128 + status.signal().unwrap_or(0)) as u8)
            .into(),
        Err(error) => {
            eprintln!("leanwake-supervisor: {error}");
            255.into()
        }
    }
}
