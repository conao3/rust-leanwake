import argparse
import json
from pathlib import Path


def cell(value):
    return (
        str(value)
        .replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace("|", "&#124;")
        .replace("`", "&#96;")
        .replace("\r", "")
        .replace("\n", "<br>")
    )


def table(headers, rows):
    return [
        "| " + " | ".join(headers) + " |",
        "| " + " | ".join("---" for _ in headers) + " |",
        *("| " + " | ".join(cell(value) for value in row) + " |" for row in rows),
    ]


def source_range(value):
    if value is None:
        return "unknown"
    start, end = value["Start"], value["End"]
    return f"{start['Line']}:{start['Column']}-{end['Line']}:{end['Column']}"


def inside_key(entry):
    return (
        -entry["ElapsedNs"],
        entry["Kind"] or entry["Class"],
        source_range(entry["Range"]),
        entry["Class"],
    )


def render(hotspots, metadata, profile):
    listed = sorted(
        hotspots["Hotspots"],
        key=lambda entry: (-entry["ElapsedNs"], entry["Path"], entry["Key"]),
    )[:50]
    rows = []
    tactics = {}
    identities = set()
    for rank, entry in enumerate(listed, 1):
        identity = (entry["Path"], entry["Key"])
        if identity in identities:
            raise ValueError("duplicate hotspot identity: " + repr(identity))
        identities.add(identity)
        traced = entry.get("Traced")
        inside = traced["Inside"] if traced else []
        largest = min(inside, key=inside_key) if inside else None
        span = (
            f"{source_range(largest['Range'])} "
            if largest and largest["Range"] is not None
            else ""
        )
        summary = (
            f"{largest['Kind'] or largest['Class']} "
            f"{span}x{largest['Count']} "
            f"{largest['Share']:.1%}"
            if largest
            else "unavailable"
        )
        position = entry["Range"]
        line = position["Start"]["Line"] if position else "unknown"
        rows.append(
            [
                rank,
                f"{entry['ElapsedNs'] / 1e9:.6f}",
                f"{entry['Share']:.1%}",
                f"{entry['Path']}:{line}",
                entry["Name"] or entry["Kind"],
                entry["Heartbeats"],
                summary,
            ]
        )
        for item in inside:
            if item["Class"] != "Leanwake.tactic":
                continue
            kind = item["Kind"] or "unknown"
            aggregate = tactics.setdefault(
                kind, {"ElapsedNs": 0, "Count": 0, "Declarations": set()}
            )
            aggregate["ElapsedNs"] += item["ElapsedNs"]
            aggregate["Count"] += item["Count"]
            aggregate["Declarations"].add(identity)
    tactic_rows = [
        [
            kind,
            f"{data['ElapsedNs'] / 1e9:.6f}",
            data["ElapsedNs"],
            data["Count"],
            len(data["Declarations"]),
        ]
        for kind, data in sorted(
            tactics.items(), key=lambda pair: (-pair[1]["ElapsedNs"], pair[0])
        )
    ]
    error_rows = []
    for file in sorted(profile["Files"], key=lambda file: file["Path"]):
        if not file["HasErrors"]:
            continue
        commands = file.get("Commands")
        errors = [command for command in commands or [] if command["HasErrors"]]
        first = errors[0] if errors else None
        position = first.get("Range") if first else None
        location = f" (line {position['Start']['Line']})" if position else ""
        error_rows.append(
            [
                file["Path"],
                f"{first['Key']}{location}" if first else "unavailable",
                f"{len(errors)}/{len(commands)}"
                if commands is not None
                else "unavailable",
            ]
        )
    lines = [
        "# Mathlib v4.35.0 hotspots",
        "",
        (
            f"Setup: Mathlib v4.35.0 ({cell(metadata['MathlibCommit'])}); "
            f"toolchain: {cell(metadata['Toolchain'])}; "
            f"leanwake: {cell(metadata['LeanwakeCommit'])}; jobs: {metadata['Jobs']}; "
            f"CPU: {cell(metadata['CpuModel'])} (nproc: {metadata['Nproc']}); "
            f"wall time: {metadata['WallSeconds']:.2f} s; "
            f"total ElapsedNs: {hotspots['ElapsedNs']}; commands: {hotspots['Commands']}; "
            f"files: {hotspots['Files']}; FilesWithErrors: {hotspots['FilesWithErrors']}; "
            f"exit code: {metadata['ExitCode']}."
        ),
        "",
        (
            f"Recording: {metadata['RecordWallSeconds']:.2f} s, "
            f"exit code {metadata['RecordExitCode']}; "
            f"checkpoint tracing: {metadata['TraceWallSeconds']:.2f} s, "
            f"exit code {metadata['TraceExitCode']}."
        ),
        "",
        "## Files with errors",
        "",
        (
            "These files fail only under synchronous elaboration in Lean 4.35.0 "
            "(reproducible with `lean -DElab.async=false --setup ...`), so their commands "
            "after the failure are missing or errored; Lake builds them cleanly."
            if error_rows
            else "No files with errors in the untraced recording."
        ),
        "",
        "Counts are errored commands / recorded commands from profile.json, not diagnostic counts.",
        "",
        *table(
            ["Path", "First errored command", "Errored / recorded commands"], error_rows
        ),
        "",
        "## Top 50 declarations",
        "",
        (
            f"Showing {len(listed)} listed declarations. Ranking, seconds and share use the untraced pass. "
            "Inside shares use each declaration's traced elapsed time. "
            "Source lines are one-based, columns zero-based, and range ends exclusive."
        ),
        "",
        *table(
            [
                "Rank",
                "Seconds",
                "Share",
                "Path:line",
                "Declaration",
                "Heartbeats",
                "Top Inside entry",
            ],
            rows,
        ),
        "",
        "## Time-consuming tactics",
        "",
        (
            "This aggregation covers only the top-50 declarations' traced pass, not all of Mathlib. "
            "Only Inside entries with Class Leanwake.tactic contribute; occurrences sum Count, "
            "and declarations count distinct (Path, Key) pairs per tactic kind."
        ),
        "",
        *table(
            ["Tactic kind", "Seconds", "ElapsedNs", "Occurrences", "Declarations"],
            tactic_rows,
        ),
        "",
    ]
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(
        description="Render a deterministic Mathlib hotspot report"
    )
    default = Path(__file__).resolve().parent.parent / "results/mathlib-v4.35.0"
    parser.add_argument("--input-dir", type=Path, default=default)
    parser.add_argument("--output-dir", type=Path, default=default)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    target = args.output_dir / "README.md"
    try:
        hotspots = json.loads((args.input_dir / "hotspots.json").read_text())
        metadata = json.loads((args.input_dir / "metadata.json").read_text())
        profile = json.loads((args.input_dir / "profile.json").read_text())
        result = render(hotspots, metadata, profile).encode()
        if args.check:
            if target.read_bytes() != result:
                parser.exit(1, f"Report differs: {target}\n")
            print(f"Report matches: {target}")
        else:
            args.output_dir.mkdir(parents=True, exist_ok=True)
            target.write_bytes(result)
            print(f"Wrote report: {target}")
    except (OSError, ValueError, KeyError, TypeError) as error:
        parser.exit(1, f"Cannot generate report: {error}\n")


if __name__ == "__main__":
    main()
