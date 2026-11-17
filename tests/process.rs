#![cfg(target_os = "linux")]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Host(Child);

impl Drop for Host {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct Descendant(i32);

impl Drop for Descendant {
    fn drop(&mut self) {
        if running(self.0) {
            unsafe { libc::kill(self.0, libc::SIGKILL) };
        }
    }
}

fn running(pid: i32) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()
        .and_then(|stat| {
            stat.rsplit_once(") ")
                .map(|(_, tail)| !tail.starts_with('Z'))
        })
        .unwrap_or(false)
}

fn project(source: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join(format!(
            "process-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
    std::fs::create_dir_all(root.join("Probe")).unwrap();
    std::fs::write(root.join("lean-toolchain"), "leanprover/lean4:v4.35.0\n").unwrap();
    std::fs::write(
        root.join("lakefile.toml"),
        "name = \"probe\"\ndefaultTargets = [\"Probe\"]\n[[lean_lib]]\nname = \"Probe\"\n",
    )
    .unwrap();
    std::fs::write(
        root.join("Probe.lean"),
        "import Probe.Work\ndef value : Nat := 1\n",
    )
    .unwrap();
    std::fs::write(root.join("Probe/Work.lean"), source).unwrap();
    root
}

fn command(root: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_leanwake"));
    command
        .args(["profile", "record-profile", "--project-dir"])
        .arg(root)
        .args(["--paths", "Probe.lean"])
        .stdout(Stdio::null())
        .stderr(std::fs::File::create(root.join("host.stderr")).unwrap());
    command
}

fn ready(root: &Path, host: &mut Host, name: &str) -> Descendant {
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        if let Ok(text) = std::fs::read_to_string(root.join(name))
            && let Ok(pid) = text.trim().parse()
        {
            return Descendant(pid);
        }
        if host.0.try_wait().unwrap().is_some() || Instant::now() >= deadline {
            panic!(
                "process did not become ready: {}",
                std::fs::read_to_string(root.join("host.stderr")).unwrap()
            );
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn wait_host(host: &mut Host) -> std::process::ExitStatus {
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        if let Some(status) = host.0.try_wait().unwrap() {
            return status;
        }
        assert!(Instant::now() < deadline, "host did not exit");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn assert_stopped(descendant: &Descendant) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while running(descendant.0) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        !running(descendant.0),
        "descendant {} survived the host",
        descendant.0
    );
}

fn interrupt(signal: i32) {
    let root = project(
        "import Lean\nrun_cmd do\n  IO.FS.writeFile \"compiler.pid\" (toString (← IO.Process.getPID))\n  IO.sleep 3600000\n",
    );
    let mut host = Host(command(&root).spawn().unwrap());
    let compiler = ready(&root, &mut host, "compiler.pid");
    let stat = std::fs::read_to_string(format!("/proc/{}/stat", compiler.0)).unwrap();
    let parent = stat
        .rsplit_once(") ")
        .unwrap()
        .1
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    let lake = Descendant(parent);
    assert_ne!(lake.0, host.0.id() as i32);
    let executable = std::fs::read_link(format!("/proc/{}/exe", lake.0)).unwrap();
    assert!(
        executable
            .file_name()
            .unwrap()
            .to_string_lossy()
            .contains("lake"),
        "{}",
        executable.display()
    );
    assert_eq!(unsafe { libc::kill(host.0.id() as i32, signal) }, 0);
    wait_host(&mut host);
    assert_stopped(&compiler);
    assert_stopped(&lake);
}

#[test]
fn lake_compiler_descendants_die_after_sigkill() {
    interrupt(libc::SIGKILL);
}

#[test]
fn lake_compiler_descendants_die_after_sigterm() {
    interrupt(libc::SIGTERM);
}

#[test]
fn lake_compiler_descendants_die_after_sigint() {
    interrupt(libc::SIGINT);
}

#[test]
fn completed_and_failed_lake_builds_stop_background_descendants() {
    for failure in [false, true] {
        let source = format!(
            "import Lean\nrun_cmd do\n  let child ← IO.Process.spawn {{ cmd := \"sleep\", args := #[\"3600\"], stdin := .null, stdout := .null, stderr := .null }}\n  IO.FS.writeFile \"descendant.pid\" (toString child.pid)\n{}",
            if failure {
                "#check missingDeclaration\n"
            } else {
                "def dependency : Nat := 1\n"
            }
        );
        let root = project(&source);
        let mut host = Host(command(&root).spawn().unwrap());
        let descendant = ready(&root, &mut host, "descendant.pid");
        let status = wait_host(&mut host);
        assert_eq!(
            status.code(),
            Some(if failure { 254 } else { 0 }),
            "{}",
            std::fs::read_to_string(root.join("host.stderr")).unwrap()
        );
        assert_stopped(&descendant);
    }
}

#[test]
fn input_write_errors_stop_descendants_before_joining_readers() {
    use std::os::unix::fs::PermissionsExt;
    let root = project("def dependency : Nat := 1\n");
    std::fs::write(root.join("Probe.lean"), "def value : Nat := 1\n").unwrap();
    let config = root.join("lakefile.toml");
    std::fs::write(
        &config,
        format!(
            "{}\nmoreLeanArgs = [\"-Dweak.large={}\"]\n",
            std::fs::read_to_string(&config).unwrap(),
            "x".repeat(2_000_000)
        ),
    )
    .unwrap();
    let shell = Command::new("sh")
        .args(["-c", "command -v sh"])
        .output()
        .unwrap();
    assert!(shell.status.success());
    let adapter = root.join("adapter");
    std::fs::write(
        &adapter,
        format!(
            "#!{}\nsleep 3600 </dev/null &\necho $! > descendant.pid\nexit 0\n",
            String::from_utf8(shell.stdout).unwrap().trim()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&adapter, std::fs::Permissions::from_mode(0o755)).unwrap();
    let native = PathBuf::from(std::env::var_os("LEANWAKE_ADAPTER").unwrap())
        .canonicalize()
        .unwrap();
    std::os::unix::fs::symlink(
        native.parent().unwrap().join("leanwake-project"),
        root.join("leanwake-project"),
    )
    .unwrap();
    let mut host = Host(
        command(&root)
            .env("LEANWAKE_ADAPTER", &adapter)
            .spawn()
            .unwrap(),
    );
    let descendant = ready(&root, &mut host, "descendant.pid");
    assert_eq!(wait_host(&mut host).code(), Some(255));
    assert_stopped(&descendant);
}
