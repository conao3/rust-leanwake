use crate::cli::Record;
use crate::error::{Error, Result};
use crate::model::{Conditions, File, Profile, Tool};
use crate::process;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Metadata {
    path: PathBuf,
    module: String,
    registered: bool,
    arguments: Vec<String>,
    options: Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ProjectInfo {
    files: Vec<Metadata>,
    environment: HashMap<String, String>,
}

struct Prepared {
    metadata: Metadata,
    path: String,
    setup: Option<Value>,
    source: String,
}

fn lake(root: &Path) -> Command {
    let mut command = Command::new("lake");
    command.current_dir(root).arg("--no-cache");
    command
}

fn query(root: &Path, targets: &[String]) -> Result<Vec<Value>> {
    let bytes = process::checked(lake(root).args(["-J", "query"]).args(targets))?;
    serde_json::Deserializer::from_slice(&bytes)
        .into_iter::<Value>()
        .map(|value| value.map_err(Error::from))
        .collect()
}

fn visit(root: &Path, path: &Path, files: &mut BTreeSet<PathBuf>) -> Result<()> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| Error::parameter("--paths must stay inside --project-dir"))?;
    let mut cursor = root.to_path_buf();
    for part in relative.components() {
        match part {
            std::path::Component::Normal(name) => {
                if name.to_string_lossy().starts_with('.') {
                    return Ok(());
                }
                cursor.push(name);
            }
            std::path::Component::ParentDir => {
                if cursor == root {
                    return Err(Error::parameter("--paths must stay inside --project-dir"));
                }
                cursor.pop();
            }
            std::path::Component::CurDir => continue,
            _ => return Err(Error::parameter("invalid source path")),
        }
        if std::fs::symlink_metadata(&cursor)
            .map_err(|e| Error::parameter(format!("{}: {e}", cursor.display())))?
            .file_type()
            .is_symlink()
        {
            return Ok(());
        }
    }
    let meta = std::fs::symlink_metadata(path)
        .map_err(|e| Error::parameter(format!("{}: {e}", path.display())))?;
    if meta.file_type().is_symlink() {
        return Ok(());
    }
    let absolute = path.canonicalize()?;
    if !absolute.starts_with(root) {
        return Err(Error::parameter("--paths must stay inside --project-dir"));
    }
    if meta.is_dir() {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            if !entry.file_name().to_string_lossy().starts_with('.') {
                visit(root, &entry.path(), files)?;
            }
        }
    } else if path.extension().is_some_and(|x| x == "lean") {
        files.insert(absolute);
    }
    Ok(())
}

