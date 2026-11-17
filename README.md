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
```

The default selection follows Lake's default targets and their transitive local
imports. Use `--paths File.lean Directory` to select source files relative to the
project root. Directory traversal skips hidden entries and symbolic links.
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

This release implements recording and listing with `Conditions.Detail = "none"`.
Profile comparison with Lean-verified rules and detailed step profiles belong
to the subsequent implementation stages.
