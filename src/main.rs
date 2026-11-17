mod cli;
mod compare;
mod error;
mod json_io;
mod model;
mod process;
mod project;
mod rules;
mod steps;

use clap::Parser;
use cli::{Cli, Command, Output, ProfileCommand};
use error::{Error, Result};
use model::Profile;
use serde_json::{Value, json};
use std::io::Write;

fn load_profile(input: &str) -> Result<Profile> {
    let profile: Profile =
        json_io::read(input.as_bytes()).map_err(|e| Error::parameter(format!("--profile: {e}")))?;
    if profile.schema_version != 1
        || profile.conditions.r#async
        || !matches!(profile.conditions.detail.as_str(), "none" | "steps")
    {
        return Err(Error::target(
            "unsupported profile schema or measurement conditions",
        ));
    }
    profile.validate_steps()?;
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
            let command_rows = value.get("Commands").is_some();
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
        ProfileCommand::ListSteps {
            profile,
            path,
            key,
            limit,
        } => steps::run(&load_profile(&profile)?, &path, &key, limit, cli.output),
        ProfileCommand::CompareProfiles(options) => compare::run(options, cli.output),
        ProfileCommand::RecordProfile(options) => {
            let profile = project::record(&options)?;
            let failed = profile.files.iter().any(|file| file.has_errors);
            if let Some(path) = &options.output_file {
                json_io::write(
                    &profile,
                    std::io::BufWriter::new(std::fs::File::create(path)?),
                )?;
            } else if matches!(cli.output, Output::Json) {
                json_io::write(&profile, std::io::stdout().lock())?;
            } else {
                emit(&json!({}), &file_rows(&profile)?, cli.output)?;
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
            if matches!(cli.output, Output::Json) {
                #[derive(serde::Serialize)]
                #[serde(rename_all = "PascalCase")]
                struct Row<'a> {
                    path: &'a str,
                    module: &'a str,
                    #[serde(flatten)]
                    command: &'a model::Command,
                }
                #[derive(serde::Serialize)]
                #[serde(rename_all = "PascalCase")]
                struct Listing<'a> {
                    commands: Vec<Row<'a>>,
                }
                let listing = Listing {
                    commands: commands
                        .iter()
                        .map(|(file, command)| Row {
                            path: &file.path,
                            module: &file.module,
                            command,
                        })
                        .collect(),
                };
                json_io::write(&listing, std::io::stdout().lock())?;
                return Ok(0);
            }
            let rows: Vec<_> = commands
                .into_iter()
                .map(|(file, command)| {
                    json!({"Path": file.path, "Key": command.key,
                        "Heartbeats": command.heartbeats, "ElapsedNs": command.elapsed_ns,
                        "HasErrors": command.has_errors})
                })
                .collect();
            emit(&json!({"Commands": rows}), &rows, cli.output)?;
            Ok(0)
        }
    }
}

fn main() -> std::process::ExitCode {
    let mut args: Vec<_> = std::env::args_os().collect();
    cli::normalize_help(&mut args);
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
