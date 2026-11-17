import os
import subprocess
import tempfile
from pathlib import Path

root = Path(__file__).resolve().parent.parent
assert ".claude-dev" not in (root / ".gitignore").read_text()
(root / "target").mkdir(exist_ok=True)
env = {
    **{
        name: value for name, value in os.environ.items() if not name.startswith("GIT_")
    },
    "GIT_CONFIG_GLOBAL": os.devnull,
    "GIT_CONFIG_NOSYSTEM": "1",
}
system = subprocess.check_output(
    ["nix", "eval", "--raw", "--impure", "--expr", "builtins.currentSystem"], text=True
).strip()
formatter = subprocess.check_output(
    [
        "nix",
        "build",
        "--no-link",
        "--print-out-paths",
        "--option",
        "cores",
        "4",
        "--max-jobs",
        "1",
        f"{root}#formatter.{system}",
    ],
    text=True,
).strip()
with tempfile.TemporaryDirectory(prefix="format-", dir=root / "target") as directory:
    work = Path(directory)
    subprocess.run(["git", "init", "--quiet", str(work)], check=True, env=env)
    (work / "flake.nix").write_text("{ }\n")
    tracked = work / "tracked.py"
    tracked.write_text("value = 1\n")
    subprocess.run(
        ["git", "-C", str(work), "add", "tracked.py", "flake.nix"], check=True, env=env
    )
    (work / "untracked.py").write_text("unfinished(\n")
    (work / ".claude-dev").mkdir()
    (work / ".claude-dev/unfinished.py").write_text("unfinished(\n")
    command = [str(Path(formatter) / "bin/format-tracked"), "--ci"]
    subprocess.run(command, cwd=work, env=env, check=True)
    tracked.write_text("value=1\n")
    result = subprocess.run(
        command, cwd=work, env=env, capture_output=True, text=True, check=False
    )
    assert result.returncode != 0, result.stdout + result.stderr
print("Formatting checks tracked files and excludes untracked local files")
