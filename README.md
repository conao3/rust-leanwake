# Leanwake

Leanwake records elaboration costs for commands and declarations in Lean 4.35.0 Lake projects, then compares two recordings to detect heartbeat regressions in CI. It records source ranges, declaration names, elapsed time and errors, and can include a trace tree with tactic locations. Comparison rules are defined and proved in Lean and generated into Rust with [leanfran](https://github.com/conao3/lean-leanfran).

## Installation

With [Nix](https://nixos.org/download/) and flakes enabled:

```sh
nix run github:conao3/rust-leanwake -- help
```

The package supplies the Rust CLI, native Lean adapter and pinned Lean/Lake toolchain. To build a local checkout:

```sh
nix build
./result/bin/leanwake help
```

## Record and inspect

The examples below use `tests/fixtures/steps` from this repository. Replace it with your Lake project directory.

```sh
nix run github:conao3/rust-leanwake -- profile record-profile \
  --project-dir tests/fixtures/steps --jobs 4 --output-file base.json
nix run github:conao3/rust-leanwake -- profile list-commands \
  --profile file://base.json --limit 3 --output table
```

```text
Heartbeats	ElapsedNs	Path	Key	HasErrors
421038	46718672	Steps.lean	_aux_Steps___elabRules_tacticLocalClose_1	false
84492	11920477	Steps.lean	simplifyMany	false
50430	8005983	Steps.lean	heavy	false
```

By default, recording selects Lake's default target modules and their transitive imports inside the project. Use `--paths File.lean Directory` to choose files and directory trees relative to the project root. Hidden paths and symbolic links are excluded. Dependencies are prepared serially, then up to `--jobs` files are measured concurrently; the default is one worker.

```sh
nix run github:conao3/rust-leanwake -- profile list-files \
  --profile file://base.json --output table
```

```text
Path	Module	Commands	Heartbeats	ElapsedNs	HasErrors
Steps.lean	Steps	12	707878	262923073	false
```

`--output-file` writes a JSON profile and leaves standard output empty. Without it, the profile is written to standard output; `--output text|table` selects a file summary instead. An error in one source is recorded with `HasErrors`; independent files continue.

## Compare

Record the same source as `head.json`, then compare:

```sh
nix run github:conao3/rust-leanwake -- profile record-profile \
  --project-dir tests/fixtures/steps --output-file head.json
nix run github:conao3/rust-leanwake -- profile compare-profiles \
  --base-profile file://base.json --head-profile file://head.json --output table
```

```text
Regressed: 0  Improved: 0  Unchanged: 12  Added: 0  Removed: 0
Heartbeats: 707878 -> 707878; thresholds: > 1000 heartbeats and > 1%
Status	BaseHeartbeats	HeadHeartbeats	DeltaHeartbeats	Path	Key
```

Changing the fixture's `heavy` theorem from `(List.range 100).length = 100` to `(List.range 300).length = 300` and recording again produces the following comparison, with exit code 254:

```text
Regressed: 1  Improved: 0  Unchanged: 11  Added: 0  Removed: 0
Heartbeats: 707878 -> 785078; thresholds: > 1000 heartbeats and > 1%
Status	BaseHeartbeats	HeadHeartbeats	DeltaHeartbeats	Path	Key
regressed	50430	127631	77201	Steps.lean	heavy
```

Commands are matched by `(Path, Key)` and classified as `regressed`, `improved`, `unchanged`, `added` or `removed`. A regression must satisfy both strict inequalities:

```text
head - base > absolute-threshold-heartbeats
(head - base) * 100 > base * relative-threshold-percent
```

Defaults are **1,000 heartbeats and 1%**. Improvement uses the same rule with base and head exchanged. Thresholds are nonnegative 64-bit integers; intermediate products and totals use arbitrary precision natural numbers. Equality at either threshold is unchanged. Added or removed commands alone do not fail CI.

JSON output contains `Summary`, `Thresholds` and `Commands`. Unchanged rows are omitted; regressions come first, sorted by descending heartbeat increase. Missing counts are null, with zero used for their delta. The displayed range comes from head, or base for removals. Differing schema versions, toolchains or conditions, errored profiles and duplicate `(Path, Key)` identities are rejected before comparison results are emitted. Detail steps comparisons use command totals, not sums of trace nodes.

Matching groups commands by path before calling the Lean rules. Its worst-case work is the sum of each file's squared command count, rather than the square of the entire profile's count. Aggregation uses a tail-recursive fold. In a synthetic release-build benchmark with reversed file and command orders, 100 files × 500 commands took 0.37 s and 91 MiB peak RSS; 200 files × 500 commands took 0.72 s and 178 MiB. These are observations on one machine, not runtime guarantees. A single file with very many commands still has quadratic matching cost. The integration suite also compares one file containing 20,000 commands in reverse order on the standard process stack.

## Inspect steps

```sh
nix run github:conao3/rust-leanwake -- profile record-profile \
  --project-dir tests/fixtures/steps --detail steps --output-file steps.json
nix run github:conao3/rust-leanwake -- profile list-steps \
  --profile file://steps.json --path Steps.lean --key repeated --limit 12 --output table
```

```text
Heartbeats	Depth	Range	Class	Kind
49227	0	3:0-6:28	Elab.command	-
31388	1	-	Elab.definition.header	-
30070	2	-	Elab.step	-
29930	3	-	Elab.step	-
29596	4	-	Elab.app.args	-
18245	5	-	Elab.step	-
18144	6	-	Elab.step	-
18025	7	-	Elab.step	-
14789	1	-	Elab.definition.value	-
14001	2	-	Elab.step	-
13921	3	4:2-6:28	Leanwake.tactic	Lean.Parser.Tactic.tacticSeq
13870	4	-	Elab.step	-
```

Each command's `Steps` is a trace forest. Nodes contain `Class`, `Heartbeats`, `Range` and `Children`; tactic nodes also contain `Kind`, the Lean syntax kind. `Leanwake.tactic` nodes preserve syntax positions by wrapping tactic elaborators, including newly registered elaborators in the measured file. Two occurrences of the same tactic on different lines remain distinct. Macro expansion may produce several nested nodes at the same source range. Nodes without retained source syntax have a null range.

`--step-threshold-heartbeats N` keeps nodes with at least N heartbeats; the default is **1,000**. Retained children of omitted nodes are promoted to the nearest retained ancestor. Capture always uses `trace.profiler.useHeartbeats=true` with a zero capture threshold; filtering happens after measurement, so changing the output threshold does not change the measured command cost. `Depth` in `list-steps` is the depth of the retained tree, with roots at zero. Equal-cost nodes retain traversal order.

Trace costs include their children. Adding all nodes double-counts nested work; use the command's `Heartbeats` for comparison. Trace collection adds elaboration work, so `Detail=none` and `Detail=steps` recordings cannot be compared. `list-steps` returns 254 for detail none or an unknown path/key.

For all 61 files and 967 commands in lean-ritsuryo, the development host and native adapter on four CPUs recorded these sizes and times (decimal MB):

| Detail / retained threshold | JSON size | Recording time |
| --- | ---: | ---: |
| none | 0.52 MB | 64.3 s |
| steps / 0 | 6,907.92 MB | 189.3 s |
| steps / 1,000 | 385.84 MB | 92.6–117.5 s |
| steps / 10,000 | 47.05 MB | 94.3 s |

Every recording had zero errored files and commands. The 1,000 default reduces the unfiltered JSON size by 94.4% and keeps 6,164 distinct tactic position/kind pairs; 10,000 keeps 4,632. It also keeps both small `simp` calls in the example. These elapsed times include concurrent machine load and are observations, not performance guarantees.

Two recordings at the default step threshold differed in 15 command counts, with a maximum difference of 6 raw heartbeats. Comparison classified all 967 commands as unchanged and exited successfully.

## CI for a Lean project

This pull request job records the exact base and head revisions with the same Leanwake installation. Both revisions must use Lean 4.35.0. Pin the Leanwake flake to a chosen revision when maintaining long-lived baselines.

```yaml
name: Elaboration costs
on:
  pull_request:
permissions:
  contents: read
jobs:
  profile:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7.0.1
        with:
          ref: ${{ github.event.pull_request.base.sha }}
          path: base
      - uses: actions/checkout@v7.0.1
        with:
          ref: ${{ github.event.pull_request.head.sha }}
          path: head
      - uses: cachix/install-nix-action@v31.11.5
      - run: nix build github:conao3/rust-leanwake --out-link leanwake
      - run: ./leanwake/bin/leanwake profile record-profile --project-dir base --jobs 2 --output-file base.json
      - run: ./leanwake/bin/leanwake profile record-profile --project-dir head --jobs 2 --output-file head.json
      - run: ./leanwake/bin/leanwake profile compare-profiles --base-profile file://base.json --head-profile file://head.json --output table
```

Generate an [Actions lockfile](https://github.com/github/gh-actions-lock) for the workflow to pin its action tags to commits. The CLI returns 254 if a regression is found, causing the comparison step to fail.

## Measurement and reproducibility

The native adapter imports a file's dependencies, parses each top-level command, and measures `Command.elabCommandTopLevel` between calls to `IO.getNumHeartbeats` and `IO.monoNanosNow`. Elaboration runs synchronously with `Elab.async=false`, including source-level `set_option` overrides and nested `elabCommand` calls from custom command elaborators. An unfinished registered elaboration task marks the command and file as errored, reports a diagnostic, and stops the file. Detached tasks created outside Lean's snapshot registry are outside the measurement contract. Each source file has its own adapter process. Module linters receive the parsed command sequence, and terminal diagnostics contribute to file errors. Imported and `weak.` options are reparsed using Lean's frontend before measurement overrides are applied. Command time excludes parsing, imports, name discovery, hashing and JSON serialization; import time is recorded separately.

Heartbeats count small allocations on the elaborating thread. They are neither elapsed time nor allocated bytes. One unit of Lean's `maxHeartbeats` corresponds to **1,000 raw heartbeats** in a profile. Project heartbeat limits remain in effect. Recording executes elaborators and `#eval` actions as compilation does.

`Names` lists newly introduced constants with declaration ranges, ordered by range start and then name. `Key` is the first name when present. Otherwise it is:

```text
<Path>::<last component of Kind>:<first 12 hex digits of source SHA-256>#<occurrence>
```

The hash covers the command syntax's UTF-8 source, excluding surrounding trivia. Occurrences are one-based within a file for identical kind and source. Paths are relative to the project root. Lines are one-based, columns zero-based, and end positions are exclusive.

Repeated recordings of unchanged real projects on four cores produced:

| Project / selection | Files | Commands | Maximum command heartbeat difference | Recording time | Lake build |
| --- | ---: | ---: | ---: | ---: | ---: |
| lean-ritsuryo / default | 54 | 893 | 8 | 43.1–43.5 s | 48.4 s |
| lean-ritsuryo / all | 61 | 967 | 15 | 45.8–61.7 s | 78.1 s |
| lean-assay / default | 8 | 133 | 0 | 14.1–17.7 s | 23.0 s |
| lean-assay / all | 30 | 253 | 137 | 199.4–248.2 s | 147.5 s |

All files had `HasErrors=false`. Small remaining differences are preserved, rather than rounded away. The deterministic integration fixture matches exactly; this is not a guarantee for arbitrary Lean metaprograms. Lean's lazy realization and kernel replay use additional internal mechanisms beyond the top-level async switch, and the precise source of the remaining variation is not isolated. The default absolute threshold of 1,000 exceeds the largest observed difference of 137, while the relative threshold also requires a change above 1%.

## Verified rules

The source is [`src/rules.rs`](src/rules.rs). `leanfran` extracts it into `lean/`, checks proofs and generates the committed Rust in `.leanfran/cache/`. The CLI calls that generated implementation for matching, classification, aggregation and failure.

| Theorems | Property |
| --- | --- |
| `grows_lt`, `grows_asymmetric` | Growth implies an increase and cannot hold in both directions. |
| `classify_regressed`, `classify_improved` | Classification agrees with the growth predicate. |
| `equal_unchanged` | Equal counts are unchanged. |
| `swap_classification` | Swapping inputs swaps regression and improvement. |
| `regression_monotone` | Increasing head preserves regression. |
| `grows_thresholds`, `regression_thresholds`, `improvement_thresholds` | Raising thresholds cannot introduce growth, regression or improvement. |
| `takeMatch_conservation` | Taking a match preserves all head entries. |
| `matching_base_once`, `matching_head_once` | Every input occurrence appears exactly once in the result. |
| `matching_files_base_once`, `matching_files_head_once` | File grouping preserves every base and head occurrence exactly once. |
| `summary_acc_regressed`, `summary_regressed` | The fold's regression total equals the initial count plus the number of regressed rows. |
| `failure_iff_regression` | Failure holds exactly when a regression exists. |

`make check` checks 18 theorems and audits their axioms, accepting only Lean's standard `propext`, `Classical.choice` and `Quot.sound`; regenerates Rust and checks byte-for-byte agreement; and verifies that deliberately wrong classification and dropped-file implementations fail proof checking. Proofs cover the comparison definitions, while integration tests exercise file selection, native measurement and the host boundary.

## Profile format

The JSON schema remains version 1. This is an excerpt from an actual recording, with other commands omitted:

```json
{
  "SchemaVersion": 1,
  "Tool": {
    "Name": "leanwake",
    "Version": "0.1.0"
  },
  "Toolchain": "Lean (version 4.35.0, Release)",
  "Conditions": {
    "Async": false,
    "Detail": "none"
  },
  "Files": [
    {
      "Path": "Steps.lean",
      "Module": "Steps",
      "SourceSha256": "64745bd140e3e359c7aeba6625d8f32e12e7a54fca6739292033d19576c44f19",
      "HasErrors": false,
      "ImportElapsedNs": 806632810,
      "Commands": [
        {
          "Key": "heavy",
          "Kind": "Lean.Parser.Command.declaration",
          "Names": [
            "heavy"
          ],
          "Range": {
            "Start": {
              "Line": 8,
              "Column": 0
            },
            "End": {
              "Line": 9,
              "Column": 16
            }
          },
          "Heartbeats": 50430,
          "ElapsedNs": 8005983,
          "HasErrors": false
        }
      ]
    }
  ]
}
```

With `Detail=steps`, every command additionally has `Steps: [...]`; with `Detail=none`, that field is absent. Unknown fields, unsupported schema versions and inconsistent detail/step fields are rejected. All CLI JSON field names use PascalCase. File-valued parameters use `file://path`, and `file:///dev/stdin` reads standard input. Every command accepts `--output json|text|table`, `--debug`, `--no-debug`, `--version` and `help` at each command level. Global options can appear before or after the command names and `help`; an option value equal to `help` remains a value.

## Exit codes

| Code | Meaning |
| ---: | --- |
| 0 | Successful recording/listing, or a comparison without regressions. |
| 252 | Invalid command syntax or parameter, including malformed input JSON. |
| 254 | Project/source error, incompatible or invalid profile, unknown step selection, or detected regression. |
| 255 | Host I/O failure, missing runtime configuration, or invalid adapter protocol. |

## Limitations

- The project must declare `leanprover/lean4:v4.35.0`. Different toolchain recordings cannot be compared.
- Lake's module setup, options, plugins and `-Dname=value` arguments are supported. Other `weakLeanArgs` or `moreLeanArgs` are rejected explicitly.
- Measurements are top-level command costs. A command introducing multiple declarations has one cost and a list of names; it is not split into individual kernel declarations.
- Trace locations depend on retained syntax. Generated tactic syntax can share its macro call's range, and many internal trace classes have no position.
- Very detailed recordings can require substantial memory and disk space. The output threshold reduces retained JSON, but all trace capture still runs.
- Elapsed time is informational. Git checkout orchestration, time-based regression gates, graphs, cross-toolchain comparison and Mathlib-scale profiling are outside this release's validated scope.
- Linux is exercised in the integration tests. The flake defines an Apple Silicon package, but macOS execution has not been validated.

## Related tools

- [Lean `trace.profiler`](https://github.com/leanprover/lean4/blob/v4.35.0/src/Lean/Util/Trace.lean): built-in nested tracing with time or heartbeat measurements.
- [lakeprof](https://github.com/Kha/lakeprof): Lake build job timing and critical-path analysis.
- [Radar](https://github.com/leanprover/radar): performance measurements across Lean revisions.
- [leanprof](https://github.com/unbalancedparentheses/leanprof): runtime and elaboration profiling with source attribution and CI comparisons.
- [lean-source-profiler](https://github.com/Vilin97/lean-source-profiler): source-level timing logs, VS Code overlays and a repository viewer.

## Development

```sh
nix develop --command make check
nix build
git config core.hooksPath .githooks
```

`make check` builds both components, checks formatting and Clippy, checks the Lean proofs and generated Rust, and runs executable integration tests. `nix build` builds the packaged CLI and adapters with `LEANFRAN_OFFLINE=1` and runs integration tests inside the Nix sandbox. The pre-commit hook and GitHub Actions use `make check`. See [`AGENTS.md`](AGENTS.md) for component responsibilities and regeneration commands.

## License

[Apache-2.0](LICENSE).
