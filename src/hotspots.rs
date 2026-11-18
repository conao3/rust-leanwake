use crate::cli::{Detail, Hotspots, Output, Record, STEP_THRESHOLD_NS};
use crate::error::{Error, Result};
use crate::model::{Command, File, Profile, Range, Step};
use crate::project::{self, Targets};
use serde::Serialize;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

const TACTIC: &str = "Leanwake.tactic";

const TRACED_THRESHOLD_NS: u64 = 1_000_000;
const RIGHT_ALIGNED: [usize; 3] = [0, 1, 4];

const SEQUENCES: [&str; 3] = [
    "Lean.Parser.Tactic.tacticSeq",
    "Lean.Parser.Tactic.tacticSeq1Indented",
    "Lean.Parser.Tactic.tacticSeqBracketed",
];

type RangeKey = (u64, u64, u64, u64);

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct Listing {
    detail: &'static str,
    elapsed_ns: u64,
    commands: usize,
    files: usize,
    files_with_errors: usize,
    listed_elapsed_ns: u64,
    listed_share: f64,
    hotspots: Vec<Hotspot>,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct Hotspot {
    path: String,
    module: String,
    key: String,
    name: String,
    kind: String,
    range: Range,
    elapsed_ns: u64,
    share: f64,
    heartbeats: u64,
    has_errors: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    traced: Option<Traced>,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct Traced {
    elapsed_ns: u64,
    has_errors: bool,
    inside: Vec<Inside>,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct Inside {
    class: String,
    kind: Option<String>,
    range: Option<Range>,
    count: u64,
    elapsed_ns: u64,
    share: f64,
}

fn share(part: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        part as f64 / total as f64
    }
}

fn range_key(range: &Range) -> RangeKey {
    (
        range.start.line,
        range.start.column,
        range.end.line,
        range.end.column,
    )
}

#[derive(Clone, Copy, PartialEq)]
enum Ranges {
    Empty,
    One(RangeKey),
    Many,
}

impl Ranges {
    fn join(self, other: Self) -> Self {
        match (self, other) {
            (Self::Empty, other) | (other, Self::Empty) => other,
            (Self::One(a), Self::One(b)) if a == b => self,
            _ => Self::Many,
        }
    }
}

fn tactic_range(step: &Step) -> Option<&Range> {
    if step.class == TACTIC {
        step.range.as_ref()
    } else {
        None
    }
}

fn inside(steps: &[Step], total: u64) -> Vec<Inside> {
    let mut nodes: Vec<(&Step, Option<usize>)> = Vec::new();
    let mut pending: Vec<_> = steps.iter().rev().map(|step| (step, None)).collect();
    while let Some((step, parent)) = pending.pop() {
        let index = nodes.len();
        nodes.push((step, parent));
        pending.extend(step.children.iter().rev().map(|child| (child, Some(index))));
    }
    let mut below = vec![Ranges::Empty; nodes.len()];
    let mut sizes = vec![1usize; nodes.len()];
    for index in (0..nodes.len()).rev() {
        let (step, parent) = nodes[index];
        let own = tactic_range(step).map_or(Ranges::Empty, |range| Ranges::One(range_key(range)));
        let subtree = own.join(below[index]);
        if let Some(parent) = parent {
            below[parent] = below[parent].join(subtree);
            sizes[parent] += sizes[index];
        }
    }
    let mut covered = vec![false; nodes.len()];
    let mut groups: BTreeMap<(String, RangeKey), (Range, u64, u64)> = BTreeMap::new();
    for index in 0..nodes.len() {
        let (step, parent) = nodes[index];
        if parent.is_some_and(|parent| covered[parent]) {
            covered[index] = true;
            continue;
        }
        let Some(range) = tactic_range(step) else {
            continue;
        };
        let key = range_key(range);
        if matches!(below[index], Ranges::Empty) || below[index] == Ranges::One(key) {
            covered[index] = true;
            let kind = nodes[index..index + sizes[index]]
                .iter()
                .map(|(node, _)| node)
                .find(|node| {
                    tactic_range(node).map(range_key) == Some(key)
                        && !node
                            .kind
                            .as_deref()
                            .is_some_and(|kind| SEQUENCES.contains(&kind))
                })
                .unwrap_or(&step)
                .kind
                .clone()
                .unwrap_or_default();
            let entry = groups
                .entry((kind, key))
                .or_insert_with(|| (range.clone(), 0, 0));
            entry.1 += 1;
            entry.2 += step.elapsed_ns;
        }
    }
    let mut entries: Vec<Inside> = if groups.is_empty() {
        let mut classes: BTreeMap<&str, (u64, u64)> = BTreeMap::new();
        for (step, _) in &nodes {
            let children: u64 = step.children.iter().map(|child| child.elapsed_ns).sum();
            let entry = classes.entry(&step.class).or_default();
            entry.0 += 1;
            entry.1 += step.elapsed_ns.saturating_sub(children);
        }
        classes
            .into_iter()
            .map(|(class, (count, elapsed_ns))| Inside {
                class: class.into(),
                kind: None,
                range: None,
                count,
                elapsed_ns,
                share: share(elapsed_ns, total),
            })
            .collect()
    } else {
        groups
            .into_iter()
            .map(|((kind, _), (range, count, elapsed_ns))| Inside {
                class: TACTIC.into(),
                kind: Some(kind),
                range: Some(range),
                count,
                elapsed_ns,
                share: share(elapsed_ns, total),
            })
            .collect()
    };
    entries.sort_by(|a, b| {
        b.elapsed_ns
            .cmp(&a.elapsed_ns)
            .then_with(|| {
                a.range
                    .as_ref()
                    .map(range_key)
                    .cmp(&b.range.as_ref().map(range_key))
            })
            .then_with(|| a.kind.cmp(&b.kind))
            .then_with(|| a.class.cmp(&b.class))
    });
    entries
}

fn ranked(profile: &Profile) -> Vec<(&File, &Command)> {
    let mut commands: Vec<_> = profile
        .files
        .iter()
        .flat_map(|file| file.commands.iter().map(move |command| (file, command)))
        .collect();
    commands.sort_by(|(af, a), (bf, b)| {
        b.elapsed_ns
            .cmp(&a.elapsed_ns)
            .then_with(|| af.path.cmp(&bf.path))
            .then_with(|| a.key.cmp(&b.key))
    });
    commands
}

fn sum(mut values: impl Iterator<Item = u64>) -> Result<u64> {
    values
        .try_fold(0u64, |total, value| total.checked_add(value))
        .ok_or_else(|| Error::target("profile totals exceed 64-bit integers"))
}

fn short_kind(kind: &str) -> &str {
    kind.rsplit('.').next().unwrap_or(kind)
}

fn format_range(range: &Range) -> String {
    format!(
        "{}:{}-{}:{}",
        range.start.line, range.start.column, range.end.line, range.end.column
    )
}

fn percent(share: f64) -> String {
    format!("{:.1}%", share * 100.0)
}

fn seconds(ns: u64) -> String {
    format!("{:.2}", ns as f64 / 1e9)
}

fn count(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("1 {noun}")
    } else {
        format!("{n} {noun}s")
    }
}

fn describe(entry: &Inside) -> String {
    match (&entry.kind, &entry.range) {
        (Some(kind), Some(range)) => format!(
            "{} {} x{} {}",
            short_kind(kind),
            format_range(range),
            entry.count,
            percent(entry.share)
        ),
        _ => format!("{} x{} {}", entry.class, entry.count, percent(entry.share)),
    }
}

fn emit(listing: &Listing, output: Output) -> Result<()> {
    let mut stdout = std::io::stdout().lock();
    if matches!(output, Output::Json) {
        writeln!(stdout, "{}", serde_json::to_string_pretty(listing)?)?;
        return Ok(());
    }
    let steps = listing.detail == "steps";
    let table = matches!(output, Output::Table);
    let mut rows = Vec::new();
    if table {
        writeln!(
            stdout,
            "{} s in {} and {}; the {} listed take {}",
            seconds(listing.elapsed_ns),
            count(listing.commands, "command"),
            count(listing.files, "file"),
            listing.hotspots.len(),
            percent(listing.listed_share)
        )?;
        let mut header = vec!["Seconds", "Share", "Location", "Declaration", "Heartbeats"];
        if steps {
            header.push("Inside");
        }
        rows.push(header.into_iter().map(String::from).collect::<Vec<_>>());
    }
    for hotspot in &listing.hotspots {
        let mut fields = vec![
            seconds(hotspot.elapsed_ns),
            percent(hotspot.share),
            format!("{}:{}", hotspot.path, hotspot.range.start.line),
            hotspot.name.clone(),
            hotspot.heartbeats.to_string(),
        ];
        if let Some(traced) = &hotspot.traced {
            fields.push(traced.inside.first().map_or_else(|| "-".into(), describe));
        }
        rows.push(fields);
    }
    if !table {
        for row in &rows {
            writeln!(stdout, "{}", row.join("\t"))?;
        }
        return Ok(());
    }
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    let widths: Vec<usize> = (0..columns)
        .map(|column| {
            rows.iter()
                .filter_map(|row| row.get(column))
                .map(|cell| cell.chars().count())
                .max()
                .unwrap_or(0)
        })
        .collect();
    for row in &rows {
        let last = row.len() - 1;
        let cells: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(column, cell)| {
                if RIGHT_ALIGNED.contains(&column) {
                    format!("{cell:>width$}", width = widths[column])
                } else if column == last {
                    cell.clone()
                } else {
                    format!("{cell:<width$}", width = widths[column])
                }
            })
            .collect();
        writeln!(stdout, "{}", cells.join("  "))?;
    }
    Ok(())
}

