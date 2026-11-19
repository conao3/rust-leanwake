# Leanwake development

Use Rust edition 2024, clap derive, serde, flake-parts and treefmt-nix. Lean and
Lake are fixed to 4.35.0. Keep code, tests and documentation in English. Write
code without comments, trailing spaces or alignment padding. End files with a
newline. Environment variables used by the host are required inputs.

Run `nix develop --command make check` and
`nix build --option cores 4 --max-jobs 1`. Run `nix fmt`, inspect the staged diff
and commit each completed unit. Add new Lean and Nix files to Git before using
Nix. Formatting passes only Git-tracked paths to treefmt, and the check verifies
that untracked local files are excluded. Limit local builds and benchmarks to at most four available CPUs. Use
`cargo -j 4` for Rust builds. Tests and CI must not require fixed CPU numbers.

Enable the repository's pre-commit check with `git config core.hooksPath .githooks`.
The hook runs `nix develop --command make check`. CI runs the same check and
`nix build` on pushes to master and pull requests. Keep action version tags in
`.github/workflows/ci.yml` and update their commits through `gh actions-lock
--no-interactive`; verify with `gh actions-lock --verify` and commit the workflow
and `actions.lock` together. Dependabot groups weekly minor and patch updates
for Cargo and GitHub Actions.

## Responsibilities

- `src/cli.rs`: the command tree, help and file-valued parameters.
- `src/project.rs`: Lake discovery, setup, selection and bounded file workers.
- `src/process.rs`: child pipes and supervisor lifetime channels.
- `src/bin/leanwake-supervisor.rs`: process groups, host-death detection and descendant cleanup.
- `src/model.rs`: the PascalCase schema, version 2, with the version checked before decoding.
- `src/json_io.rs`: stack-growing JSON decoding and serialization for deep traces.
- `src/hotspots.rs`: the untraced ranking, the traced pass for listed declarations, the tactic breakdown and its output.
- `src/main.rs`: exit codes, command and file listings, and output formats.
- `src/steps.rs`: command selection, trace flattening, sorting and step output.
- `src/compare.rs`: profile validation, grouping by path, metadata lookup, sorting and output.
- `src/rules.rs`: embedded Lean comparison definitions and proofs.
- `lean/` and `.leanfran/cache/`: extracted Lean and generated Rust, committed together.
- `tools/check-rules.py`: source agreement, regeneration, axiom audit and proof mutation.
- `audit/`: leanassay mutation checks for the generated functions called by Rust.
- `tools/check-format.py`: tracked-file formatting with isolated Git configuration.
- `tools/check-package.py`: installed wrapper, plugin compiler, local Git dependency and PATH priority.
- `tools/profile-mathlib.py` and `tools/test-profile-mathlib.py`: pinned official-toolchain Mathlib recording, checkpoints and per-phase measurement metadata.
- `tools/report-mathlib.py` and `tools/test-report-mathlib.py`: deterministic Mathlib reports, phase metadata and synchronous-error file coverage.
- `native/Project.lean`: Lake's loaded workspace, module arguments and paths.
- `native/Main.lean`: synchronous elaboration, per-command tracing, ranges, names and measurements.
- `native/Sha256.lean`: SHA-256 for source identity and unnamed command keys.
- `tests/profile.rs` and `tests/fixtures/`: executable acceptance tests.

The Rust host owns I/O and orchestration. Native Lean owns compiler state and
measurement. The embedded Lean core owns classification, matching by `(Path, Key)`,
summary counts and heartbeat totals, and regression failure. Rust calls the generated
`match_files`, `summarize` and `failed` functions in the comparison path.
Native runtime objects stay inside the adapter process; the boundary uses JSON.

## Lake 4.35.0 discovery

The pinned nixpkgs revision `e126e9e73c335158ff8279ac14d5f4afec153716`
provides `Lean (version 4.35.0, Release)`. The flake lock preserves this revision.

The fixture produced these actual query results (the absolute project prefix
in the source path is replaced with `<project>`):

