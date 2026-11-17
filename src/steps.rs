use crate::cli::Output;
use crate::error::{Error, Result};
use crate::model::Profile;
use serde_json::json;
use std::io::Write;

pub fn run(
    profile: &Profile,
    path: &str,
    key: &str,
    limit: Option<usize>,
    output: Output,
) -> Result<u8> {
    if profile.conditions.detail != "steps" {
        return Err(Error::target("list-steps requires Conditions.Detail=steps"));
    }
    let mut matches = profile
        .files
        .iter()
        .filter(|file| file.path == path)
        .flat_map(|file| file.commands.iter().filter(|command| command.key == key));
    let command = matches
        .next()
        .ok_or_else(|| Error::target(format!("no command with Path {path:?}, Key {key:?}")))?;
    if matches.next().is_some() {
        return Err(Error::target(format!(
            "duplicate Path {path:?}, Key {key:?}"
        )));
    }
    let steps = command
        .steps
        .as_ref()
        .ok_or_else(|| Error::target("command has no Steps"))?;
    let mut pending: Vec<_> = steps.iter().rev().map(|step| (step, 0usize)).collect();
    let mut rows = Vec::new();
    while let Some((step, depth)) = pending.pop() {
        rows.push((step, depth));
        pending.extend(step.children.iter().rev().map(|child| (child, depth + 1)));
    }
    rows.sort_by_key(|(step, _)| std::cmp::Reverse(step.heartbeats));
    if let Some(limit) = limit {
        rows.truncate(limit);
    }
    let mut stdout = std::io::stdout().lock();
    match output {
        Output::Json => {
            let rows: Vec<_> = rows
                .iter()
                .map(|(step, depth)| {
                    json!({
                        "Class": step.class, "Kind": step.kind, "Range": step.range,
                        "Heartbeats": step.heartbeats, "Depth": depth,
                    })
                })
                .collect();
            writeln!(
                stdout,
                "{}",
                serde_json::to_string_pretty(&json!({
                    "Path": path, "Key": key, "Steps": rows,
                }))?
            )?;
        }
        Output::Text | Output::Table => {
            if matches!(output, Output::Table) {
                writeln!(stdout, "Heartbeats\tDepth\tRange\tClass\tKind")?;
            }
            for (step, depth) in rows {
                let range = step
                    .range
                    .as_ref()
                    .map(|range| {
                        format!(
                            "{}:{}-{}:{}",
                            range.start.line, range.start.column, range.end.line, range.end.column,
                        )
                    })
                    .unwrap_or_else(|| "-".into());
                writeln!(
                    stdout,
                    "{}\t{}\t{}\t{}\t{}",
                    step.heartbeats,
                    depth,
                    range,
                    step.class,
                    step.kind.as_deref().unwrap_or("-")
                )?;
            }
        }
    }
    Ok(0)
}