fn prepare(
    options: &Record,
    root: &Path,
    helper: &Path,
) -> Result<(Vec<Prepared>, HashMap<String, String>)> {
    let mut files = BTreeSet::new();
    if options.paths.is_empty() {
        let values = query(root, &[":defaultModules".into()])?;
        let modules: Vec<String> = serde_json::from_value(
            values
                .into_iter()
                .next()
                .ok_or_else(|| Error::target("Lake returned no default modules"))?,
        )?;
        if !modules.is_empty() {
            let imports: Vec<_> = modules
                .iter()
                .map(|name| format!("+{name}:transImports"))
                .collect();
            let mut modules: BTreeSet<String> = modules.into_iter().collect();
            for value in query(root, &imports)? {
                modules.extend(serde_json::from_value::<Vec<String>>(value)?);
            }
            let targets: Vec<_> = modules.iter().map(|name| format!("+{name}:lean")).collect();
            for value in query(root, &targets)? {
                let path: PathBuf = serde_json::from_value(value)?;
                let path = root.join(path);
                if path.starts_with(root) {
                    visit(root, &path, &mut files)?;
                }
            }
        }
    } else {
        for path in &options.paths {
            visit(root, &root.join(path), &mut files)?;
        }
    }
    if files.is_empty() {
        return Err(Error::parameter("no Lean source files selected"));
    }
    let info: ProjectInfo = serde_json::from_slice(&process::checked(
        lake(root).arg("env").arg(helper).args(&files),
    )?)?;
    let mut prepared = Vec::new();
    for metadata in info.files {
        let path = metadata
            .path
            .strip_prefix(root)
            .map_err(|_| Error::target("source outside project"))?
            .to_str()
            .ok_or_else(|| Error::parameter("source paths must be UTF-8"))?
            .replace('\\', "/");
        let source = std::fs::read_to_string(&metadata.path)?;
        let mut command = lake(root);
        if metadata.registered {
            command.args(["-J", "query", &format!("+{}:setup", metadata.module)]);
        } else {
            command.arg("setup-file").arg(&metadata.path);
        }
        let output = process::run(&mut command, None)?;
        let setup = if output.status.success() {
            let mut setup: Value = serde_json::from_slice(&output.stdout)?;
            if !metadata.registered {
                setup["options"] = metadata.options.clone();
                setup["name"] = json!(metadata.module);
            }
            Some(setup)
        } else {
            eprintln!(
                "leanwake: cannot prepare {path}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            None
        };
        prepared.push(Prepared {
            metadata,
            path,
            setup,
            source,
        });
    }
    Ok((prepared, info.environment))
}

fn measure(
    root: &Path,
    adapter: &Path,
    environment: &HashMap<String, String>,
    file: &Prepared,
    options: &Record,
) -> Result<File> {
    let Some(setup) = &file.setup else {
        return Ok(File {
            path: file.path.clone(),
            module: file.metadata.module.clone(),
            source_sha256: sha256(&file.source),
            has_errors: true,
            import_elapsed_ns: 0,
            commands: Vec::new(),
        });
    };
    let request = json!({"path": file.path, "setup": setup, "arguments": file.metadata.arguments,
        "detail": options.detail.as_str(), "stepThresholdNs": options.step_threshold_ns});
    let mut input = serde_json::to_vec(&request)?;
    input.push(b'\n');
    let output = process::run(
        Command::new(adapter).current_dir(root).envs(environment),
        Some(&input),
    )?;
    if !output.status.success() {
        return Err(Error::target(format!(
            "adapter failed for {}: {}",
            file.path,
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    let measured: File = crate::json_io::read(&output.stdout)
        .map_err(|e| Error::internal(format!("invalid adapter response for {}: {e}", file.path)))?;
    if measured.has_errors {
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
    }
    if measured.path != file.path
        || measured.module != file.metadata.module
        || measured.source_sha256 != sha256(&file.source)
    {
        return Err(Error::target(format!(
            "source changed during measurement: {}",
            file.path
        )));
    }
    Ok(measured)
}

pub fn record(options: &Record) -> Result<Profile> {
    let root = options
        .project_dir
        .canonicalize()
        .map_err(|e| Error::parameter(format!("--project-dir: {e}")))?;
    let toolchain = std::fs::read_to_string(root.join("lean-toolchain"))
        .map_err(|e| Error::target(format!("lean-toolchain: {e}")))?;
    if toolchain.trim() != "leanprover/lean4:v4.35.0" {
        return Err(Error::target("project must use leanprover/lean4:v4.35.0"));
    }
    let adapter = PathBuf::from(
        std::env::var_os("LEANWAKE_ADAPTER")
            .ok_or_else(|| Error::internal("LEANWAKE_ADAPTER is required"))?,
    )
    .canonicalize()?;
    let helper = adapter
        .parent()
        .ok_or_else(|| Error::internal("adapter has no directory"))?
        .join("leanwake-project");
    let version = String::from_utf8(process::checked(Command::new("lean").arg("--version"))?)
        .map_err(|e| Error::internal(e.to_string()))?;
    if !version.starts_with("Lean (version 4.35.0,") {
        return Err(Error::target(format!("unsupported toolchain: {version}")));
    }
    let (files, environment) = prepare(options, &root, &helper)?;
    let next = AtomicUsize::new(0);
    let results = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..options.jobs.min(files.len()) {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(file) = files.get(index) else {
                        break;
                    };
                    let result = measure(&root, &adapter, &environment, file, options);
                    results.lock().unwrap().push((index, result));
                }
            });
        }
    });
    let mut results = results
        .into_inner()
        .map_err(|_| Error::internal("worker panicked"))?;
    results.sort_by_key(|(index, _)| *index);
    let files = results
        .into_iter()
        .map(|(_, result)| result)
        .collect::<Result<Vec<_>>>()?;
    Ok(Profile {
        schema_version: crate::model::SCHEMA_VERSION,
        tool: Tool {
            name: "leanwake".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        },
        toolchain: version.trim().into(),
        conditions: Conditions {
            r#async: false,
            detail: options.detail.as_str().into(),
        },
        files,
    })
}

fn sha256(source: &str) -> String {
    Sha256::digest(source.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