```text
lake -J query :defaultModules
["Sample.Basic","Sample.Options","Sample"]
lake -J query +Sample:transImports
["Sample.Basic","Sample.Options"]
lake -J query +Sample.Basic:lean
"<project>/Sample/Basic.lean"
lake -J query +Sample.Basic:setup
{"dynlibs":[],"importArts":{},"isModule":false,"name":"Sample.Basic","options":{"autoImplicit":false},"package":"sample","plugins":[]}
```

The host expands `:defaultModules` with `:transImports`, queries each `:lean`
path, and checks every lexical path component before canonicalization. Keep
source files in the project outside hidden paths and symbolic links, including
symbolic link ancestors, using the same selection function as explicit paths. This
handles executable roots, library globs and source directories through Lake.
`:setup` builds imports, plugins and other module dependencies before returning
`ModuleSetup`. Preparation is serial to avoid concurrent Lake writers. Adapter
processes then run with at most `--jobs` files in parallel.

`lake setup-file Sample/Basic.lean` returned the same setup JSON for this basic
fixture. Its implementation uses `Module.serverOptions`, however, whereas
`:setup` uses the build's `Module.leanOptions`. Use `:setup` for registered
modules. `native/Project.lean` loads the workspace with Lake's own loader and
reads `Module.weakLeanArgs ++ Module.leanArgs`. In the option fixture it returns:

```json
{"Arguments":["-DmaxRecDepth=1234","-Dlinter.unusedVariables=false"],"Options":{"autoImplicit":false}}
```

The adapter parses `-D` arguments and merges setup options with the same
precedence as Lean's frontend. After imports initialize option declarations,
`Language.Lean.reparseOptions` resolves weak prefixes and imported Bool, Nat and
String options, and rejects unknown strong options. Measurement overrides are
applied last. Other Lean CLI arguments fail explicitly. Dynamic libraries, plugins, package identity,
module-system mode and pre-resolved import artifacts come from `ModuleSetup`.
`LEAN_PATH` and `LEAN_SRC_PATH` come from a `lake env` invocation of the helper.
Explicit files outside Lake targets use `setup-file` for their imports and the
package's build options and arguments, with their module name derived by Lean.

## Measurement contract

Parse each command before measuring `Command.elabCommandTopLevel`. Pass the
previously parsed commands to the terminal module linters, as the Lean frontend
does; terminal linter errors contribute to the file's `HasErrors`. Read the raw
thread heartbeat counter and monotonic nanosecond clock immediately around
elaboration. Name collection, sorting, hashing, diagnostics and serialization
are outside the measurement interval. Preserve all project limits and options
except `Elab.async`, which is forced to false. In steps mode, also force the trace
profiler in time mode (`trace.profiler.useHeartbeats=false`, so node start and stop
times come from `IO.monoNanosNow`), enable `trace.Leanwake.tactic`, and retain the
trace state. `record-profile` captures with a zero threshold. Apply the requested
step threshold in nanoseconds only when exporting the tree, outside measurement.
Keep nodes of at least that many nanoseconds and every `Leanwake.tactic` node,
promoting retained children of omitted nodes. This keeps command and trace
measurements independent of the output threshold.

`list-hotspots` ranks commands by `ElapsedNs` from a detail none measurement and
never from traced times. For the breakdown it sends the adapter the start positions
of the listed commands in each file containing them. The adapter traces only those
commands with a 1 ms capture threshold, sets `trace.profiler` and
`trace.Leanwake.tactic` to false for the other commands, emits rows only for the
traced commands and stops after the last one. A per-command mode stored in an
`IO.Ref` is read by the command elaborator wrappers, so nested scopes get the same
options. The host matches traced rows by path and start position, verifies source
hashes, rejects a different `Key` on error-free commands, and groups the innermost
tactic nodes with a range by kind and range. In a chain of nodes with one range it reports the outermost kind that is not
a tactic sequence. Without tactic nodes it reports the self time of trace classes.
Shares inside a declaration are relative to its traced `ElapsedNs`.

With both `--profile` and `--project-dir`, validate synchronous detail-none conditions,
the current toolchain and every recorded source hash, then reuse the untraced ranking
without recording again. Errored traced rows may have fallback keys when declaration
creation fails; retain the untraced identity and emit their incomplete breakdowns.

