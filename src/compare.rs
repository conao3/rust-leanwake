use crate::cli::{Compare, Output};
use crate::error::{Error, Result};
use crate::model::{Profile, Range};
use crate::rules::core::{self, Entry, Status};
use leanfran::Nat;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::Write;

type Ranges = BTreeMap<(String, String), Range>;

fn read(input: &str, parameter: &str) -> Result<Profile> {
    crate::json_io::read(input.as_bytes())
        .map_err(|e| Error::parameter(format!("{parameter}: {e}")))
}

fn validate(profile: Profile, side: &str) -> Result<(Vec<Entry>, Ranges)> {
    if profile.schema_version != 1
        || profile.conditions.r#async
        || !matches!(profile.conditions.detail.as_str(), "none" | "steps")
    {
        return Err(Error::target(format!(
            "{side}: unsupported SchemaVersion={} or Conditions={}",
            profile.schema_version,
            serde_json::to_string(&profile.conditions)?
        )));
    }
    profile.validate_steps()?;
    let mut entries = Vec::new();
    let mut ranges = BTreeMap::new();
    for file in profile.files {
        if file.has_errors {
            return Err(Error::target(format!(
                "{side}: HasErrors in file {:?}",
                file.path
            )));
        }
        for command in file.commands {
            if command.has_errors {
                return Err(Error::target(format!(
                    "{side}: HasErrors in file {:?}, Key {:?}",
                    file.path, command.key
                )));
            }
            let identity = (file.path.clone(), command.key.clone());
            if ranges.insert(identity, command.range).is_some() {
                return Err(Error::target(format!(
                    "{side}: duplicate Path {:?}, Key {:?}",
                    file.path, command.key
                )));
            }
            entries.push(Entry {
                path: file.path.clone(),
                key: command.key,
                heartbeats: command.heartbeats.into(),
            });
        }
    }
    Ok((entries, ranges))
}

fn number(n: &Nat) -> Result<u64> {
    u64::try_from(n).map_err(|_| Error::internal("comparison changed a command's heartbeat count"))
}

fn natural_json(n: &Nat) -> Result<Value> {
    Ok(serde_json::from_str(&n.to_string())?)
}

fn status(status: &Status) -> &'static str {
    match status {
        Status::Regressed => "regressed",
        Status::Improved => "improved",
        Status::Unchanged => "unchanged",
        Status::Added => "added",
        Status::Removed => "removed",
    }
}

fn rank(status: &Status) -> u8 {
    match status {
        Status::Regressed => 0,
        Status::Improved => 1,
        Status::Unchanged => 2,
        Status::Added => 3,
        Status::Removed => 4,
    }
}

struct DisplayRow {
    rank: u8,
    path: String,
    key: String,
    delta: i128,
    value: Value,
}

