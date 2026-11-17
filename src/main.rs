mod cli;
mod error;
mod model;
mod process;
mod project;

use clap::{CommandFactory, Parser};
use cli::{Cli, Command, Output, ProfileCommand};
use error::{Error, Result};
use model::Profile;
use serde_json::{Value, json};
use std::io::Write;

fn load_profile(input: &str) -> Result<Profile> {
    let profile: Profile =
        serde_json::from_str(input).map_err(|e| Error::parameter(format!("--profile: {e}")))?;
    if profile.schema_version != 1
        || profile.conditions.r#async
        || profile.conditions.detail != "none"
    {
        return Err(Error::target(
            "unsupported profile schema or measurement conditions",
        ));
    }
    Ok(profile)
}

fn file_rows(profile: &Profile) -> Result<Vec<Value>> {
    profile
        .files
        .iter()
        .map(|file| {
            let sum = |f: fn(&model::Command) -> u64| {
                file.commands
                    .iter()
                    .try_fold(0u64, |total, command| total.checked_add(f(command)))
                    .ok_or_else(|| Error::target("profile totals exceed 64-bit integers"))
            };
            Ok(
                json!({"Path": file.path, "Module": file.module, "Commands": file.commands.len(),
            "Heartbeats": sum(|c| c.heartbeats)?, "ElapsedNs": sum(|c| c.elapsed_ns)?,
            "ImportElapsedNs": file.import_elapsed_ns, "HasErrors": file.has_errors}),
            )
        })
        .collect()
}

fn emit(value: &Value, rows: &[Value], output: Output) -> Result<()> {
    let mut stdout = std::io::stdout().lock();
    match output {
        Output::Json => writeln!(stdout, "{}", serde_json::to_string_pretty(value)?)?,
        Output::Text | Output::Table => {
            let command_rows = rows.first().is_some_and(|row| row.get("Key").is_some());
            let fields: &[&str] = if command_rows {
                &["Heartbeats", "ElapsedNs", "Path", "Key", "HasErrors"]
            } else {
                &[
                    "Path",
                    "Module",
                    "Commands",
                    "Heartbeats",
                    "ElapsedNs",
                    "HasErrors",
                ]
            };
            if matches!(output, Output::Table) {
                writeln!(stdout, "{}", fields.join("\t"))?;
            }
            for row in rows {
                let values: Vec<_> = fields
                    .iter()
                    .map(|key| match &row[*key] {
                        Value::String(text) => text.clone(),
                        value => value.to_string(),
                    })
                    .collect();
                writeln!(stdout, "{}", values.join("\t"))?;
            }
        }
    }
    Ok(())
}

fn run(cli: Cli) -> Result<u8> {
    process::DEBUG.store(cli.debug, std::sync::atomic::Ordering::Relaxed);
    let Command::Profile { command } = cli.command;
    match command {
        ProfileCommand::RecordProfile(options) => {
            let profile = project::record(&options)?;
            let failed = profile.files.iter().any(|file| file.has_errors);
            let value = serde_json::to_value(&profile)?;
            if let Some(path) = &options.output_file {
                let mut file = std::fs::File::create(path)?;
                writeln!(file, "{}", serde_json::to_string_pretty(&value)?)?;
            } else {
                emit(&value, &file_rows(&profile)?, cli.output)?;
            }
            Ok(if failed { 254 } else { 0 })
        }
        ProfileCommand::ListFiles { profile } => {
            let rows = file_rows(&load_profile(&profile)?)?;
            emit(&json!({"Files": rows}), &rows, cli.output)?;
            Ok(0)
        }
        ProfileCommand::ListCommands { profile, limit } => {
            let profile = load_profile(&profile)?;
            let mut commands: Vec<_> = profile
                .files
                .iter()
                .flat_map(|file| file.commands.iter().map(move |command| (file, command)))
                .collect();
            commands.sort_by(|(af, a), (bf, b)| {
                b.heartbeats
                    .cmp(&a.heartbeats)
                    .then_with(|| af.path.cmp(&bf.path))
                    .then_with(|| a.key.cmp(&b.key))
            });
            if let Some(limit) = limit {
                commands.truncate(limit);
            }
            let rows: Vec<_> = commands
                .into_iter()
                .map(|(file, command)| {
                    let mut row = serde_json::to_value(command).expect("command serialization");
                    row["Path"] = json!(file.path);
                    row["Module"] = json!(file.module);
                    row
                })
                .collect();
            emit(&json!({"Commands": rows}), &rows, cli.output)?;
            Ok(0)
        }
    }
}

fn main() -> std::process::ExitCode {
    let mut args: Vec<_> = std::env::args_os().collect();
    if args.last().is_some_and(|arg| arg == "help") {
        let command = Cli::command();
        let mut current = Some(&command);
        for arg in &args[1..args.len() - 1] {
            current =
                current.and_then(|cmd| arg.to_str().and_then(|name| cmd.find_subcommand(name)));
        }
        if current.is_some() {
            *args.last_mut().unwrap() = "--help".into();
        }
    }
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(error) => {
            let code = if error.use_stderr() { 252 } else { 0 };
            let _ = error.print();
            return code.into();
        }
    };
    match run(cli) {
        Ok(code) => code.into(),
        Err(error) => {
            eprintln!("leanwake: {error}");
            error.code.into()
        }
    }
}