Wrap every command elaborator entry, restoring measurement options before and
after each handler. Refresh wrappers before each top-level command; a scoped
extension tracks wrapped entries across section boundaries. This also covers
nested `elabCommand` calls from custom elaborators. In steps mode, wrap tactic
elaborator entries with `Leanwake.tactic` trace nodes carrying originating syntax
and the syntax kind. A separate scoped extension tracks wrapped tactics.
Unknown trace positions stay null; child nodes never inherit a parent's position. Read and
write trace trees through `json_io` and drain nested children iteratively on
drop; real profiles exceed serde_json's default nesting limit.

Before stopping the clock and reading the final heartbeat counter, wait for the
command environment's `checked` task. This joins Lean-owned kernel checking and
lazy-realization replay, including the checked-task dependency chain, inside the
measured wall time. Heartbeats remain the raw measuring thread's counter delta;
do not add worker-thread allocations or replace it with elapsed time.
Kernel replay completion can leave its diagnostic snapshot callback queued. Allow
registered snapshots and their children up to one second total to settle after
joining `checked`, still inside the measured interval. This is a reporting grace
period, not a timeout on kernel checking. Snapshot promises do not identify their
producer, so this bounded wait also covers user-created snapshots without hanging.
Then collect completed snapshot diagnostics and trace trees after measurement.
Check every snapshot and its children without waiting; an unfinished task marks
the command and file as errored, reports why, and stops that file. Arbitrary
detached tasks created by user metaprograms outside Lean's snapshot registry are
outside the measurement contract. Only the environment's checked chain is joined without a
deadline; never unconditionally join arbitrary snapshot promises. Fixtures check
nested synchronous proof costs, realized theorem kernel replay and its diagnostics
and traces, and rejection of unfinished registered tasks in both detail modes.

Keep parser diagnostics separately: Lean's top-level elaborator resets its
message log. A file with parser, import or elaboration errors has `HasErrors`.
Report each command's own errors without carrying an earlier command's errors
into subsequent commands. Redirect elaborator standard output to standard
error so that `#eval IO.println` preserves the JSON protocol.
Record an explicit `#exit` before stopping; omit only the synthetic end-of-input
command.

Collect new constants with declaration-range metadata. Sort by declaration
range start and then lexicographically by name. The first name is `Key`. For
unnamed commands, hash the UTF-8 source from the syntax start to its tail,
excluding surrounding trivia, and use the first 12 SHA-256 hex digits plus a
one-based occurrence number for that exact kind and source in the file.

Tests check repeatability, source hashing, duplicate examples, names, module
paths, plugins, configuration mutations, heartbeat limits, syntax errors,
continued recording after failed imports, explicit hidden paths and symlink
ancestors, all help levels, exit codes, PascalCase and file/stdin inputs.
Each subprocess runs through a supervisor in a separate process group. The
supervisor watches a close-on-exec lifetime channel held only by the host and
kills the subprocess group when the host exits, including SIGKILL. It observes
child exit with `waitid(WNOWAIT)`, kills the group before reaping its leader, and
preserves the child's status. Closing the lifetime channel also cleans up a
failed input write before the host joins pipe readers. Subprocesses that
explicitly detach into another process group are outside this lifecycle.

Linux process tests build a real Lake dependency whose Lean compiler signals
PID readiness and blocks. They check SIGINT, SIGTERM and SIGKILL, completed and
failed builds with background descendants, and an input-pipe error. The
120-second startup and 30-second termination deadlines are watchdogs; fixtures
wait for an hour so natural completion cannot satisfy the checks.
Unsupported toolchains and unsupported compiler arguments fail rather
than silently changing the execution conditions.

## Repeatability evidence

Official Lean 4.35.0 has synchronous private-name export failures reproducible with
plain `lean -DElab.async=false --setup ...`; Lake's default asynchronous builds of
the same Mathlib files succeed. Do not work around this in the adapter or change
the pinned toolchain. Mathlib reports list errored files from the untraced checkpoint
and warn that commands after failures may be missing or errored.

