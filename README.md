# Leanwake

Leanwake tells you, with one command, which proofs in a Lean 4.35.0 Lake project take the most time and which tactic inside each of them uses that time. It ranks declarations by elaboration time measured without tracing, then elaborates only the listed declarations again under Lean's trace profiler to break their time down by tactic. The same recordings can be saved and compared to detect heartbeat regressions in CI; the comparison rules are defined and proved in Lean and generated into Rust with [leanfran](https://github.com/conao3/lean-leanfran).

## Installation

With [Nix](https://nixos.org/download/) and flakes enabled:

```sh
nix run github:conao3/rust-leanwake -- help
```

The package supplies the Rust CLI, process supervisor, native Lean adapter, pinned Lean/Lake toolchain, C compiler and Git. Compiler and Git paths are appended to PATH so caller-provided tools take precedence. To build a local checkout:

```sh
nix build
./result/bin/leanwake help
```

## Find slow proofs

Run `list-hotspots` in a Lake project. The example uses `tests/fixtures/hotspots` from this repository, where `slow` checks `(List.range 8000).length = 8000` with `decide +kernel`:

```sh
nix run github:conao3/rust-leanwake -- profile list-hotspots \
  --project-dir tests/fixtures/hotspots --jobs 4 --output table
```

```text
0.79 s in 6 commands and 3 files; the 6 listed take 100.0%
Seconds  Share  Location  Declaration  Heartbeats  Inside
0.66  83.1%  Hot/Slow.lean:4  slow  3132843  decide 6:4-6:18 x1 98.5%
0.06  7.2%  Hot/Slow.lean:1  small  18007  simp 2:2-2:6 x1 2.3%
0.05  6.9%  Hot/Quick.lean:1  double  18804  addDecl x1 80.2%
0.01  1.8%  Hot/Slow.lean:9  termMode  85105  Meta.whnf x161 41.5%
0.00  0.5%  Hot/Quick.lean:5  add_zero'  17293  simp 6:2-6:6 x1 23.1%
0.00  0.4%  Hot/Quick.lean:3  double_zero  8408  Elab.command x1 44.3%
```

The first line gives the total elaboration time of all measured commands, the numbers of commands and files, and the share of the total taken by the listed rows. Each row is one top-level command in descending elapsed time: seconds, share of the total, file and start line, declaration name (the first of `Names`, or the command kind for commands that introduce no named declaration), raw heartbeats, and the largest entry of the breakdown. `decide 6:4-6:18 x1 98.5%` means that the `decide` tactic at line 6, columns 4 to 18, ran once and took 98.5% of the declaration's traced time. Lines are one-based, columns zero-based, and end positions exclusive.

The breakdown groups the innermost tactic nodes that carry a source range by tactic kind and range, and sums their time; `x4` means the tactic node at that position ran four times, for example under `<;>` or `all_goals`. When a chain of nodes shares one range, the row names the outermost tactic that is not a tactic sequence. A declaration without tactic nodes, such as a term-mode proof or a definition, shows the trace classes with the most self time (node time minus the time of its retained children) instead, like `Meta.whnf` and `addDecl` above. Breakdown shares are relative to the declaration's time in the traced pass and never add up to more than 100%; the remainder is spent outside the listed tactics, for example in the kernel check of the finished proof.

The default is `--limit 10`. JSON, the default output, carries the full data with PascalCase keys: `ElapsedNs`, `Commands`, `Files`, `FilesWithErrors`, `ListedElapsedNs`, `ListedShare` and `Hotspots`. Each hotspot has `Path`, `Module`, `Key`, `Name`, `Kind`, `Range`, `ElapsedNs`, `Share`, `Heartbeats` and `HasErrors`, and with the breakdown a `Traced` object with the traced `ElapsedNs`, `HasErrors` and every `Inside` entry (`Class`, `Kind`, `Range`, `Count`, `ElapsedNs`, `Share`). This excerpt is the hotspot of `--limit 1` on the same fixture, with the other two `Inside` entries omitted:

```json
{
  "Path": "Hot/Slow.lean",
  "Module": "Hot.Slow",
  "Key": "slow",
  "Name": "slow",
  "Kind": "Lean.Parser.Command.declaration",
  "Range": {
    "Start": { "Line": 4, "Column": 0 },
    "End": { "Line": 7, "Column": 8 }
  },
  "ElapsedNs": 650165304,
  "Share": 0.8829720873961836,
  "Heartbeats": 3132843,
  "HasErrors": false,
  "Traced": {
    "ElapsedNs": 558910421,
    "HasErrors": false,
    "Inside": [
      {
        "Class": "Leanwake.tactic",
        "Kind": "Lean.Parser.Tactic.decide",
        "Range": {
          "Start": { "Line": 6, "Column": 4 },
          "End": { "Line": 6, "Column": 18 }
        },
        "Count": 1,
        "ElapsedNs": 552499982,
        "Share": 0.9885304715046636
      }
    ]
  }
}
```

`list-hotspots` measures in two passes. The first pass elaborates every selected file without tracing; ranking, seconds and shares come only from it. Tracing makes elaboration several times slower and changes the order: when the file containing lean-ritsuryo's slowest theorem was recorded alternately, the theorem took 9.3–9.4 s without tracing, 29.3–29.6 s with every trace node captured and 18.1–18.6 s with a 1 ms capture threshold. Heartbeat order differs from time order (a `decide +kernel` theorem is second by time and fifth by heartbeats). The second pass elaborates again only the files that contain listed declarations, stops after the last listed command of each file, and enables the trace profiler only for the listed commands, in time mode with a 1 ms capture threshold; tactic nodes are always kept. With the same commit of lean-ritsuryo (54 files, 893 commands, `--jobs 4`), the first pass alone (`--detail none`) took 47.5 s of wall-clock time and the whole command 72.8–85.3 s. These are observations on a shared machine, not guarantees.

Options:

- `--project-dir DIR` measures now. `--paths PATH ...` and `--jobs N` select files and parallel workers as in `record-profile`.
- `--profile file://profile.json` ranks an existing `--detail none` recording without measuring. It prints the ranking that measuring would print for that recording, without the breakdown.
- `--detail none|steps` turns the breakdown off or on. It defaults to `steps` with `--project-dir`; `--profile` accepts only `none`.

Give exactly one of `--project-dir` and `--profile`; otherwise the exit code is 252. Files with errors are reported on standard error, counted in `FilesWithErrors` and make the exit code 254, as does an error that appears only in the traced pass (tracing raises heartbeats, so a proof close to `maxHeartbeats` can fail there); the listing is still printed.

## Record and inspect

`record-profile` writes the untraced measurement as a JSON profile that `list-hotspots --profile`, `list-commands`, `list-files` and `compare-profiles` read. The examples below use `tests/fixtures/steps`.

```sh
nix run github:conao3/rust-leanwake -- profile record-profile \
  --project-dir tests/fixtures/steps --jobs 4 --output-file base.json
nix run github:conao3/rust-leanwake -- profile list-hotspots \
  --profile file://base.json --limit 3 --output table
nix run github:conao3/rust-leanwake -- profile list-commands \
  --profile file://base.json --limit 3 --output table
```

```text
0.22 s in 12 commands and 1 file; the 3 listed take 89.0%
Seconds  Share  Location  Declaration  Heartbeats
0.16  69.2%  Steps.lean:3  repeated  34230
0.04  16.6%  Steps.lean:22  _aux_Steps___elabRules_tacticLocalClose_1  421099
0.01  3.3%  Steps.lean:8  heavy  50440
```

```text
ElapsedNs	Heartbeats	Path	Key	HasErrors
155421449	34230	Steps.lean	repeated	false
37321254	421099	Steps.lean	_aux_Steps___elabRules_tacticLocalClose_1	false
7383216	50440	Steps.lean	heavy	false
```

`list-commands` lists commands in descending elapsed time with their heartbeats; its JSON output carries each command's full record. By default, recording selects Lake's default target modules and their transitive imports inside the project. Use `--paths File.lean Directory` to choose files and directory trees relative to the project root. Hidden paths and symbolic links are excluded. Dependencies are prepared serially, then up to `--jobs` files are measured concurrently; the default is one worker.

```sh
nix run github:conao3/rust-leanwake -- profile list-files \
  --profile file://base.json --output table
```

```text
Path	Module	Commands	Heartbeats	ElapsedNs	HasErrors
Steps.lean	Steps	12	708097	224743642	false
```

`--output-file` writes a JSON profile and leaves standard output empty. Without it, the profile is written to standard output; `--output text|table` selects a file summary instead. An error in one source is recorded with `HasErrors`; independent files continue. A process supervisor stops each subprocess group after completion or failure and when the host exits, including SIGINT, SIGTERM and SIGKILL. Processes deliberately detached into a separate process group are outside this lifecycle.

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
Heartbeats: 708097 -> 708097; thresholds: > 1000 heartbeats and > 1%
Status	BaseHeartbeats	HeadHeartbeats	DeltaHeartbeats	Path	Key
```

Changing the fixture's `heavy` theorem from `(List.range 100).length = 100` to `(List.range 300).length = 300` and recording again produces the following comparison, with exit code 254:

```text
Regressed: 1  Improved: 0  Unchanged: 11  Added: 0  Removed: 0
Heartbeats: 708097 -> 785298; thresholds: > 1000 heartbeats and > 1%
Status	BaseHeartbeats	HeadHeartbeats	DeltaHeartbeats	Path	Key
regressed	50440	127641	77201	Steps.lean	heavy
```

Commands are matched by `(Path, Key)` and classified as `regressed`, `improved`, `unchanged`, `added` or `removed`. Comparison uses heartbeats, which are far more repeatable than elapsed time. A regression must satisfy both strict inequalities:

```text
head - base > absolute-threshold-heartbeats
(head - base) * 100 > base * relative-threshold-percent
```

Defaults are **1,000 heartbeats and 1%**. Improvement uses the same rule with base and head exchanged. Thresholds are nonnegative 64-bit integers; intermediate products and totals use arbitrary precision natural numbers. Equality at either threshold is unchanged. Added or removed commands alone do not fail CI.

JSON output contains `Summary`, `Thresholds` and `Commands`. Unchanged rows are omitted; regressions come first, sorted by descending heartbeat increase. Missing counts are null, with zero used for their delta. The displayed range comes from head, or base for removals. Differing toolchains or conditions, unsupported schema versions, errored profiles and duplicate `(Path, Key)` identities are rejected before comparison results are emitted. Detail steps comparisons use command totals, not sums of trace nodes.

Matching groups commands by path before calling the Lean rules. Its worst-case work is the sum of each file's squared command count, rather than the square of the entire profile's count. Aggregation uses a tail-recursive fold. In a synthetic release-build benchmark with reversed file and command orders, 100 files × 500 commands took 0.37 s and 91 MiB peak RSS; 200 files × 500 commands took 0.72 s and 178 MiB. These are observations on one machine, not runtime guarantees. A single file with very many commands still has quadratic matching cost. The integration suite also compares one file containing 20,000 commands in reverse order on the standard process stack.

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
      - run: ./leanwake/bin/leanwake profile list-hotspots --profile file://head.json --output table
      - run: ./leanwake/bin/leanwake profile compare-profiles --base-profile file://base.json --head-profile file://head.json --output table
```

Generate an [Actions lockfile](https://github.com/github/gh-actions-lock) for the workflow to pin its action tags to commits. The CLI returns 254 if a regression is found, causing the comparison step to fail.

## Inspect steps

`record-profile --detail steps` records the whole trace tree of every command:

```sh
nix run github:conao3/rust-leanwake -- profile record-profile \
  --project-dir tests/fixtures/steps --detail steps --output-file steps.json
nix run github:conao3/rust-leanwake -- profile list-steps \
  --profile file://steps.json --path Steps.lean --key repeated --limit 12 --output table
```

```text
ElapsedNs	Depth	Range	Class	Kind
234794706	0	3:0-6:28	Elab.command	-
228961345	1	-	Elab.def.processPreDef	-
228768432	2	-	addDecl	-
3230632	1	-	Elab.definition.header	-
2756877	2	-	Elab.step	-
2720898	3	-	Elab.step	-
2671615	4	-	Elab.app.args	-
1834465	5	-	Elab.step	-
1821430	6	-	Elab.step	-
1798013	7	-	Elab.step	-
1785936	1	-	Elab.definition.value	-
1596162	2	-	Elab.step	-
```

Each command's `Steps` is a trace forest. Nodes contain `Class`, `ElapsedNs`, `Range` and `Children`; tactic nodes also contain `Kind`, the Lean syntax kind. Capture uses `trace.profiler.useHeartbeats=false`, so node times come from Lean's monotonic clock. `Leanwake.tactic` nodes preserve syntax positions by wrapping tactic elaborators, including newly registered elaborators in the measured file. Two occurrences of the same tactic on different lines remain distinct. Macro expansion may produce several nested nodes at the same source range. Nodes without retained source syntax have a null range.

`--step-threshold-ns N` keeps nodes of at least N nanoseconds; the default is **100,000** (0.1 ms). `Leanwake.tactic` nodes are always kept. Retained children of omitted nodes are promoted to the nearest retained ancestor. Capture uses a zero threshold and filtering happens after measurement, so changing the output threshold does not change the measured command cost. `Depth` in `list-steps` is the depth of the retained tree, with roots at zero. Equal-cost nodes retain traversal order.

Trace costs include their children. Adding all nodes double-counts nested work; use the command's `Heartbeats` for comparison. Trace collection adds elaboration work, so `Detail=none` and `Detail=steps` recordings cannot be compared, and `list-hotspots --profile` accepts only `Detail=none`. `list-steps` returns 254 for detail none or an unknown path/key.

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
| leanassay / default | 8 | 133 | 0 | 14.1–17.7 s | 23.0 s |
| leanassay / all | 30 | 253 | 137 | 199.4–248.2 s | 147.5 s |

All files had `HasErrors=false`. Small remaining differences are preserved, rather than rounded away. The deterministic integration fixture matches exactly; this is not a guarantee for arbitrary Lean metaprograms. Lean's lazy realization and kernel replay use additional internal mechanisms beyond the top-level async switch, and the precise source of the remaining variation is not isolated. Another pair of leanassay / all recordings differed by at most 183. The default absolute threshold of 1,000 exceeds every observed difference, while the relative threshold also requires a change above 1%.

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
| `summary_acc_counts`, `summary_counts` | Improved, unchanged, added and removed counts equal their initial counts plus the numbers of rows with those statuses. |
| `summary_acc_heartbeats`, `summary_heartbeats` | Base and head totals equal their initial totals plus the heartbeat sums of the respective present entries. |
| `failure_iff_regression` | Failure holds exactly when a regression exists. |

`make check` checks 22 theorems and audits their axioms, accepting only Lean's standard `propext`, `Classical.choice` and `Quot.sound`; regenerates Rust and checks byte-for-byte agreement; and verifies that deliberately wrong classification and dropped-file implementations fail proof checking. Proofs cover the comparison definitions, while integration tests exercise file selection, native measurement and the host boundary.

## Profile format

The JSON schema is version 2. This is an excerpt from an actual recording, with other commands omitted:

```json
{
  "SchemaVersion": 2,
  "Tool": {
    "Name": "leanwake",
    "Version": "0.2.0"
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
      "SourceSha256": "6adf6d5b41d62942741726bc8dd0950e618ba8b07a88689cdb225aa65262a354",
      "HasErrors": false,
      "ImportElapsedNs": 839380942,
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
          "Heartbeats": 50440,
          "ElapsedNs": 7383216,
          "HasErrors": false
        }
      ]
    }
  ]
}
```

With `Detail=steps`, every command additionally has `Steps: [...]`, whose nodes carry `ElapsedNs`; with `Detail=none`, that field is absent. Profiles with another `SchemaVersion` are rejected with exit code 254 and a message asking to record again. Unknown fields and inconsistent detail/step fields are rejected. All CLI JSON field names use PascalCase. File-valued parameters use `file://path`, and `file:///dev/stdin` reads standard input. Every command accepts `--output json|text|table`, `--debug`, `--no-debug`, `--version` and `help` at each command level. Global options can appear before or after the command names and `help`; an option value equal to `help` remains a value.

## Exit codes

| Code | Meaning |
| ---: | --- |
| 0 | Successful listing or recording, or a comparison without regressions. |
| 252 | Invalid command syntax or parameter, including malformed input JSON and `list-hotspots` without exactly one of `--project-dir` and `--profile`. |
| 254 | Project/source error, incompatible or invalid profile, unknown step selection, or detected regression. |
| 255 | Host I/O failure, missing runtime configuration, or invalid adapter protocol. |

## Limitations

- The project must declare `leanprover/lean4:v4.35.0`. Different toolchain recordings cannot be compared.
- Lake's module setup, options, plugins and `-Dname=value` arguments are supported. Other `weakLeanArgs` or `moreLeanArgs` are rejected explicitly.
- Measurements are top-level command costs. A command introducing multiple declarations has one cost and a list of names; it is not split into individual kernel declarations.
- Elapsed time depends on machine load; the shares and the order of clearly separated declarations are more stable than the seconds. Declarations with similar times can change places between runs.
- The breakdown covers time inside tactic nodes, or the self time of trace classes for declarations without tactics. Work outside them, such as the kernel check of a finished tactic proof, appears only as the part of the share that the listed entries do not cover.
- Trace locations depend on retained syntax. Generated tactic syntax can share its macro call's range, and many internal trace classes have no position.
- Very detailed recordings can require substantial memory and disk space. The output threshold reduces retained JSON, but all trace capture still runs.
- Time-based regression gates, graphs, cross-toolchain comparison and Mathlib-scale profiling are outside this release's validated scope.
- Linux is exercised in the integration tests. The flake defines an Apple Silicon package, but macOS execution has not been validated.

## Related tools

- [Lean `trace.profiler`](https://github.com/leanprover/lean4/blob/v4.35.0/src/Lean/Util/Trace.lean#L175-L235): built-in nested tracing with time or heartbeat measurements. Leanwake uses it for the breakdown.
- [lean-source-profiler](https://github.com/Vilin97/lean-source-profiler): source-level timing of files, declarations, tactics and terms, with VS Code overlays and a repository viewer.
- [leanprof](https://github.com/unbalancedparentheses/leanprof): runtime and elaboration profiling with file, declaration, term and tactic time and heartbeats, source attribution, and CI comparisons of declaration heartbeats.
- [lakeprof](https://github.com/Kha/lakeprof): Lake build job timing and critical-path analysis.
- [Radar](https://github.com/leanprover/radar): performance measurements across Lean revisions.

lean-source-profiler and leanprof also measure declaration and tactic time. Leanwake takes its ranking and seconds from an untraced measurement and traces only the declarations it lists, so the ranking is not distorted by tracing cost. Its comparison rules are proved in Lean, and its recordings carry their measurement conditions (toolchain, synchronous elaboration, detail), so comparing recordings made under different conditions fails instead of producing results.

## Development

`make check` runs leanassay on the generated comparison functions to check that
their theorems reject broken implementations; holes and audit warnings fail the check.

```sh
nix develop --command make check
nix build
git config core.hooksPath .githooks
```

`make check` builds both components, checks formatting and Clippy, checks the Lean proofs and generated Rust, runs executable integration tests, and builds and tests the installed package. `nix fmt` formats Git-tracked files; stage new source files before checking them. `nix build` builds the packaged CLI and adapters with `LEANFRAN_OFFLINE=1` and runs integration tests inside the Nix sandbox. Installation tests invoke the actual wrapper with an empty PATH to compile a plugin and fetch a local Git dependency, then check that caller-provided compiler and Git commands take precedence. The pre-commit hook and GitHub Actions use `make check`. See [`AGENTS.md`](AGENTS.md) for component responsibilities and regeneration commands.

## License

[Apache-2.0](LICENSE).
