import argparse
import json
import os
import shutil
import subprocess
import time
from pathlib import Path


def positive(value):
    number = int(value)
    if number < 1:
        raise argparse.ArgumentTypeError("must be positive")
    return number


def utc_now():
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())


def main():
    parser = argparse.ArgumentParser(
        description="Measure pinned Mathlib hotspots with the official Lean release"
    )
    parser.add_argument("--work-dir", required=True, type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--jobs", type=positive, default=4)
    parser.add_argument("--limit", type=positive, default=50)
    parser.add_argument("--paths", nargs="+", default=[])
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    work = args.work_dir.resolve()
    output = args.output_dir.resolve()
    work.mkdir(parents=True, exist_ok=True)
    output.mkdir(parents=True, exist_ok=True)
    project = work / "mathlib4"
    revision = "56a005855ea17e0b0958f73ec801ae140b0a1679"
    env = os.environ.copy()
    elan = shutil.which("elan") or str(Path.home() / ".elan/bin/elan")

    def capture(command, cwd=root):
        return subprocess.check_output(command, cwd=cwd, env=env, text=True).strip()

    with (output / "setup.log").open("w") as log:

        def run(command, cwd=root):
            print("Running:", " ".join(map(str, command)), flush=True)
            subprocess.run(
                command, cwd=cwd, env=env, stdout=log, stderr=log, check=True
            )

        if not project.exists():
            run(
                [
                    "git",
                    "clone",
                    "--branch",
                    "v4.35.0",
                    "--single-branch",
                    "https://github.com/leanprover-community/mathlib4.git",
                    str(project),
                ]
            )
        if capture(["git", "rev-parse", "HEAD"], project) != revision:
            raise RuntimeError("Mathlib checkout must be at " + revision)
        if capture(["git", "status", "--porcelain", "--untracked-files=no"], project):
            raise RuntimeError("Mathlib checkout has tracked modifications")
        toolchain = (project / "lean-toolchain").read_text().strip()
        run([elan, "toolchain", "install", toolchain])
        env["ELAN_TOOLCHAIN"] = toolchain
        lean = Path(capture([elan, "which", "lean"], project))
        env["PATH"] = str(lean.parent) + os.pathsep + env["PATH"]
        env["LEAN_SYSROOT"] = str(lean.parent.parent)
        env["LEAN_NUM_THREADS"] = "4"
        env["LEANFRAN_OFFLINE"] = "1"
        env["LEANWAKE_ADAPTER"] = str(root / "native/.lake/build/bin/leanwake-adapter")
        run(["lake", "exe", "cache", "get"], project)
        run(["make", "-C", "native", "build"])
        run(["cargo", "build", "-j", "4"])
        run(["lake", "--no-build", "build"], project)

    command = [
        str(root / "target/debug/leanwake"),
        "profile",
        "list-hotspots",
        "--project-dir",
        str(project),
        "--jobs",
        str(args.jobs),
        "--limit",
        str(args.limit),
        "--output",
        "json",
    ]
    if args.paths:
        command.extend(["--paths", *args.paths])
    cpu_model = next(
        (
            line.split(":", 1)[1].strip()
            for line in Path("/proc/cpuinfo").read_text().splitlines()
            if line.startswith("model name")
        ),
        "unknown",
    )
    metadata = {
        "MathlibCommit": revision,
        "Toolchain": capture(["lean", "--version"]),
        "LeanwakeCommit": capture(["git", "rev-parse", "HEAD"]),
        "Jobs": args.jobs,
        "Limit": args.limit,
        "Paths": args.paths,
        "CpuModel": cpu_model,
        "Nproc": int(capture(["nproc"])),
        "StartUtc": utc_now(),
    }
    print("Running:", " ".join(command), flush=True)
    start = time.monotonic()
    with (
        (output / "hotspots.json").open("w") as stdout,
        (output / "stderr.log").open("w") as stderr,
    ):
        result = subprocess.run(
            command, cwd=root, env=env, stdout=stdout, stderr=stderr, check=False
        )
    metadata.update(
        {
            "EndUtc": utc_now(),
            "WallSeconds": time.monotonic() - start,
            "ExitCode": result.returncode,
        }
    )
    (output / "metadata.json").write_text(json.dumps(metadata, indent=2) + "\n")
    print(json.dumps(metadata, indent=2), flush=True)
    return result.returncode


if __name__ == "__main__":
    raise SystemExit(main())