On 2026-11-17, two recordings with `--jobs 4` on four Linux CPU cores measured
61 files and 967 commands in lean-ritsuryo, and 30 files and 253 commands in
leanassay. Both full selections and their corresponding Lake builds succeeded
without error files. Heartbeats differed in 22 ritsuryo commands (maximum 15)
and 16 assay commands (maximum 137). Assay's default selection of 8 files and
133 commands matched exactly, as did the deterministic integration fixtures.

Keep raw counters; do not round these differences away. Lean 4.35.0's
`Environment.realizeValue` explicitly resets heartbeats around nondeterministic
realization work, and `realizeConst` schedules kernel replay tasks independently
of the top-level elaboration option. The precise source of the remaining small
differences has not been isolated. Use the measured variation when choosing
comparison thresholds. One task worker alone did not eliminate the
variation in a repeated single-file experiment.

Keys follow the specified declaration-name rule and are scoped by file, not
globally unique across a profile. The full ritsuryo selection has `main` in four
files; assay has 24 keys each occurring in two different files. The repeatability
measurements and comparisons match `(Path, Key)` pairs. Reject duplicate pairs
within each input before calling the generated matcher.

## Comparison rules and proofs

`src/rules.rs` is the source for the `core` Lean block. `grows b h a r` means
`a < h - b` and `b * r < (h - b) * 100`, with all operations on `Nat`.
`classify` first tests growth and then the same predicate with inputs reversed.
The generated runtime uses arbitrary precision naturals, including intermediate
products and totals. Rust accepts heartbeat fields and thresholds as `u64` and
serializes totals and signed deltas as exact JSON integers.

`takeMatch` removes the first head entry with the same path and key. `matchEntries`
consumes the base list, classifies matched entries, marks absent entries removed,
and marks the remaining head entries added. Duplicate input identities are rejected
by Rust. Rust groups both inputs by path, including repeated file objects with
different keys. `matchFiles` applies `matchEntries` to each pair of file lists.
Matching costs the sum of the per-file squared command counts, so work scales
with file count at a fixed number of commands per file. A single very large file
still has quadratic cost. `summarize` folds with an accumulator to avoid stack
growth proportional to the total number of rows.

The proofs checked in `make check` include:

- `grows_lt`: growth requires a strictly larger head heartbeat count.
- `grows_asymmetric`: growth in one direction excludes growth in the other.
- `classify_regressed`: regression holds exactly when forward growth exceeds both thresholds.
- `classify_improved`: improvement holds exactly when reverse growth exceeds both thresholds.
- `equal_unchanged`: equal heartbeat counts classify as unchanged.
- `swap_classification`: reversing inputs reverses regression and improvement.
- `regression_monotone`: increasing the head preserves a regression.
- `grows_thresholds`: increasing either threshold can only restrict growth.
- `regression_thresholds` and `improvement_thresholds`: increasing either threshold
  cannot introduce a regression or improvement.
- `takeMatch_conservation`: the extracted match and remainder preserve every head entry.
- `matching_base_once`: the base projection of the result equals the input list.
- `matching_head_once`: the head projection is a permutation of the input list.
- `matching_files_base_once` and `matching_files_head_once`: the same occurrence
  conservation across the grouped input lists.
- `summary_acc_regressed`: the fold preserves the initial regression count.
- `summary_regressed`: the regression count equals the number of regressed rows.
- `failure_iff_regression`: failure holds exactly when a regressed row exists.
- `summary_acc_counts`: the fold adds improved, unchanged, added and removed row
  counts to the corresponding initial counts.
- `summary_counts`: improved, unchanged, added and removed counts equal the
  numbers of rows with those statuses.
- `summary_acc_heartbeats`: the fold adds each side's present-entry heartbeat sum
  to that side's initial total.
- `summary_heartbeats`: base and head totals equal the heartbeat sums of the
  respective present entries, including added and removed rows.

`audit/` is a separate Lake package requiring `lean/` and leanassay at
`ef054d7c45ab4f9e14a201ae7e3bfbe9dd080abb`. `Audit/Core.lean` audits every generated
function called by Rust: `matchFiles`, `summarize` and `failed`. Its local
`Inhabited Summary` instance supplies the all-zero result
for the default-result mutation of `summarize`.

