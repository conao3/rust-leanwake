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
        vec!["profile", "list-hotspots", "help"],
        vec!["profile", "list-commands", "help"],
        vec!["profile", "list-files", "help"],
        vec!["profile", "list-steps", "help"],
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
            "unknown",
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
        assert!(row.get("Steps").is_none());
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
        listed["Commands"][0]["ElapsedNs"].as_u64() >= listed["Commands"][1]["ElapsedNs"].as_u64()
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
    let input = json!({"SchemaVersion":2,"Tool":{"Name":"leanwake","Version":"0.2.0"},"Toolchain":"test","Conditions":{"Async":false,"Detail":"none"},"Files":[]});
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
        "ElapsedNs\tHeartbeats\tPath\tKey\tHasErrors\n"
    );
}

#[test]
fn module_realization_snapshots_preserve_success_and_unchanged_comparisons() {
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
    let keys = |profile: &Value| {
        commands(profile)
            .iter()
            .map(|row| row["Key"].clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(keys(&first), keys(&second));
    let result = comparison(&first, &second, &[], 0);
    assert_eq!(result["Summary"]["Unchanged"], rows.len());
    assert_eq!(result["Summary"]["Regressed"], 0);
    assert_eq!(result["Summary"]["Improved"], 0);
    assert_eq!(result["Summary"]["Added"], 0);
    assert_eq!(result["Summary"]["Removed"], 0);
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
            "#!{}\necho $$ > \"$LEANWAKE_TEST_PID\"\nexec sleep 3600\n",
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
    let deadline = Instant::now() + Duration::from_secs(120);
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
    let deadline = Instant::now() + Duration::from_secs(30);
    while running() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    let leaked = running();
    if leaked {
        unsafe { libc::kill(pid, libc::SIGKILL) };
    }
    assert!(!leaked, "child {pid} survived the host's SIGKILL");
}

fn compare(base: &Value, head: &Value, options: &[&str]) -> Output {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join(format!(
            "compare-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("base.json"), base.to_string()).unwrap();
    std::fs::write(directory.join("head.json"), head.to_string()).unwrap();
    let base = format!("file://{}", directory.join("base.json").display());
    let head = format!("file://{}", directory.join("head.json").display());
    let mut args = vec![
        "profile",
        "compare-profiles",
        "--base-profile",
        &base,
        "--head-profile",
        &head,
    ];
    args.extend_from_slice(options);
    let output = cli(&args);
    std::fs::remove_dir_all(directory).unwrap();
    output
}

fn comparison(base: &Value, head: &Value, options: &[&str], code: i32) -> Value {
    let output = compare(base, head, options);
    assert_code(&output, code);
    let result = serde_json::from_slice(&output.stdout).unwrap();
    pascal_case(&result);
    result
}

fn synthetic_profile(entries: &[(&str, &str, u64)]) -> Value {
    json!({
        "SchemaVersion": 2,
        "Tool": {"Name": "leanwake", "Version": "0.2.0"},
        "Toolchain": "Lean (version 4.35.0, Release)",
        "Conditions": {"Async": false, "Detail": "none"},
        "Files": entries.iter().map(|(path, key, heartbeats)| json!({
            "Path": path, "Module": "Fixture", "SourceSha256": "unused",
            "HasErrors": false, "ImportElapsedNs": 0,
            "Commands": [{"Key": key, "Kind": "declaration", "Names": [key],
                "Range": {"Start": {"Line": 1, "Column": 0}, "End": {"Line": 2, "Column": 0}},
                "Heartbeats": heartbeats, "ElapsedNs": 0, "HasErrors": false}]
        })).collect::<Vec<_>>()
    })
}

#[test]
fn large_comparisons_match_by_file_and_key_without_losing_entries() {
    let mut base = synthetic_profile(&[]);
    base["Files"] = Value::Array(
        (0..200)
            .map(|file| {
                let path = format!("File{file:03}.lean");
                let mut profile = synthetic_profile(&[(&path, "command000", 100_000)]);
                let mut file = profile["Files"].as_array_mut().unwrap().remove(0);
                let template = file["Commands"][0].clone();
                file["Commands"] = Value::Array(
                    (0..500)
                        .map(|command| {
                            let mut entry = template.clone();
                            entry["Key"] = json!(format!("command{command:03}"));
                            entry
                        })
                        .collect(),
                );
                file
            })
            .collect(),
    );
    let mut head = base.clone();
    head["Files"].as_array_mut().unwrap().reverse();
    for file in head["Files"].as_array_mut().unwrap() {
        file["Commands"].as_array_mut().unwrap().reverse();
    }
    head["Files"][0]["Commands"][0]["Heartbeats"] = json!(120_000);
    head["Files"][1]["Commands"][0]["Heartbeats"] = json!(80_000);
    head["Files"][2]["Commands"][0]["Key"] = json!("renamed");
    let result = comparison(&base, &head, &[], 254);
    assert_eq!(
        result["Summary"],
        json!({
            "Regressed": 1, "Improved": 1, "Unchanged": 99_997,
            "Added": 1, "Removed": 1,
            "BaseHeartbeats": 10_000_000_000u64,
            "HeadHeartbeats": 10_000_000_000u64,
        })
    );
    let rows = result["Commands"].as_array().unwrap();
    assert_eq!(rows.len(), 4);
    assert_eq!(rows[0]["Path"], "File199.lean");
    assert_eq!(rows[0]["Key"], "command499");
    assert_eq!(rows[0]["Status"], "regressed");
    assert_eq!(rows[1]["Path"], "File198.lean");
    assert_eq!(rows[1]["Status"], "improved");
    assert_eq!(rows[2]["Path"], "File197.lean");
    assert_eq!(rows[2]["Key"], "renamed");
    assert_eq!(rows[3]["Path"], "File197.lean");
    assert_eq!(rows[3]["Key"], "command499");
}

#[test]
fn comparisons_classify_every_identity_and_sort_regressions() {
    let base = synthetic_profile(&[
        ("A.lean", "same", 100_000),
        ("B.lean", "same", 200_000),
        ("A.lean", "fast", 300_000),
        ("A.lean", "stable", 10),
        ("A.lean", "gone", 20),
    ]);
    let mut head = synthetic_profile(&[
        ("A.lean", "stable", 10),
        ("A.lean", "new", 30),
        ("A.lean", "fast", 100_000),
        ("B.lean", "same", 220_000),
        ("A.lean", "same", 105_000),
    ]);
    head["Files"][3]["Commands"][0]["Range"]["Start"]["Line"] = json!(25);
    let result = comparison(&base, &head, &[], 254);
    assert_eq!(
        result["Summary"],
        json!({
            "Regressed": 2, "Improved": 1, "Unchanged": 1, "Added": 1, "Removed": 1,
            "BaseHeartbeats": 600030, "HeadHeartbeats": 425040,
        })
    );
    assert_eq!(
        result["Thresholds"],
        json!({
            "AbsoluteThresholdHeartbeats": 1000, "RelativeThresholdPercent": 1,
        })
    );
    let rows = result["Commands"].as_array().unwrap();
    assert_eq!(rows.len(), 5);
    assert_eq!(rows[0]["Path"], "B.lean");
    assert_eq!(rows[0]["DeltaHeartbeats"], 20000);
    assert_eq!(rows[0]["Range"]["Start"]["Line"], 25);
    assert_eq!(rows[1]["Path"], "A.lean");
    assert_eq!(rows[1]["DeltaHeartbeats"], 5000);
    assert_eq!(rows[2]["Status"], "improved");
    assert_eq!(rows[2]["DeltaHeartbeats"], -200000);
    assert_eq!(rows[3]["Status"], "added");
    assert_eq!(rows[3]["BaseHeartbeats"], Value::Null);
    assert_eq!(rows[3]["DeltaHeartbeats"], 30);
    assert_eq!(rows[4]["Status"], "removed");
    assert_eq!(rows[4]["HeadHeartbeats"], Value::Null);
    assert_eq!(rows[4]["DeltaHeartbeats"], -20);
    let reverse = comparison(&head, &base, &[], 254);
    assert_eq!(reverse["Summary"]["Regressed"], 1);
    assert_eq!(reverse["Summary"]["Improved"], 2);
    for format in ["text", "table"] {
        let output = compare(&base, &head, &["--output", format]);
        assert_code(&output, 254);
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains("Regressed: 2"));
        assert!(
            text.contains("Status\tBaseHeartbeats\tHeadHeartbeats\tDeltaHeartbeats\tPath\tKey")
        );
        assert!(text.contains("regressed\t200000\t220000\t20000\tB.lean\tsame"));
    }
}

#[test]
fn threshold_boundaries_are_strict_and_use_exact_natural_arithmetic() {
    for (base, head, absolute, relative, expected) in [
        (100_000, 101_000, 1000, 1, "unchanged"),
        (100_000, 101_001, 1000, 1, "regressed"),
        (100_000, 101_001, 1001, 1, "unchanged"),
        (100_000, 101_001, 0, 2, "unchanged"),
        (101, 102, 0, 1, "unchanged"),
        (101, 103, 0, 1, "regressed"),
        (103, 101, 0, 1, "improved"),
        (0, 0, 0, 0, "unchanged"),
        (0, 1, 0, u64::MAX, "regressed"),
        (1, 0, 0, u64::MAX, "improved"),
        (0, u64::MAX, 0, 1, "regressed"),
        (u64::MAX, 0, 0, 1, "improved"),
        (u64::MAX / 2, u64::MAX, 0, u64::MAX, "unchanged"),
        (u64::MAX / 2, u64::MAX, 0, 99, "regressed"),
    ] {
        let b = synthetic_profile(&[("A.lean", "x", base)]);
        let h = synthetic_profile(&[("A.lean", "x", head)]);
        let result = comparison(
            &b,
            &h,
            &[
                "--absolute-threshold-heartbeats",
                &absolute.to_string(),
                "--relative-threshold-percent",
                &relative.to_string(),
            ],
            if expected == "regressed" { 254 } else { 0 },
        );
        if expected == "unchanged" {
            assert_eq!(result["Commands"], json!([]));
            assert_eq!(result["Summary"]["Unchanged"], 1);
        } else {
            assert_eq!(result["Commands"][0]["Status"], expected);
            assert_eq!(
                result["Commands"][0]["DeltaHeartbeats"].to_string(),
                (i128::from(head) - i128::from(base)).to_string()
            );
        }
    }
    let base = synthetic_profile(&[("A", "x", u64::MAX), ("B", "x", u64::MAX)]);
    let result = comparison(&base, &base, &[], 0);
    assert_eq!(
        result["Summary"]["BaseHeartbeats"].to_string(),
        "36893488147419103230"
    );
    assert_eq!(
        result["Summary"]["HeadHeartbeats"],
        result["Summary"]["BaseHeartbeats"]
    );
}

#[test]
fn invalid_comparison_inputs_are_rejected_before_results() {
    let valid = synthetic_profile(&[("A.lean", "x", 1)]);
    let result = comparison(&valid, &valid, &[], 0);
    assert_eq!(result["Summary"]["Unchanged"], 1);
    for (field, value) in [
        ("Toolchain", json!("different toolchain")),
        ("Conditions", json!({"Async": true, "Detail": "none"})),
        ("Conditions", json!({"Async": false, "Detail": "steps"})),
    ] {
        let mut invalid = valid.clone();
        invalid[field] = value.clone();
        for (base, head) in [(&valid, &invalid), (&invalid, &valid)] {
            let output = compare(base, head, &[]);
            assert_code(&output, 254);
            assert!(output.stdout.is_empty());
            let error = String::from_utf8(output.stderr).unwrap();
            assert!(error.contains(field), "{error}");
            assert!(error.contains(&format!("base={}", base[field])), "{error}");
            assert!(error.contains(&format!("head={}", head[field])), "{error}");
        }
    }
    let mut variants = Vec::new();
    let mut file_error = valid.clone();
    file_error["Files"][0]["HasErrors"] = json!(true);
    variants.push((file_error, "HasErrors"));
    let mut command_error = valid.clone();
    command_error["Files"][0]["Commands"][0]["HasErrors"] = json!(true);
    variants.push((command_error, "HasErrors"));
    let mut duplicate = valid.clone();
    let command = duplicate["Files"][0]["Commands"][0].clone();
    duplicate["Files"][0]["Commands"]
        .as_array_mut()
        .unwrap()
        .push(command);
    variants.push((duplicate, "duplicate"));
    let duplicate_files = synthetic_profile(&[("A.lean", "x", 1), ("A.lean", "x", 2)]);
    variants.push((duplicate_files, "duplicate"));
    for (invalid, expected) in variants {
        for (base, head, side) in [(&valid, &invalid, "head"), (&invalid, &valid, "base")] {
            let output = compare(base, head, &[]);
            assert_code(&output, 254);
            assert!(output.stdout.is_empty());
            let error = String::from_utf8(output.stderr).unwrap();
            assert!(
                error.contains(expected) && error.contains("A.lean") && error.contains(side),
                "{error}"
            );
            if expected == "duplicate" {
                assert!(error.contains("Key \"x\""), "{error}");
            }
        }
    }
    for parameter in [
        "--absolute-threshold-heartbeats",
        "--relative-threshold-percent",
    ] {
        for value in ["-1", "1.5", "not-a-number", "18446744073709551616"] {
            assert_code(&compare(&valid, &valid, &[parameter, value]), 252);
        }
    }
    let mut unsupported = valid.clone();
    unsupported["SchemaVersion"] = json!(1);
    for (base, head) in [(&valid, &unsupported), (&unsupported, &valid)] {
        let output = compare(base, head, &[]);
        assert_code(&output, 254);
        assert!(output.stdout.is_empty());
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(error.contains("SchemaVersion=1"), "{error}");
        assert!(error.contains("record the profile again"), "{error}");
    }
    assert_code(
        &cli(&["profile", "compare-profiles", "--base-profile", "{}"]),
        252,
    );
    assert_code(&cli(&["profile", "compare-profiles", "help"]), 0);
}

#[test]
fn empty_added_removed_and_renamed_commands_do_not_fail() {
    let empty = synthetic_profile(&[]);
    let base = synthetic_profile(&[("A.lean", "old", 2000)]);
    let head = synthetic_profile(&[("A.lean", "new", 2000)]);
    assert_eq!(
        comparison(&empty, &empty, &[], 0)["Summary"],
        json!({
            "Regressed": 0, "Improved": 0, "Unchanged": 0, "Added": 0, "Removed": 0,
            "BaseHeartbeats": 0, "HeadHeartbeats": 0,
        })
    );
    assert_eq!(comparison(&empty, &head, &[], 0)["Summary"]["Added"], 1);
    assert_eq!(comparison(&base, &empty, &[], 0)["Summary"]["Removed"], 1);
    let result = comparison(&base, &head, &[], 0);
    assert_eq!(result["Summary"]["Added"], 1);
    assert_eq!(result["Summary"]["Removed"], 1);
    let moved = synthetic_profile(&[("B.lean", "old", 2000)]);
    let result = comparison(&base, &moved, &[], 0);
    assert_eq!(result["Summary"]["Added"], 1);
    assert_eq!(result["Summary"]["Removed"], 1);
}

#[test]
fn recorded_changes_isolate_the_heavier_theorem_and_track_declarations() {
    let root = fixture("good");
    let base = record(&root, 0);
    let repeat = record(&root, 0);
    let result = comparison(&base, &repeat, &[], 0);
    assert_eq!(result["Summary"]["Regressed"], 0);
    assert_eq!(result["Summary"]["Improved"], 0);
    assert_eq!(result["Commands"], json!([]));
    let path = root.join("Sample/Options.lean");
    let original = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        original.replace("range 500).length = 500", "range 1500).length = 1500"),
    )
    .unwrap();
    let head = record(&root, 0);
    let result = comparison(&base, &head, &[], 254);
    assert_eq!(result["Summary"]["Regressed"], 1);
    assert_eq!(result["Summary"]["Improved"], 0);
    assert_eq!(result["Commands"].as_array().unwrap().len(), 1);
    assert_eq!(result["Commands"][0]["Key"], "Sample.heavy");
    assert_eq!(result["Commands"][0]["Path"], "Sample/Options.lean");
    assert_eq!(result["Commands"][0]["Status"], "regressed");
    let relaxed = comparison(
        &base,
        &head,
        &["--absolute-threshold-heartbeats", "18446744073709551615"],
        0,
    );
    assert_eq!(relaxed["Summary"]["Regressed"], 0);
    let relaxed = comparison(
        &base,
        &head,
        &["--relative-threshold-percent", "18446744073709551615"],
        0,
    );
    assert_eq!(relaxed["Summary"]["Regressed"], 0);
    std::fs::write(path, original).unwrap();
    let path = root.join("Sample/Basic.lean");
    let original = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        original
            .replace("theorem answer_eq", "theorem renamed_answer_eq")
            .replace("def café : Nat := hidden", "def fresh : Nat := hidden"),
    )
    .unwrap();
    let head = record(&root, 0);
    let result = comparison(&base, &head, &[], 0);
    assert_eq!(result["Summary"]["Added"], 2);
    assert_eq!(result["Summary"]["Removed"], 2);
    for (key, status) in [
        ("Sample.answer_eq", "removed"),
        ("Sample.renamed_answer_eq", "added"),
        ("Sample.café", "removed"),
        ("Sample.fresh", "added"),
    ] {
        assert!(
            result["Commands"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["Key"] == key && row["Status"] == status)
        );
    }
    let base_path = root.join("base.json");
    let head_path = root.join("head.json");
    std::fs::write(&base_path, base.to_string()).unwrap();
    std::fs::write(&head_path, head.to_string()).unwrap();
    let threshold_path = root.join("threshold.txt");
    std::fs::write(&threshold_path, "1000\n").unwrap();
    let output = cli(&[
        "profile",
        "compare-profiles",
        "--base-profile",
        &format!("file://{}", base_path.display()),
        "--head-profile",
        &format!("file://{}", head_path.display()),
        "--absolute-threshold-heartbeats",
        &format!("file://{}", threshold_path.display()),
    ]);
    assert_code(&output, 0);
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        result
    );
}

