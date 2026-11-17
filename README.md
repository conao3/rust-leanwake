# Leanwake

Leanwake records command elaboration costs for Lean 4.35.0 Lake projects. A Rust
host prepares dependencies and runs one native Lean adapter process per file.
Profiles contain raw heartbeats, elapsed nanoseconds, declaration names, source
ranges and errors. Elaboration runs synchronously; files can run in parallel.

```sh
nix build --option cores 4 --max-jobs 1
result/bin/leanwake profile record-profile --project-dir ./project --jobs 4 --output-file profile.json
result/bin/leanwake profile list-commands --profile file://profile.json --limit 10
result/bin/leanwake profile list-files --profile file://profile.json --output table
result/bin/leanwake profile compare-profiles --base-profile file://base.json --head-profile file://head.json
```

The default selection follows Lake's default targets and their transitive local
imports. Use `--paths File.lean Directory` to select source files relative to the
project root. Selection skips hidden entries and symbolic links, including
explicit paths through hidden or linked directories.
An explicit source outside a declared Lake target uses the package's Lean
options. The project must declare `leanprover/lean4:v4.35.0` in `lean-toolchain`.

`record-profile` writes JSON to `--output-file`, or to standard output when that
parameter is omitted. All commands accept `--output json|text|table` (default
`json`), `--debug`, `--no-debug` and `--version`. Each command level accepts
`help`. Parameter values beginning with `file://` are read from that file;
`file:///dev/stdin` reads standard input.

Exit codes are 0 for success, 252 for invalid syntax or parameters, 254 for
project errors (including any profile file with `HasErrors`), and 255 for host
or protocol failures. Import preparation failures produce an errored file with
an empty command list. Independent files continue to be measured.

Heartbeats count small allocations on the elaborating thread. `maxHeartbeats`
uses thousands of these units; Leanwake preserves the project's limit. Command
time excludes parsing, imports, name collection and JSON serialization. Import
time is recorded separately. Elaboration can execute the project's metaprograms
and `#eval` actions, just as compiling its source can.
Synchronous elaboration does not guarantee identical raw counters for every
Lean workload; small differences remain in the real-project measurements.

## Development

```sh
nix develop --command make check
nix build --option cores 4 --max-jobs 1
```

`make check` builds the native adapter and Rust host, checks formatting and
Clippy, and runs integration tests. The dev shell sets `LEAN_SYSROOT` and
`LEANWAKE_ADAPTER`; the distribution supplies both through wrappers. `native/`
contains the profiler and Lake configuration reader, and `src/` contains the
CLI, process management, recording and listing code.

`compare-profiles` matches `(Path, Key)` pairs. A regression exceeds both the
absolute threshold (default 1,000 heartbeats) and the relative threshold (default
1% of base). Improvement applies the same rule with base and head exchanged.
Set `--absolute-threshold-heartbeats N` and `--relative-threshold-percent N` to
change these strict thresholds. Both accept nonnegative 64-bit integers.

Comparison returns 254 when any command regresses, otherwise 0. Different schema
versions, toolchains or conditions, errored records, and duplicate identities also
return 254 with diagnostics. JSON includes `Summary`, `Thresholds` and changed
`Commands`; regressions come first in descending delta order. Added and removed
commands have null for the missing count and use zero for that side of the delta.
`Range` refers to head, or base for removals. Text and table output include totals
and a command table.

Classification, matching, aggregation and failure are defined and proved in Lean,
then generated into Rust with leanfran. `make check` checks the proofs, the generated
cache and a deliberately incorrect comparison definition, in addition to CLI tests.
This release records `Conditions.Detail = "none"`.