| Function | Mutants | Killed | Holes | Untested | Equivalent | Timed out | Stillborn |
|---|---:|---:|---:|---:|---:|---:|---:|
| `matchFiles` | 2 | 2 | 0 | 0 | 0 | 0 | 0 |
| `summarize` | 1 | 1 | 0 | 0 | 0 | 0 | 0 |
| `failed` | 7 | 7 | 0 | 0 | 0 | 0 | 0 |

`lake -d audit build --wfail` rejects a missing theorem, zero killed mutations,
holes, and possibly vacuous hypotheses. Each generated function added to the Rust
call path gets a useful theorem and an `#assay` line in the same change. The audit
mutates the named function's body; the helper definitions keep their original
bodies. Equivalent means equal outputs on the sampled inputs, with the default
sample size bounded by 30. The audit complements the kernel-checked proofs and
the classification and file-conservation mutations below.

The helper lemmas are checked with these theorems. The axiom audit permits only
Lean's standard `propext`, `Classical.choice` and `Quot.sound`. A deliberately
incorrect unchanged branch and dropping the first file must each fail proof
checking on every `make check`. The 100,000-command integration test reverses
both file and command order and checks unchanged, regressed, improved, added
and removed counts, totals and identities without a timing assertion. A separate
single-file test reverses 20,000 commands and checks exact counts and totals on
the standard process stack.

Run `nix develop --command cargo build -j 4` after changing embedded Lean to extract
and generate through `build.rs`. The dev shell sets `LEANFRAN_CLI` to the compiler
from the pinned leanfran input. Add new `lean/` and `.leanfran/cache/` files before
invoking Nix again, then run `nix develop --command make check`. That command checks
that the extracted source matches the embedded block, rebuilds the Lean package,
regenerates Rust and compares it byte for byte with the cache, audits every theorem,
checks `lake -d audit build --wfail`, and runs CLI acceptance tests. Commit the
source and both generated directories, plus the audit manifest and toolchain.
`nix build` sets `LEANFRAN_OFFLINE=1` and builds from the committed cache inside the
network-isolated Nix build. A missing cache is an error.

Every profile reader rejects a `SchemaVersion` other than 2 before decoding the rest,
asking to record again. Comparison validation checks `Toolchain` and `Conditions`
before classification and prints both values for every differing field. Reject errored
files, errored commands and duplicate pairs on either side. The supported profile
schema and conditions are version 2, synchronous elaboration, and detail `none`
or `steps`. Every command in a steps profile must have `Steps`; commands in a
none profile must omit it. Comparing different details fails before classification.
Missing sides of added or removed commands are JSON null; their delta uses zero
for that side. Ranges come from head when present and from base for removals.
Omit unchanged rows and order regressions first by descending delta, then use
status, descending delta, path and key for deterministic output.

## Step acceptance tests

The steps fixture contains identical `simp` tactics on separate lines, a locally
registered tactic, nested and persistent profiler option overrides, instance
resolution and conditional simplification. Check exact ranges, positive counts,
repeated trace structure (class, kind and range), positive node times,
threshold-independent command counts, retained tactic nodes at any threshold, omitted
`Steps` for detail none, and comparison using command counts for detail steps.
`list-steps` must reject unavailable commands, detail none and inconsistent
records, and report descending elapsed times with tree depths in every output format.

## Hotspot acceptance tests

The hotspots fixture has one theorem whose time is dominated by `decide +kernel`
at a known range, a term-mode proof and several fast declarations in three files.
`list-hotspots` must rank that theorem first with its path, line and name, report
the `decide` range as the first inside entry, report trace classes for the
term-mode proof, keep the total equal to the sum of command times and keep listed
and inside times at or below their totals. Rankings from `--profile` must equal the
recording sorted by elapsed time, path and key. Tests check 252 for neither input,
`--profile` alone with `--detail steps`, `--jobs` or `--paths`, and both inputs with
`--detail none`. Checkpoint tests cover unchanged rankings, empty profiles, source,
toolchain and conditions mismatches, and error-free key mismatches. A heartbeat-bound
fixture checks traced-only failure before declaration creation, retained identity,
errored breakdowns and printed output. Tests assert no absolute times.