fn trace(
    options: &Hotspots,
    project_dir: &Path,
    listed: &[(&File, &Command)],
) -> Result<BTreeMap<(String, u64, u64), Command>> {
    let mut targets = Targets::new();
    for (file, command) in listed {
        targets
            .entry(file.path.clone())
            .or_default()
            .push((command.range.start.line, command.range.start.column));
    }
    let record = Record {
        project_dir: project_dir.to_path_buf(),
        paths: targets.keys().map(PathBuf::from).collect(),
        output_file: None,
        jobs: options.jobs.unwrap_or(1),
        detail: Detail::Steps,
        step_threshold_ns: TRACED_THRESHOLD_NS,
        capture_threshold_ms: TRACED_THRESHOLD_NS / 1_000_000,
    };
    let profile = project::record(&record, Some(&targets))?;
    let mut traced = BTreeMap::new();
    for file in profile.files {
        for command in file.commands {
            traced.insert(
                (
                    file.path.clone(),
                    command.range.start.line,
                    command.range.start.column,
                ),
                command,
            );
        }
    }
    Ok(traced)
}

pub fn run(options: Hotspots, output: Output) -> Result<u8> {
    let (profile, detail) = match (&options.project_dir, &options.profile) {
        (Some(project_dir), None) => {
            let record = Record {
                project_dir: project_dir.clone(),
                paths: options.paths.clone(),
                output_file: None,
                jobs: options.jobs.unwrap_or(1),
                detail: Detail::None,
                step_threshold_ns: STEP_THRESHOLD_NS,
                capture_threshold_ms: 0,
            };
            (
                project::record(&record, None)?,
                options.detail.unwrap_or(Detail::Steps),
            )
        }
        (None, Some(input)) => {
            if options.detail == Some(Detail::Steps) {
                return Err(Error::parameter(
                    "--detail steps measures the project again; use --project-dir",
                ));
            }
            if options.jobs.is_some() || !options.paths.is_empty() {
                return Err(Error::parameter(
                    "--jobs and --paths apply to --project-dir",
                ));
            }
            let profile = crate::model::read(input, "--profile")?;
            profile.validate("--profile")?;
            if profile.conditions.detail != "none" {
                return Err(Error::target(
                    "--profile: list-hotspots ranks untraced elapsed times; use a profile recorded with --detail none",
                ));
            }
            (profile, Detail::None)
        }
        _ => {
            return Err(Error::parameter(
                "use exactly one of --project-dir and --profile",
            ));
        }
    };
    let ranked = ranked(&profile);
    let total = sum(ranked.iter().map(|(_, command)| command.elapsed_ns))?;
    let listed: Vec<_> = ranked.into_iter().take(options.limit).collect();
    let listed_total = sum(listed.iter().map(|(_, command)| command.elapsed_ns))?;
    let files_with_errors = profile.files.iter().filter(|file| file.has_errors).count();
    let mut failed = files_with_errors > 0;
    let mut traced = match (detail, &options.project_dir) {
        (Detail::Steps, Some(project_dir)) if !listed.is_empty() => {
            Some(trace(&options, project_dir, &listed)?)
        }
        _ => None,
    };
    let mut hotspots = Vec::new();
    for (file, command) in &listed {
        let traced = match &mut traced {
            Some(traced) => {
                let start = (
                    file.path.clone(),
                    command.range.start.line,
                    command.range.start.column,
                );
                let measured = traced.remove(&start).ok_or_else(|| {
                    Error::target(format!(
                        "traced measurement found no command at {}:{}:{}",
                        file.path, command.range.start.line, command.range.start.column
                    ))
                })?;
                if measured.key != command.key {
                    return Err(Error::target(format!(
                        "source changed during measurement: {} has Key {:?} instead of {:?}",
                        file.path, measured.key, command.key
                    )));
                }
                if measured.has_errors {
                    eprintln!(
                        "leanwake: traced measurement of {}:{} {} reported errors; its breakdown is incomplete",
                        file.path, command.range.start.line, command.key
                    );
                    failed = true;
                }
                let steps = measured
                    .steps
                    .as_deref()
                    .ok_or_else(|| Error::internal("traced command has no Steps"))?;
                Some(Traced {
                    elapsed_ns: measured.elapsed_ns,
                    has_errors: measured.has_errors,
                    inside: inside(steps, measured.elapsed_ns),
                })
            }
            None => None,
        };
        hotspots.push(Hotspot {
            path: file.path.clone(),
            module: file.module.clone(),
            key: command.key.clone(),
            name: command
                .names
                .first()
                .cloned()
                .unwrap_or_else(|| command.kind.clone()),
            kind: command.kind.clone(),
            range: command.range.clone(),
            elapsed_ns: command.elapsed_ns,
            share: share(command.elapsed_ns, total),
            heartbeats: command.heartbeats,
            has_errors: command.has_errors,
            traced,
        });
    }
    if files_with_errors > 0 {
        eprintln!("leanwake: {files_with_errors} files have errors");
    }
    let listing = Listing {
        detail: detail.as_str(),
        elapsed_ns: total,
        commands: profile.files.iter().map(|file| file.commands.len()).sum(),
        files: profile.files.len(),
        files_with_errors,
        listed_elapsed_ns: listed_total,
        listed_share: share(listed_total, total),
        hotspots,
    };
    emit(&listing, output)?;
    Ok(if failed { 254 } else { 0 })
}
