use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

pub fn value(input: &str) -> Result<String, String> {
    if let Some(path) = input.strip_prefix("file://") {
        std::fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))
    } else {
        Ok(input.to_owned())
    }
}

fn path(input: &str) -> Result<PathBuf, String> {
    value(input).map(|s| PathBuf::from(s.trim_end_matches(['\n', '\r'])))
}

fn natural(input: &str) -> Result<u64, String> {
    value(input)?
        .trim()
        .parse::<u64>()
        .map_err(|e| e.to_string())
}

fn positive(input: &str) -> Result<usize, String> {
    let n = value(input)?
        .trim()
        .parse::<usize>()
        .map_err(|e| e.to_string())?;
    if n == 0 {
        Err("value must be positive".into())
    } else {
        Ok(n)
    }
}

fn output(input: &str) -> Result<Output, String> {
    Output::from_str(value(input)?.trim(), false)
}

#[derive(Clone, Copy, ValueEnum)]
pub enum Output {
    Json,
    Text,
    Table,
}

#[derive(Parser)]
#[command(
    version,
    propagate_version = true,
    about = "Record Lean 4.35.0 command elaboration costs"
)]
pub struct Cli {
    #[arg(long, global = true, default_value = "json", value_parser = output)]
    pub output: Output,
    #[arg(long, global = true, overrides_with = "no_debug")]
    pub debug: bool,
    #[arg(long, global = true, overrides_with = "debug")]
    pub no_debug: bool,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    #[command(about = "Record and inspect elaboration profiles")]
    Profile {
        #[command(subcommand)]
        command: ProfileCommand,
    },
}

#[derive(Subcommand)]
pub enum ProfileCommand {
    #[command(about = "Record command costs in a Lake project")]
    RecordProfile(Record),
    #[command(about = "List commands in descending heartbeat order")]
    ListCommands {
        #[arg(long, value_parser = value)]
        profile: String,
        #[arg(long, value_parser = positive)]
        limit: Option<usize>,
    },
    #[command(about = "Compare profiles using verified heartbeat rules")]
    CompareProfiles(Compare),
    #[command(about = "List totals for each file")]
    ListFiles {
        #[arg(long, value_parser = value)]
        profile: String,
    },
}

#[derive(Args)]
pub struct Record {
    #[arg(long, value_parser = path)]
    pub project_dir: PathBuf,
    #[arg(long, num_args = 1.., value_parser = path)]
    pub paths: Vec<PathBuf>,
    #[arg(long, value_parser = path)]
    pub output_file: Option<PathBuf>,
    #[arg(long, default_value = "1", value_parser = positive)]
    pub jobs: usize,
}

#[derive(Args)]
pub struct Compare {
    #[arg(long, value_parser = value)]
    pub base_profile: String,
    #[arg(long, value_parser = value)]
    pub head_profile: String,
    #[arg(long, default_value = "1", value_parser = natural)]
    pub relative_threshold_percent: u64,
    #[arg(long, default_value = "1000", value_parser = natural)]
    pub absolute_threshold_heartbeats: u64,
}
