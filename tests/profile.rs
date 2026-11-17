use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_leanwake"))
        .args(args)
        .output()
        .unwrap()
}

fn assert_code(output: &Output, code: i32) {
    assert_eq!(
        output.status.code(),
        Some(code),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn copy_tree(source: &Path, target: &Path) {
    std::fs::create_dir_all(target).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        if entry.file_name() == ".lake" || entry.file_name() == "lake-manifest.json" {
            continue;
        }
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target.join(entry.file_name()));
        } else {
            std::fs::copy(entry.path(), target.join(entry.file_name())).unwrap();
        }
    }
}

fn fixture(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dir = root.join("target").join(format!(
        "test-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    copy_tree(&root.join("tests/fixtures").join(name), &dir);
    dir
}

fn record(root: &Path, code: i32) -> Value {
    let output = cli(&[
        "profile",
        "record-profile",
        "--project-dir",
        root.to_str().unwrap(),
        "--jobs",
        "4",
    ]);
    assert_code(&output, code);
    serde_json::from_slice(&output.stdout).unwrap()
}

fn commands(profile: &Value) -> Vec<&Value> {
    profile["Files"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|file| file["Commands"].as_array().unwrap())
        .collect()
}

fn pascal_case(value: &Value) {
    match value {
        Value::Object(fields) => {
            for (key, value) in fields {
                assert!(key.chars().next().unwrap().is_ascii_uppercase(), "{key}");
                assert!(!key.contains('_'), "{key}");
                pascal_case(value);
            }
        }
        Value::Array(values) => {
            for value in values {
                pascal_case(value);
            }
        }
        _ => (),
    }
}

fn lake_build(root: &Path) -> Output {
    Command::new("lake")
        .current_dir(root)
        .args(["--no-cache", "build"])
        .output()
        .unwrap()
}

#[test]
fn help_and_syntax_follow_the_command_tree() {
    for args in [
        vec!["help"],
        vec!["profile", "help"],
        vec!["profile", "record-profile", "help"],
        vec!["profile", "list-commands", "help"],
        vec!["profile", "list-files", "help"],
        vec!["--version"],
        vec!["profile", "--version"],
    ] {
        assert_code(&cli(&args), 0);
    }
    for args in [
        vec![],
        vec!["unknown"],
        vec!["profile"],
        vec!["profile", "record-profile"],
        vec![
            "profile",
            "record-profile",
            "--project-dir",
            ".",
            "--jobs",
            "0",
        ],
        vec![
            "profile",
            "record-profile",
            "--project-dir",
            ".",
            "--detail",
            "steps",
        ],
        vec!["profile", "list-files", "bare.json"],
        vec!["profile", "list-files", "--profile", "{}"],
        vec![
            "profile",
            "list-files",
            "--profile",
            "file://missing-profile.json",
        ],
    ] {
        assert_code(&cli(&args), 252);
    }
}

#[test]
fn records_are_deterministic_and_preserve_lake_configuration() {
    let root = fixture("good");
    assert_code(&lake_build(&root), 0);
    let first = record(&root, 0);
    let second = record(&root, 0);
    pascal_case(&first);
    assert_eq!(
        first["Conditions"],
        json!({"Async": false, "Detail": "none"})
    );
    assert_eq!(first["Files"].as_array().unwrap().len(), 3);
    let signature = |profile: &Value| {
        commands(profile)
            .iter()
            .map(|row| (row["Key"].clone(), row["Heartbeats"].clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(signature(&first), signature(&second));
    for file in first["Files"].as_array().unwrap() {
        assert_eq!(file["HasErrors"], false);
        let input = std::fs::read(root.join(file["Path"].as_str().unwrap())).unwrap();
        let hash: String = Sha256::digest(input)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(file["SourceSha256"], hash);
    }
    let rows = commands(&first);
    for name in [
        "Sample.answer",
        "Sample.answer_eq",
        "Sample.Box",
        "Sample.instInhabitedBox",
        "Sample.zEven",
        "Sample.café",
        "Sample.heavy",
        "Sample.nested",
    ] {
        assert!(
            rows.iter()
                .any(|row| row["Names"].as_array().unwrap().contains(&json!(name))),
            "missing {name}: {rows:?}"
        );
    }
    for name in ["Sample.heavy", "Sample.nested"] {
        let row = rows
            .iter()
            .find(|row| row["Names"].as_array().unwrap().contains(&json!(name)))
            .unwrap();
        assert!(
            row["Heartbeats"].as_u64().unwrap() > 100_000,
            "proof work missing: {row}"
        );
    }
    let mutual = rows
        .iter()
        .find(|row| row["Kind"] == "Lean.Parser.Command.mutual")
        .unwrap();
    assert_eq!(mutual["Names"], json!(["Sample.zEven", "Sample.aOdd"]));
    assert_eq!(mutual["Key"], "Sample.zEven");
    let structure = rows.iter().find(|row| row["Key"] == "Sample.Box").unwrap();
    assert_eq!(
        structure["Names"],
        json!(["Sample.Box", "Sample.Box.mk", "Sample.Box.value"])
    );
    assert!(
        rows.iter()
            .any(|row| row["Kind"] == "Lean.Parser.Command.exit")
    );
    assert!(!rows.iter().any(|row| row["Key"] == "skippedAfterExit"));
    let examples: Vec<_> = rows
        .iter()
        .filter(|row| {
            row["Names"] == json!([]) && row["Key"].as_str().unwrap().contains("::declaration:")
        })
        .collect();
    assert_eq!(examples.len(), 2, "{examples:?}");
    let keys: Vec<_> = examples
        .iter()
        .map(|row| row["Key"].as_str().unwrap())
        .collect();
    assert_eq!(keys[0].strip_suffix("#1"), keys[1].strip_suffix("#2"));
    assert!(keys[0].ends_with("#1"));
    let source = "example : answer = 42 := rfl";
    let hash: String = Sha256::digest(source)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    assert!(keys[0].contains(&hash[..12]));
    for row in rows {
        assert_eq!(row["HasErrors"], false);
        if !row["Names"].as_array().unwrap().is_empty() {
            assert_eq!(row["Key"], row["Names"][0]);
        }
        assert!(row["Range"]["Start"]["Line"].as_u64().unwrap() > 0);
    }
    let output_file = root.join("profile.json");
    let output = cli(&[
        "profile",
        "record-profile",
        "--project-dir",
        root.to_str().unwrap(),
        "--paths",
        "Sample/Basic.lean",
        "--output-file",
        output_file.to_str().unwrap(),
    ]);
    assert_code(&output, 0);
    assert!(output.stdout.is_empty());
    let uri = format!("file://{}", output_file.display());
    let output = cli(&[
        "profile",
        "list-commands",
        "--profile",
        &uri,
        "--limit",
        "2",
    ]);
    assert_code(&output, 0);
    let listed: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(listed["Commands"].as_array().unwrap().len(), 2);
    assert!(
        listed["Commands"][0]["Heartbeats"].as_u64()
            >= listed["Commands"][1]["Heartbeats"].as_u64()
    );
    for format in ["json", "text", "table"] {
        let output = cli(&[
            "profile",
            "list-files",
            "--profile",
            &uri,
            "--output",
            format,
        ]);
        assert_code(&output, 0);
        assert!(!output.stdout.is_empty());
    }
}

#[test]
fn errors_are_recorded_without_stopping_other_files() {
    let root = fixture("errors");
    assert!(!lake_build(&root).status.success());
    let profile = record(&root, 254);
    assert_eq!(profile["Files"].as_array().unwrap().len(), 5);
    assert_eq!(
        profile["Files"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|file| file["HasErrors"] == true)
            .count(),
        4
    );
    for path in ["Errors/Dependent.lean", "Errors/Missing.lean"] {
        let file = profile["Files"]
            .as_array()
            .unwrap()
            .iter()
            .find(|file| file["Path"] == path)
            .unwrap();
        assert_eq!(file["HasErrors"], true);
        assert_eq!(file["Commands"], json!([]));
    }
    assert!(
        commands(&profile)
            .iter()
            .any(|row| row["Key"] == "afterError" && row["HasErrors"] == false)
    );
    assert!(
        commands(&profile)
            .iter()
            .any(|row| row["Key"] == "valid" && row["HasErrors"] == false)
    );
    assert!(
        commands(&profile)
            .iter()
            .any(|row| row["HasErrors"] == true)
    );
}

#[test]
fn heartbeat_limits_match_lake_build() {
    let root = fixture("budget");
    assert!(!lake_build(&root).status.success());
    let profile = record(&root, 254);
    assert!(
        commands(&profile)
            .iter()
            .any(|row| row["HasErrors"] == true)
    );
    let config = root.join("lakefile.toml");
    let text = std::fs::read_to_string(&config)
        .unwrap()
        .replace("maxHeartbeats = 1", "maxHeartbeats = 200000");
    std::fs::write(config, text).unwrap();
    assert_code(&lake_build(&root), 0);
    assert_eq!(record(&root, 0)["Files"][0]["HasErrors"], false);
}

#[test]
fn changing_lake_options_changes_elaboration_success() {
    let root = fixture("good");
    let config = root.join("lakefile.toml");
    let original = std::fs::read_to_string(&config).unwrap();
    for (from, to) in [
        ("autoImplicit = false", "autoImplicit = true"),
        ("maxRecDepth=1234", "maxRecDepth=1235"),
        (
            "linter.unusedVariables=false",
            "linter.unusedVariables=true",
        ),
    ] {
        std::fs::write(&config, original.replace(from, to)).unwrap();
        assert!(!lake_build(&root).status.success(), "mutation {from}");
        let profile = record(&root, 254);
        assert!(
            profile["Files"]
                .as_array()
                .unwrap()
                .iter()
                .any(|file| file["HasErrors"] == true)
        );
    }
    std::fs::write(config, original).unwrap();
    assert_code(&lake_build(&root), 0);
    record(&root, 0);
}

#[test]
fn default_targets_include_executables_and_use_lake_source_paths_and_plugins() {
    let root = fixture("layout");
    assert_code(&lake_build(&root), 0);
    let profile = record(&root, 0);
    let paths: Vec<_> = profile["Files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|file| file["Path"].as_str().unwrap())
        .collect();
    assert!(paths.contains(&"src/App.lean"));
    assert!(paths.contains(&"src/First.lean"));
    assert!(paths.contains(&"src/First/More.lean"));
    assert!(!paths.contains(&"src/Unused.lean"));
    assert!(commands(&profile).iter().any(|row| row["Key"] == "first"));
    let setup = Command::new("lake")
        .current_dir(&root)
        .args(["--no-cache", "-J", "query", "+First:setup"])
        .output()
        .unwrap();
    assert_code(&setup, 0);
    let setup: Value = serde_json::from_slice(&setup.stdout).unwrap();
    assert!(!setup["plugins"].as_array().unwrap().is_empty());
    let selected = cli(&[
        "profile",
        "record-profile",
        "--project-dir",
        root.to_str().unwrap(),
        "--paths",
        "src/Unused.lean",
    ]);
    assert_code(&selected, 254);
    let selected: Value = serde_json::from_slice(&selected.stdout).unwrap();
    assert_eq!(selected["Files"][0]["Path"], "src/Unused.lean");
    assert_eq!(selected["Files"][0]["HasErrors"], true);
}

#[test]
fn explicit_paths_skip_hidden_files_and_symlinks_and_support_external_modules() {
    let root = fixture("good");
    let dir = root.join("scratch");
    std::fs::create_dir(&dir).unwrap();
    std::fs::write(dir.join("One.lean"), "def externalValue : Nat := 3\n").unwrap();
    std::fs::write(dir.join(".Hidden.lean"), "invalid syntax\n").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(dir.join("One.lean"), dir.join("Link.lean")).unwrap();
    let out = cli(&[
        "profile",
        "record-profile",
        "--project-dir",
        root.to_str().unwrap(),
        "--paths",
        "scratch",
    ]);
    assert_code(&out, 0);
    let profile: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(profile["Files"].as_array().unwrap().len(), 1);
    assert_eq!(profile["Files"][0]["Path"], "scratch/One.lean");
    assert_eq!(profile["Files"][0]["Commands"][0]["Key"], "externalValue");
    let out = cli(&[
        "profile",
        "record-profile",
        "--project-dir",
        root.to_str().unwrap(),
        "--paths",
        "scratch/.Hidden.lean",
        "scratch/One.lean",
    ]);
    assert_code(&out, 0);
    let profile: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(profile["Files"].as_array().unwrap().len(), 1);
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&dir, root.join("linked")).unwrap();
        let out = cli(&[
            "profile",
            "record-profile",
            "--project-dir",
            root.to_str().unwrap(),
            "--paths",
            "linked/One.lean",
        ]);
        assert_code(&out, 252);
    }
    let out = cli(&[
        "profile",
        "record-profile",
        "--project-dir",
        root.to_str().unwrap(),
        "--paths",
        "../..",
    ]);
    assert_code(&out, 252);
}

#[test]
fn stdin_profiles_and_file_parameters_work() {
    use std::io::Write;
    use std::process::Stdio;
    let input = json!({"SchemaVersion":1,"Tool":{"Name":"leanwake","Version":"0.1.0"},"Toolchain":"test","Conditions":{"Async":false,"Detail":"none"},"Files":[]});
    let mut child = Command::new(env!("CARGO_BIN_EXE_leanwake"))
        .args(["profile", "list-files", "--profile", "file:///dev/stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.to_string().as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert_code(&out, 0);
    assert_eq!(
        serde_json::from_slice::<Value>(&out.stdout).unwrap(),
        json!({"Files": []})
    );
    let out = cli(&[
        "profile",
        "list-commands",
        "--profile",
        &input.to_string(),
        "--output",
        "table",
    ]);
    assert_code(&out, 0);
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        "Heartbeats\tElapsedNs\tPath\tKey\tHasErrors\n"
    );
}

#[test]
fn module_realization_snapshots_preserve_success_and_heartbeats() {
    let root = fixture("modules");
    assert_code(&lake_build(&root), 0);
    let first = record(&root, 0);
    let second = record(&root, 0);
    let rows = commands(&first);
    let theorem = rows
        .iter()
        .find(|row| row["Key"] == "yearStart_decomp")
        .unwrap();
    assert_eq!(theorem["HasErrors"], false);
    assert!(theorem["Heartbeats"].as_u64().unwrap() > 100_000);
    let signature = |profile: &Value| {
        commands(profile)
            .iter()
            .map(|row| (row["Key"].clone(), row["Heartbeats"].clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(signature(&first), signature(&second));
}

#[cfg(target_os = "linux")]
#[test]
fn child_process_dies_when_host_is_killed() {
    use std::os::unix::fs::PermissionsExt;
    use std::process::Stdio;
    use std::time::{Duration, Instant};

    let root = fixture("good");
    let bin = root.join("bin");
    std::fs::create_dir(&bin).unwrap();
    let shell = Command::new("sh")
        .args(["-c", "command -v sh"])
        .output()
        .unwrap();
    assert_code(&shell, 0);
    let shell = String::from_utf8(shell.stdout).unwrap();
    let fake_lean = bin.join("lean");
    std::fs::write(
        &fake_lean,
        format!(
            "#!{}\necho $$ > \"$LEANWAKE_TEST_PID\"\nexec sleep 60\n",
            shell.trim()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&fake_lean, std::fs::Permissions::from_mode(0o755)).unwrap();
    let pid_file = root.join("child.pid");
    let mut paths = vec![bin];
    paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap()));
    let mut host = Command::new(env!("CARGO_BIN_EXE_leanwake"))
        .args(["profile", "record-profile", "--project-dir"])
        .arg(&root)
        .env("PATH", std::env::join_paths(paths).unwrap())
        .env("LEANWAKE_TEST_PID", &pid_file)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let pid = loop {
        if let Ok(text) = std::fs::read_to_string(&pid_file)
            && let Ok(pid) = text.trim().parse::<i32>()
        {
            break Some(pid);
        }
        if Instant::now() >= deadline {
            break None;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    host.kill().unwrap();
    host.wait().unwrap();
    let pid = pid.expect("host did not start the child");
    let running = || {
        std::fs::read_to_string(format!("/proc/{pid}/stat"))
            .ok()
            .and_then(|stat| {
                stat.rsplit_once(") ")
                    .map(|(_, tail)| !tail.starts_with('Z'))
            })
            .unwrap_or(false)
    };
    let deadline = Instant::now() + Duration::from_secs(2);
    while running() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    let leaked = running();
    if leaked {
        unsafe { libc::kill(pid, libc::SIGKILL) };
    }
    assert!(!leaked, "child {pid} survived the host's SIGKILL");
}
