use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};
use std::ffi::OsString;
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

fn detail(input: &str) -> Result<Detail, String> {
    Detail::from_str(value(input)?.trim(), false)
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Detail {
    None,
    Steps,
}

impl Detail {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Steps => "steps",
        }
    }
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
    #[command(about = "List commands in descending elapsed time order")]
    ListCommands {
        #[arg(long, value_parser = value)]
        profile: String,
        #[arg(long, value_parser = positive)]
        limit: Option<usize>,
    },
    #[command(about = "List a command's trace nodes in descending elapsed time order")]
    ListSteps {
        #[arg(long, value_parser = value)]
        profile: String,
        #[arg(long, value_parser = value)]
        path: String,
        #[arg(long, value_parser = value)]
        key: String,
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
    #[arg(long, default_value = "none", value_parser = detail)]
    pub detail: Detail,
    #[arg(long, default_value = "100000", value_parser = natural)]
    pub step_threshold_ns: u64,
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

pub fn normalize_help(args: &mut [OsString]) {
    let mut command = Cli::command();
    command.build();
    let mut current = &command;
    let mut remaining = 0usize;
    let mut required = 0usize;
    for arg in args.iter_mut().skip(1) {
        let Some(text) = arg.to_str() else {
            remaining = remaining.saturating_sub(1);
            required = required.saturating_sub(1);
            continue;
        };
        if text == "--" {
            return;
        }
        if let Some(option) = text.strip_prefix("--") {
            if required > 0 {
                return;
            }
            let (name, inline) = option
                .split_once('=')
                .map_or((option, false), |(name, _)| (name, true));
            let Some(parameter) = current
                .get_arguments()
                .find(|parameter| parameter.get_long() == Some(name))
            else {
                return;
            };
            let values = parameter.get_num_args().unwrap_or_default();
            remaining = values.max_values().saturating_sub(usize::from(inline));
            required = values.min_values().saturating_sub(usize::from(inline));
        } else if text.starts_with('-') {
            return;
        } else if remaining > 0 {
            remaining -= 1;
            required = required.saturating_sub(1);
        } else if text == "help" {
            *arg = "--help".into();
            return;
        } else if let Some(subcommand) = current.find_subcommand(text) {
            current = subcommand;
        } else {
            return;
        }
    }
}