fn flatten_steps(nodes: &[Value]) -> Vec<&Value> {
    nodes
        .iter()
        .flat_map(|node| {
            std::iter::once(node).chain(flatten_steps(node["Children"].as_array().unwrap()))
        })
        .collect()
}

fn shape(nodes: &[Value]) -> Vec<Value> {
    nodes
        .iter()
        .map(|node| {
            json!([
                node["Class"],
                node["Kind"],
                node["Range"],
                shape(node["Children"].as_array().unwrap())
            ])
        })
        .collect()
}

fn record_steps(root: &Path, threshold: &str) -> Value {
    let output = cli(&[
        "profile",
        "record-profile",
        "--project-dir",
        root.to_str().unwrap(),
        "--detail",
        "steps",
        "--step-threshold-ns",
        threshold,
    ]);
    assert_code(&output, 0);
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn steps_preserve_tactic_locations_kinds_and_repeatable_heartbeats() {
    let root = fixture("steps");
    assert_code(&lake_build(&root), 0);
    let first = record_steps(&root, "0");
    let second = record_steps(&root, "0");
    pascal_case(&first);
    assert_eq!(
        first["Conditions"],
        json!({"Async": false, "Detail": "steps"})
    );
    let signature = |profile: &Value| {
        commands(profile)
            .iter()
            .map(|row| {
                (
                    row["Key"].clone(),
                    row["Heartbeats"].clone(),
                    shape(row["Steps"].as_array().unwrap()),
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(signature(&first), signature(&second));
    let rows = commands(&first);
    let repeated = rows.iter().find(|row| row["Key"] == "repeated").unwrap();
    let nodes = flatten_steps(repeated["Steps"].as_array().unwrap());
    let simp: Vec<_> = nodes
        .iter()
        .filter(|node| {
            node["Class"] == "Leanwake.tactic" && node["Kind"] == "Lean.Parser.Tactic.simp"
        })
        .collect();
    assert_eq!(simp.len(), 2);
    for (node, line) in simp.iter().zip([5, 6]) {
        assert_eq!(
            node["Range"],
            json!({
                "Start": {"Line": line, "Column": 4},
                "End": {"Line": line, "Column": 28},
            })
        );
        assert!(node["ElapsedNs"].as_u64().unwrap() > 0);
    }
    assert!(nodes.iter().any(|node| node["Range"].is_null()));
    assert!(
        nodes
            .iter()
            .any(|node| node["Class"] == "Meta.synthInstance")
    );
    for key in [
        "configured",
        "afterOption",
        "locallyDefined",
        "simplifyMany",
    ] {
        let row = rows.iter().find(|row| row["Key"] == key).unwrap();
        let nodes = flatten_steps(row["Steps"].as_array().unwrap());
        assert!(
            nodes.iter().any(|node| {
                node["Class"] == "Leanwake.tactic"
                    && !node["Range"].is_null()
                    && node["ElapsedNs"].as_u64().unwrap() > 0
            }),
            "missing tactic trace for {key}"
        );
        if key == "locallyDefined" {
            assert!(nodes.iter().any(|node| {
                node["Kind"]
                    .as_str()
                    .is_some_and(|kind| kind.contains("LocalClose"))
            }));
        }
    }
    let conditional = rows
        .iter()
        .find(|row| row["Key"] == "simplifyConditional")
        .unwrap();
    assert!(
        flatten_steps(conditional["Steps"].as_array().unwrap())
            .iter()
            .any(|node| {
                node["Class"] == "Meta.Tactic.simp.discharge"
                    && node["ElapsedNs"].as_u64().unwrap() > 0
            })
    );
    let comparison = comparison(&first, &second, &[], 0);
    assert_eq!(comparison["Summary"]["Regressed"], 0);
    assert_eq!(comparison["Summary"]["Improved"], 0);
    let none = record(&root, 0);
    assert!(commands(&none).iter().all(|row| row.get("Steps").is_none()));
    assert_code(&compare(&none, &first, &[]), 254);
    assert_code(&compare(&first, &none, &[]), 254);
    let filtered = record_steps(&root, "100000");
    for (full, filtered) in commands(&first).iter().zip(commands(&filtered)) {
        assert_eq!(full["Key"], filtered["Key"]);
        assert_eq!(full["Heartbeats"], filtered["Heartbeats"]);
        for node in flatten_steps(filtered["Steps"].as_array().unwrap()) {
            assert!(
                node["Class"] == "Leanwake.tactic"
                    || node["ElapsedNs"].as_u64().unwrap() >= 100_000
            );
        }
    }
    let empty = record_steps(&root, "18446744073709551615");
    for row in commands(&empty) {
        assert!(
            flatten_steps(row["Steps"].as_array().unwrap())
                .iter()
                .all(|node| node["Class"] == "Leanwake.tactic")
        );
    }
}

#[test]
fn step_listings_select_one_command_sort_limit_and_validate_input() {
    let root = fixture("steps");
    let profile = record_steps(&root, "0");
    let path = root.join("profile.json");
    std::fs::write(&path, profile.to_string()).unwrap();
    let uri = format!("file://{}", path.display());
    let list = |profile: &str, path: &str, key: &str, format: &str| {
        cli(&[
            "profile",
            "list-steps",
            "--profile",
            profile,
            "--path",
            path,
            "--key",
            key,
            "--limit",
            "3",
            "--output",
            format,
        ])
    };
    let result = list(&uri, "Steps.lean", "repeated", "json");
    assert_code(&result, 0);
    let value: Value = serde_json::from_slice(&result.stdout).unwrap();
    pascal_case(&value);
    assert_eq!(value["Key"], "repeated");
    let steps = value["Steps"].as_array().unwrap();
    assert_eq!(steps.len(), 3);
    assert!(
        steps
            .windows(2)
            .all(|pair| pair[0]["ElapsedNs"].as_u64() >= pair[1]["ElapsedNs"].as_u64())
    );
    assert!(steps.iter().any(|step| step["Depth"].as_u64().unwrap() > 0));
    for format in ["text", "table"] {
        let result = list(&uri, "Steps.lean", "repeated", format);
        assert_code(&result, 0);
        assert!(String::from_utf8(result.stdout).unwrap().contains("Elab."));
    }
    assert_code(&list(&uri, "missing.lean", "repeated", "json"), 254);
    assert_code(&list(&uri, "Steps.lean", "missing", "json"), 254);
    let none = record(&root, 0);
    assert_code(
        &list(&none.to_string(), "Steps.lean", "repeated", "json"),
        254,
    );
    let mut invalid = profile.clone();
    invalid["Files"][0]["Commands"][0]
        .as_object_mut()
        .unwrap()
        .remove("Steps");
    std::fs::write(&path, invalid.to_string()).unwrap();
    assert_code(&list(&uri, "Steps.lean", "repeated", "json"), 254);
    assert_code(&compare(&invalid, &invalid, &[]), 254);
    let mut heavier = profile.clone();
    let row = heavier["Files"][0]["Commands"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|row| row["Key"] == "repeated")
        .unwrap();
    row["Heartbeats"] = json!(row["Heartbeats"].as_u64().unwrap() * 3);
    row["Steps"] = json!([]);
    let result = comparison(&profile, &heavier, &[], 254);
    assert_eq!(result["Summary"]["Regressed"], 1);
    assert_eq!(result["Commands"][0]["Key"], "repeated");
}

#[test]
fn deep_steps_can_be_read_listed_compared_and_serialized() {
    let mut profile = synthetic_profile(&[("Deep.lean", "deep", 10000)]);
    profile["Conditions"]["Detail"] = json!("steps");
    profile["Files"][0]["Commands"][0]["Steps"] = json!("TREE");
    let mut tree =
        String::from("{\"Class\":\"leaf\",\"ElapsedNs\":9999,\"Range\":null,\"Children\":[]}");
    for _ in 0..200 {
        tree = format!(
            "{{\"Class\":\"parent\",\"ElapsedNs\":1,\"Range\":null,\"Children\":[{tree}]}}"
        );
    }
    let root = fixture("steps");
    let path = root.join("deep.json");
    std::fs::write(
        &path,
        profile
            .to_string()
            .replace("\"TREE\"", &format!("[{tree}]")),
    )
    .unwrap();
    let uri = format!("file://{}", path.display());
    let listed = cli(&[
        "profile",
        "list-steps",
        "--profile",
        &uri,
        "--path",
        "Deep.lean",
        "--key",
        "deep",
        "--limit",
        "1",
    ]);
    assert_code(&listed, 0);
    let listed: Value = serde_json::from_slice(&listed.stdout).unwrap();
    assert_eq!(listed["Steps"][0]["Depth"], 200);
    assert_eq!(listed["Steps"][0]["Class"], "leaf");
    let compared = cli(&[
        "profile",
        "compare-profiles",
        "--base-profile",
        &uri,
        "--head-profile",
        &uri,
    ]);
    assert_code(&compared, 0);
    assert_eq!(
        serde_json::from_slice::<Value>(&compared.stdout).unwrap()["Summary"]["Unchanged"],
        1
    );
    let commands = cli(&["profile", "list-commands", "--profile", &uri]);
    assert_code(&commands, 0);
    assert!(
        String::from_utf8(commands.stdout)
            .unwrap()
            .contains("\"Class\": \"leaf\"")
    );
    let mut input = std::fs::read_to_string(&path).unwrap();
    input.push_str(" {}");
    std::fs::write(&path, input).unwrap();
    assert_code(&cli(&["profile", "list-files", "--profile", &uri]), 252);
}

#[test]
fn nested_command_elaborators_preserve_synchronous_proof_costs() {
    for detail in ["none", "steps"] {
        let root = fixture("async");
        assert_code(&lake_build(&root), 0);
        let measure = || {
            let output = cli(&[
                "profile",
                "record-profile",
                "--project-dir",
                root.to_str().unwrap(),
                "--detail",
                detail,
            ]);
            assert_code(&output, 0);
            serde_json::from_slice::<Value>(&output.stdout).unwrap()
        };
        let base = measure();
        let path = root.join("Probe.lean");
        let source = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, source.replace("500", "1500")).unwrap();
        let head = measure();
        let result = comparison(&base, &head, &[], 254);
        assert_eq!(result["Summary"]["Regressed"], 1);
        assert_eq!(result["Commands"][0]["Key"], "escaped");
        assert!(result["Commands"][0]["BaseHeartbeats"].as_u64().unwrap() > 100_000);
        assert_eq!(base["Conditions"]["Async"], false);
    }
}

#[test]
fn unfinished_elaboration_tasks_are_reported_as_errors() {
    let root = fixture("async");
    std::fs::write(root.join("lakefile.toml"), "name = \"pending\"\ndefaultTargets = [\"Probe\"]\n[[lean_lib]]\nname = \"Pending\"\n[[lean_lib]]\nname = \"Probe\"\n").unwrap();
    std::fs::write(
        root.join("Pending.lean"),
        r#"import Lean
initialize pendingSnapshot : IO.Ref (Option (IO.Promise Lean.Language.SnapshotTree)) ← IO.mkRef none
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("Probe.lean"),
        r#"import Pending
open Lean Elab Command
run_cmd do
  let promise ← IO.Promise.new
  pendingSnapshot.set (some promise)
  logSnapshotTask { stx? := none, cancelTk? := none, task := promise.result! }
"#,
    )
    .unwrap();
    for detail in ["none", "steps"] {
        let output = cli(&[
            "profile",
            "record-profile",
            "--project-dir",
            root.to_str().unwrap(),
            "--detail",
            detail,
        ]);
        assert_code(&output, 254);
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("unfinished elaboration task"),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let profile: Value = serde_json::from_slice(&output.stdout).unwrap();
        let file = profile["Files"]
            .as_array()
            .unwrap()
            .iter()
            .find(|file| file["Path"] == "Probe.lean")
            .unwrap();
        assert_eq!(file["HasErrors"], true);
        assert_eq!(
            file["Commands"].as_array().unwrap().last().unwrap()["HasErrors"],
            true
        );
    }
}

#[test]
fn imported_options_are_reparsed_like_the_lean_frontend() {
    let root = fixture("options");
    let config = root.join("lakefile.toml");
    let original = std::fs::read_to_string(&config).unwrap();
    for strong in [false, true] {
        let text = if strong {
            original.replace("weak.probe.", "probe.")
        } else {
            original.clone()
        };
        std::fs::write(&config, &text).unwrap();
        assert_code(&lake_build(&root), 0);
        let profile = record(&root, 0);
        let rows = commands(&profile);
        let theorem = rows.iter().find(|row| row["Key"] == "configured").unwrap();
        assert!(theorem["Heartbeats"].as_u64().unwrap() > 100_000);
        for (from, to) in [
            ("probe.enabled=true", "probe.enabled=false"),
            ("probe.count=42", "probe.count=43"),
            ("probe.label=hello=world", "probe.label=other"),
        ] {
            std::fs::write(&config, text.replace(from, to)).unwrap();
            assert!(!lake_build(&root).status.success());
            record(&root, 254);
        }
    }
    std::fs::write(&config, original.replace("weak.unknown", "unknown")).unwrap();
    assert!(!lake_build(&root).status.success());
    let output = cli(&[
        "profile",
        "record-profile",
        "--project-dir",
        root.to_str().unwrap(),
    ]);
    assert_code(&output, 254);
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown configuration option"));
}

#[test]
fn module_linters_receive_the_parsed_commands_at_each_terminator() {
    for terminator in ["", "#exit\n"] {
        let root = fixture("async");
        std::fs::write(
            root.join("Probe.lean"),
            format!(
                r#"import Lean
open Lean Elab Command
run_cmd addModuleLinter {{ run := fun cmds => do
  unless cmds.isEmpty do logError "module contains commands" }}
def x : Nat := 1
{terminator}"#
            ),
        )
        .unwrap();
        assert!(!lake_build(&root).status.success());
        let output = cli(&[
            "profile",
            "record-profile",
            "--project-dir",
            root.to_str().unwrap(),
        ]);
        assert_code(&output, 254);
        assert!(String::from_utf8_lossy(&output.stderr).contains("module contains commands"));
        let profile: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(profile["Files"][0]["HasErrors"], true);
        let rows = commands(&profile);
        assert!(
            rows.iter()
                .any(|row| row["Key"] == "x" && row["HasErrors"] == false)
        );
        if !terminator.is_empty() {
            assert_eq!(rows.last().unwrap()["HasErrors"], true);
        }
    }
}

#[cfg(unix)]
#[test]
fn default_selection_excludes_symlink_files_and_ancestors() {
    for ancestor in [false, true] {
        let root = fixture("async");
        std::fs::write(root.join("Probe.lean"), "private def hidden : Nat := 1\n").unwrap();
        if ancestor {
            std::fs::create_dir(root.join("real")).unwrap();
            std::fs::rename(root.join("Probe.lean"), root.join("real/Probe.lean")).unwrap();
            std::os::unix::fs::symlink("real", root.join("linked")).unwrap();
            let config = root.join("lakefile.toml");
            let text = std::fs::read_to_string(&config)
                .unwrap()
                .replace("name = \"async\"", "name = \"async\"\nsrcDir = \"linked\"");
            std::fs::write(config, text).unwrap();
        } else {
            std::fs::rename(root.join("Probe.lean"), root.join("Real.lean")).unwrap();
            std::os::unix::fs::symlink("Real.lean", root.join("Probe.lean")).unwrap();
        }
        assert_code(&lake_build(&root), 0);
        let output = cli(&[
            "profile",
            "record-profile",
            "--project-dir",
            root.to_str().unwrap(),
        ]);
        assert_code(&output, 252);
        assert!(String::from_utf8_lossy(&output.stderr).contains("no Lean source files selected"));
    }
}

#[test]
fn help_accepts_global_options_without_changing_parameter_values() {
    for path in [
        vec![],
        vec!["profile"],
        vec!["profile", "record-profile"],
        vec!["profile", "compare-profiles"],
        vec!["profile", "list-files"],
        vec!["profile", "list-commands"],
        vec!["profile", "list-steps"],
    ] {
        for globals in [
            vec!["--debug"],
            vec!["--no-debug"],
            vec!["--output", "text"],
            vec!["--output=json"],
        ] {
            for position in 0..=path.len() + 1 {
                let mut args = path.clone();
                args.push("help");
                args.splice(position..position, globals.clone());
                let output = cli(&args);
                assert_code(&output, 0);
                assert!(String::from_utf8_lossy(&output.stdout).contains("Usage:"));
            }
        }
    }
    let profile = synthetic_profile(&[("help", "help", 1)]);
    let output = cli(&["profile", "list-files", "--profile", &profile.to_string()]);
    assert_code(&output, 0);
    assert!(String::from_utf8_lossy(&output.stdout).contains("help"));
    for parameter in ["--profile", "--profile=help"] {
        let mut args = vec!["profile", "list-files", parameter];
        if parameter == "--profile" {
            args.push("help");
        }
        let output = cli(&args);
        assert_code(&output, 252);
        assert!(!String::from_utf8_lossy(&output.stdout).contains("Usage:"));
    }
    let root = fixture("async");
    let output = cli(&[
        "profile",
        "record-profile",
        "--project-dir",
        root.to_str().unwrap(),
        "--paths",
        "help",
    ]);
    assert_code(&output, 252);
    assert!(String::from_utf8_lossy(&output.stderr).contains("help"));
    assert!(!String::from_utf8_lossy(&output.stdout).contains("Usage:"));
}

#[test]
fn single_file_comparisons_handle_twenty_thousand_commands() {
    let mut profile = synthetic_profile(&[("Large.lean", "template", 100)]);
    let template = profile["Files"][0]["Commands"][0].clone();
    profile["Files"][0]["Commands"] = Value::Array(
        (0..20_000)
            .map(|index| {
                let mut command = template.clone();
                command["Key"] = json!(format!("command{index}"));
                command
            })
            .collect(),
    );
    let mut head = profile.clone();
    head["Files"][0]["Commands"]
        .as_array_mut()
        .unwrap()
        .reverse();
    let result = comparison(&profile, &head, &[], 0);
    assert_eq!(result["Summary"]["Unchanged"], 20_000);
    assert_eq!(result["Summary"]["BaseHeartbeats"], 2_000_000);
    assert_eq!(result["Summary"]["HeadHeartbeats"], 2_000_000);
    assert_eq!(result["Commands"], json!([]));
}

fn hotspots(args: &[&str], code: i32) -> Value {
    let mut all = vec!["profile", "list-hotspots"];
    all.extend_from_slice(args);
    let output = cli(&all);
    assert_code(&output, code);
    serde_json::from_slice(&output.stdout).unwrap()
}

fn assert_shares(listing: &Value) {
    let rows = listing["Hotspots"].as_array().unwrap();
    let total = listing["ElapsedNs"].as_u64().unwrap();
    let listed: u64 = rows
        .iter()
        .map(|row| row["ElapsedNs"].as_u64().unwrap())
        .sum();
    assert_eq!(listing["ListedElapsedNs"], listed);
    assert!(listed <= total);
    let shares: f64 = rows.iter().map(|row| row["Share"].as_f64().unwrap()).sum();
    assert!(shares <= 1.0 + 1e-9, "{shares}");
    assert!(listing["ListedShare"].as_f64().unwrap() <= 1.0);
    assert!(
        rows.windows(2)
            .all(|pair| pair[0]["ElapsedNs"].as_u64() >= pair[1]["ElapsedNs"].as_u64())
    );
    for row in rows {
        let Some(traced) = row.get("Traced") else {
            continue;
        };
        let inside = traced["Inside"].as_array().unwrap();
        let elapsed: u64 = inside
            .iter()
            .map(|entry| entry["ElapsedNs"].as_u64().unwrap())
            .sum();
        assert!(elapsed <= traced["ElapsedNs"].as_u64().unwrap(), "{row}");
        let shares: f64 = inside
            .iter()
            .map(|entry| entry["Share"].as_f64().unwrap())
            .sum();
        assert!(shares <= 1.0 + 1e-9, "{row}");
    }
}

#[test]
fn hotspots_rank_the_slow_declaration_and_its_dominant_tactic() {
    let root = fixture("hotspots");
    assert_code(&lake_build(&root), 0);
    let dir = root.to_str().unwrap();
    let listing = hotspots(&["--project-dir", dir, "--jobs", "4", "--limit", "50"], 0);
    pascal_case(&listing);
    assert_eq!(listing["Detail"], "steps");
    assert_eq!(listing["Files"], 3);
    assert_eq!(listing["Commands"], 6);
    assert_eq!(listing["FilesWithErrors"], 0);
    let rows = listing["Hotspots"].as_array().unwrap();
    assert_eq!(rows.len(), 6);
    let total: u64 = rows
        .iter()
        .map(|row| row["ElapsedNs"].as_u64().unwrap())
        .sum();
    assert_eq!(listing["ElapsedNs"], total);
    assert_shares(&listing);
    let first = &rows[0];
    assert_eq!(first["Path"], "Hot/Slow.lean");
    assert_eq!(first["Range"]["Start"], json!({"Line": 4, "Column": 0}));
    assert_eq!(first["Name"], "slow");
    assert_eq!(first["Key"], "slow");
    let inside = &first["Traced"]["Inside"][0];
    assert_eq!(inside["Class"], "Leanwake.tactic");
    assert_eq!(inside["Kind"], "Lean.Parser.Tactic.decide");
    assert_eq!(
        inside["Range"],
        json!({"Start": {"Line": 6, "Column": 4}, "End": {"Line": 6, "Column": 18}})
    );
    assert_eq!(inside["Count"], 1);
    let term = rows.iter().find(|row| row["Name"] == "termMode").unwrap();
    let entries = term["Traced"]["Inside"].as_array().unwrap();
    assert!(!entries.is_empty());
    assert!(
        entries
            .iter()
            .all(|entry| entry["Kind"].is_null() && entry["Range"].is_null())
    );
    let top = hotspots(&["--project-dir", dir, "--limit", "1"], 0);
    assert_eq!(top["Hotspots"].as_array().unwrap().len(), 1);
    assert_eq!(top["Hotspots"][0]["Name"], "slow");
    assert_eq!(
        top["Hotspots"][0]["Traced"]["Inside"][0]["Kind"],
        "Lean.Parser.Tactic.decide"
    );
    assert_eq!(top["Commands"], 6);
    assert!(top["ListedShare"].as_f64().unwrap() < 1.0);
    assert_shares(&top);
    let untraced = hotspots(&["--project-dir", dir, "--detail", "none"], 0);
    assert_eq!(untraced["Detail"], "none");
    assert!(untraced["Hotspots"][0].get("Traced").is_none());
    assert_eq!(untraced["Hotspots"][0]["Name"], "slow");
    let table = cli(&[
        "profile",
        "list-hotspots",
        "--project-dir",
        dir,
        "--limit",
        "1",
        "--output",
        "table",
    ]);
    assert_code(&table, 0);
    let table = String::from_utf8(table.stdout).unwrap();
    let lines: Vec<_> = table.lines().collect();
    assert_eq!(lines.len(), 3, "{table}");
    assert!(lines[0].contains("6 commands and 3 files"), "{table}");
    assert!(
        lines[2].contains("  Hot/Slow.lean:4  slow  ") && lines[2].ends_with("%"),
        "{table}"
    );
    assert!(lines[2].contains("  decide 6:4-6:18 x1 "), "{table}");
}

#[test]
fn hotspots_from_a_recording_follow_its_elapsed_times() {
    let root = fixture("hotspots");
    assert_code(&lake_build(&root), 0);
    let profile = record(&root, 0);
    let path = root.join("profile.json");
    std::fs::write(&path, profile.to_string()).unwrap();
    let uri = format!("file://{}", path.display());
    let listing = hotspots(&["--profile", &uri, "--limit", "50"], 0);
    pascal_case(&listing);
    assert_eq!(listing["Detail"], "none");
    let mut expected: Vec<_> = profile["Files"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|file| {
            file["Commands"]
                .as_array()
                .unwrap()
                .iter()
                .map(move |command| {
                    (
                        command["ElapsedNs"].as_u64().unwrap(),
                        file["Path"].as_str().unwrap().to_owned(),
                        command["Key"].as_str().unwrap().to_owned(),
                    )
                })
        })
        .collect();
    expected.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| a.1.cmp(&b.1))
            .then_with(|| a.2.cmp(&b.2))
    });
    let actual: Vec<_> = listing["Hotspots"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            (
                row["ElapsedNs"].as_u64().unwrap(),
                row["Path"].as_str().unwrap().to_owned(),
                row["Key"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert_eq!(actual, expected);
    assert_eq!(
        listing["ElapsedNs"],
        expected.iter().map(|row| row.0).sum::<u64>()
    );
    assert_shares(&listing);
    for row in listing["Hotspots"].as_array().unwrap() {
        assert!(row.get("Traced").is_none());
        let command = profile["Files"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|file| file["Path"] == row["Path"])
            .flat_map(|file| file["Commands"].as_array().unwrap())
            .find(|command| command["Key"] == row["Key"])
            .unwrap();
        assert_eq!(row["Heartbeats"], command["Heartbeats"]);
        assert_eq!(row["Range"], command["Range"]);
    }
    let again = hotspots(&["--profile", &uri, "--limit", "50"], 0);
    assert_eq!(again, listing);
    for format in ["text", "table"] {
        let output = cli(&[
            "profile",
            "list-hotspots",
            "--profile",
            &uri,
            "--output",
            format,
        ]);
        assert_code(&output, 0);
        assert!(!output.stdout.is_empty());
    }
    let mut steps = profile.clone();
    steps["Conditions"]["Detail"] = json!("steps");
    for file in steps["Files"].as_array_mut().unwrap() {
        for command in file["Commands"].as_array_mut().unwrap() {
            command["Steps"] = json!([]);
        }
    }
    let steps_path = root.join("steps.json");
    std::fs::write(&steps_path, steps.to_string()).unwrap();
    let output = cli(&[
        "profile",
        "list-hotspots",
        "--profile",
        &format!("file://{}", steps_path.display()),
    ]);
    assert_code(&output, 254);
    assert!(output.stdout.is_empty());
    let mut old = profile.clone();
    old["SchemaVersion"] = json!(1);
    let output = cli(&["profile", "list-hotspots", "--profile", &old.to_string()]);
    assert_code(&output, 254);
    assert!(String::from_utf8_lossy(&output.stderr).contains("SchemaVersion=1"));
    let dir = root.to_str().unwrap();
    for args in [
        vec![],
        vec!["--project-dir", dir, "--profile", &uri, "--detail", "none"],
        vec!["--profile", &uri, "--detail", "steps"],
        vec!["--profile", &uri, "--jobs", "2"],
        vec!["--profile", &uri, "--paths", "Hot.lean"],
        vec!["--profile", &uri, "--limit", "0"],
        vec!["--project-dir", dir, "--detail", "unknown"],
    ] {
        let mut all = vec!["profile", "list-hotspots"];
        all.extend(args);
        assert_eq!(cli(&all).status.code(), Some(252), "{all:?}");
    }
    assert_code(&cli(&["profile", "list-hotspots", "help"]), 0);
}

#[test]
fn hotspots_resume_validated_checkpoints() {
    let root = fixture("hotspots");
    assert_code(&lake_build(&root), 0);
    let profile = record(&root, 0);
    let path = root.join("profile.json");
    let uri = format!("file://{}", path.display());
    let dir = root.to_str().unwrap();
    std::fs::write(&path, profile.to_string()).unwrap();
    let listing = hotspots(
        &["--profile", &uri, "--project-dir", dir, "--limit", "1"],
        0,
    );
    assert_eq!(listing["Detail"], "steps");
    assert!(listing["Hotspots"][0].get("Traced").is_some());
    let untraced = hotspots(&["--profile", &uri, "--limit", "1"], 0);
    for field in ["Key", "Name", "ElapsedNs", "Heartbeats", "Share"] {
        assert_eq!(
            listing["Hotspots"][0][field],
            untraced["Hotspots"][0][field]
        );
    }
    for field in ["Toolchain", "Conditions", "SourceSha256", "Key"] {
        let mut changed = profile.clone();
        match field {
            "Toolchain" => changed[field] = json!("different Lean"),
            "Conditions" => changed[field]["Async"] = json!(true),
            "SourceSha256" => changed["Files"][0][field] = json!("changed"),
            "Key" => {
                for file in changed["Files"].as_array_mut().unwrap() {
                    for command in file["Commands"].as_array_mut().unwrap() {
                        command["Key"] = json!("different identity");
                    }
                }
            }
            _ => unreachable!(),
        }
        std::fs::write(&path, changed.to_string()).unwrap();
        let output = cli(&[
            "profile",
            "list-hotspots",
            "--profile",
            &uri,
            "--project-dir",
            dir,
            "--limit",
            "1",
        ]);
        assert_code(&output, 254);
        assert!(output.stdout.is_empty(), "{field}");
    }
    let mut empty = profile.clone();
    empty["Files"] = json!([]);
    std::fs::write(&path, empty.to_string()).unwrap();
    let listing = hotspots(&["--profile", &uri, "--project-dir", dir], 0);
    assert!(listing["Hotspots"].as_array().unwrap().is_empty());
    std::fs::write(&path, profile.to_string()).unwrap();
    let source = root.join("Hot/Quick.lean");
    let original = std::fs::read_to_string(&source).unwrap();
    std::fs::write(&source, format!("{original}\n")).unwrap();
    let output = cli(&[
        "profile",
        "list-hotspots",
        "--profile",
        &uri,
        "--project-dir",
        dir,
        "--limit",
        "1",
    ]);
    assert_code(&output, 254);
    assert!(String::from_utf8_lossy(&output.stderr).contains("source changed since recording"));
}

#[test]
fn hotspots_print_traced_timeouts_without_declarations() {
    let root = fixture("hotspots");
    std::fs::write(
        root.join("Hot/Burn.lean"),
        r#"import Lean
open Lean Elab Tactic
elab "burn" : tactic => do
  for _ in [:8000] do
    withTraceNode `Burn (fun _ => pure "burn") (pure ())
  evalTactic (← `(tactic| exact True.intro))
def demand (proof : True := by burn) : True := proof
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("Hot.lean"),
        r#"import Hot.Burn
set_option maxHeartbeats 20 in
def tracedTimeout : (fun (_ : True) => True) demand := True.intro
"#,
    )
    .unwrap();
    assert_code(&lake_build(&root), 0);
    let profile = record(&root, 0);
    let command = profile["Files"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|file| file["Commands"].as_array().unwrap())
        .find(|command| command["Key"] == "tracedTimeout")
        .unwrap();
    assert_eq!(command["HasErrors"], false);
    let listing = hotspots(
        &[
            "--project-dir",
            root.to_str().unwrap(),
            "--paths",
            "Hot.lean",
            "--limit",
            "50",
        ],
        254,
    );
    let row = listing["Hotspots"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["Key"] == "tracedTimeout")
        .unwrap();
    assert_eq!(row["Name"], "tracedTimeout");
    assert_eq!(row["HasErrors"], false);
    assert_eq!(row["Traced"]["HasErrors"], true);
    let output = cli(&[
        "profile",
        "record-profile",
        "--project-dir",
        root.to_str().unwrap(),
        "--paths",
        "Hot.lean",
        "--detail",
        "steps",
    ]);
    assert_code(&output, 254);
    assert!(String::from_utf8_lossy(&output.stderr).contains("maximum number of heartbeats"));
    let traced: Value = serde_json::from_slice(&output.stdout).unwrap();
    let failed = traced["Files"][0]["Commands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|command| command["HasErrors"] == true)
        .unwrap();
    assert_ne!(failed["Key"], "tracedTimeout");
    assert!(failed["Names"].as_array().unwrap().is_empty());
}

#[test]
fn hotspots_report_files_with_errors() {
    let root = fixture("errors");
    let listing = hotspots(
        &["--project-dir", root.to_str().unwrap(), "--jobs", "4"],
        254,
    );
    assert!(listing["FilesWithErrors"].as_u64().unwrap() > 0);
    assert_shares(&listing);
}