pub fn run(options: Compare, output: Output) -> Result<u8> {
    let base = read(&options.base_profile, "--base-profile")?;
    let head = read(&options.head_profile, "--head-profile")?;
    let metadata = |profile: &Profile| {
        json!({
            "SchemaVersion": profile.schema_version, "Toolchain": profile.toolchain,
            "Conditions": profile.conditions,
        })
    };
    let base_metadata = metadata(&base);
    let head_metadata = metadata(&head);
    let differences: Vec<_> = ["SchemaVersion", "Toolchain", "Conditions"]
        .into_iter()
        .filter(|field| base_metadata[*field] != head_metadata[*field])
        .map(|field| {
            format!(
                "{field}: base={}, head={}",
                base_metadata[field], head_metadata[field]
            )
        })
        .collect();
    if !differences.is_empty() {
        return Err(Error::target(format!(
            "incompatible profiles: {}",
            differences.join("; ")
        )));
    }
    let (base, base_ranges) = validate(base, "base")?;
    let (head, head_ranges) = validate(head, "head")?;
    let mut files: BTreeMap<String, (Vec<Entry>, Vec<Entry>)> = BTreeMap::new();
    for entry in base {
        files.entry(entry.path.clone()).or_default().0.push(entry);
    }
    for entry in head {
        files.entry(entry.path.clone()).or_default().1.push(entry);
    }
    let rows = core::match_files(
        files.into_values().collect(),
        options.absolute_threshold_heartbeats.into(),
        options.relative_threshold_percent.into(),
    );
    let summary = core::summarize(rows.clone());
    let failed = core::failed(summary.clone());
    let summary = json!({
        "Regressed": number(&summary.regressed)?,
        "Improved": number(&summary.improved)?,
        "Unchanged": number(&summary.unchanged)?,
        "Added": number(&summary.added)?,
        "Removed": number(&summary.removed)?,
        "BaseHeartbeats": natural_json(&summary.base_heartbeats)?,
        "HeadHeartbeats": natural_json(&summary.head_heartbeats)?,
    });
    let mut displayed = Vec::new();
    for row in rows {
        if row.status == Status::Unchanged {
            continue;
        }
        let (entry, ranges) = match (&row.head, &row.base) {
            (Some(entry), _) => (entry, &head_ranges),
            (None, Some(entry)) => (entry, &base_ranges),
            (None, None) => return Err(Error::internal("comparison row has no command")),
        };
        let base = row
            .base
            .as_ref()
            .map(|e| number(&e.heartbeats))
            .transpose()?;
        let head = row
            .head
            .as_ref()
            .map(|e| number(&e.heartbeats))
            .transpose()?;
        let delta = i128::from(head.unwrap_or(0)) - i128::from(base.unwrap_or(0));
        let range = ranges
            .get(&(entry.path.clone(), entry.key.clone()))
            .ok_or_else(|| Error::internal("comparison command has no source range"))?;
        displayed.push(DisplayRow {
            rank: rank(&row.status),
            path: entry.path.clone(),
            key: entry.key.clone(),
            delta,
            value: json!({
                "Path": entry.path,
                "Key": entry.key,
                "Status": status(&row.status),
                "BaseHeartbeats": base,
                "HeadHeartbeats": head,
                "DeltaHeartbeats": delta,
                "Range": range,
            }),
        });
    }
    displayed.sort_by(|a, b| {
        a.rank
            .cmp(&b.rank)
            .then_with(|| b.delta.cmp(&a.delta))
            .then_with(|| a.path.cmp(&b.path))
            .then_with(|| a.key.cmp(&b.key))
    });
    let commands: Vec<_> = displayed.into_iter().map(|row| row.value).collect();
    let thresholds = json!({
        "RelativeThresholdPercent": options.relative_threshold_percent,
        "AbsoluteThresholdHeartbeats": options.absolute_threshold_heartbeats,
    });
    emit(&summary, &thresholds, &commands, output)?;
    Ok(if failed { 254 } else { 0 })
}

fn emit(summary: &Value, thresholds: &Value, commands: &[Value], output: Output) -> Result<()> {
    let mut stdout = std::io::stdout().lock();
    match output {
        Output::Json => writeln!(
            stdout,
            "{}",
            serde_json::to_string_pretty(&json!({
                "Summary": summary, "Thresholds": thresholds, "Commands": commands,
            }))?
        )?,
        Output::Text | Output::Table => {
            writeln!(
                stdout,
                "Regressed: {}  Improved: {}  Unchanged: {}  Added: {}  Removed: {}",
                summary["Regressed"],
                summary["Improved"],
                summary["Unchanged"],
                summary["Added"],
                summary["Removed"]
            )?;
            writeln!(
                stdout,
                "Heartbeats: {} -> {}; thresholds: > {} heartbeats and > {}%",
                summary["BaseHeartbeats"],
                summary["HeadHeartbeats"],
                thresholds["AbsoluteThresholdHeartbeats"],
                thresholds["RelativeThresholdPercent"]
            )?;
            let fields = [
                "Status",
                "BaseHeartbeats",
                "HeadHeartbeats",
                "DeltaHeartbeats",
                "Path",
                "Key",
            ];
            writeln!(stdout, "{}", fields.join("\t"))?;
            for row in commands {
                let values: Vec<_> = fields
                    .iter()
                    .map(|field| match &row[*field] {
                        Value::String(text) => text.clone(),
                        Value::Null => "-".into(),
                        value => value.to_string(),
                    })
                    .collect();
                writeln!(stdout, "{}", values.join("\t"))?;
            }
        }
    }
    Ok(())
}
