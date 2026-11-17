import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

binary = Path(sys.argv[1]).resolve()
root = Path(__file__).resolve().parent.parent
(root / "target").mkdir(exist_ok=True)


def run(args, cwd, env=None):
    result = subprocess.run(
        args, cwd=cwd, env=env, capture_output=True, text=True, check=False
    )
    if result.returncode:
        raise RuntimeError(
            f"{args}: {result.returncode}\n{result.stdout}\n{result.stderr}"
        )
    return result.stdout


def record(project, env):
    value = json.loads(
        run(
            [str(binary), "profile", "record-profile", "--project-dir", str(project)],
            project,
            env,
        )
    )
    assert value["Files"]
    assert all(not file["HasErrors"] for file in value["Files"])
    return value


with tempfile.TemporaryDirectory(prefix="package-", dir=root / "target") as directory:
    work = Path(directory)
    env = {"PATH": "", "HOME": str(work), "TMPDIR": str(work)}
    plugin = work / "plugin"
    shutil.copytree(
        root / "tests/fixtures/layout",
        plugin,
        ignore=shutil.ignore_patterns(".lake", "lake-manifest.json"),
    )
    value = record(plugin, env)
    assert any(file["Path"] == "src/First.lean" for file in value["Files"])
    assert list((plugin / ".lake/build").rglob("*.so")) or list(
        (plugin / ".lake/build").rglob("*.dylib")
    )
    git = shutil.which("git")
    cc = shutil.which("cc")
    shell = shutil.which("sh")
    assert git and cc and shell
    dependency = work / "dependency"
    dependency.mkdir()
    (dependency / "lakefile.toml").write_text(
        'name = "dependency"\n[[lean_lib]]\nname = "Dependency"\n'
    )
    (dependency / "lean-toolchain").write_text("leanprover/lean4:v4.35.0\n")
    (dependency / "Dependency.lean").write_text("def dependencyValue : Nat := 42\n")
    git_env = {
        **os.environ,
        "GIT_CONFIG_GLOBAL": os.devnull,
        "GIT_CONFIG_NOSYSTEM": "1",
    }
    run([git, "init", "--initial-branch=main"], dependency, git_env)
    run([git, "add", "."], dependency, git_env)
    run(
        [
            git,
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-m",
            "Create dependency fixture",
        ],
        dependency,
        git_env,
    )
    revision = run([git, "rev-parse", "HEAD"], dependency, git_env).strip()
    consumer = work / "consumer"
    consumer.mkdir()
    (consumer / "lakefile.toml").write_text(
        'name = "consumer"\ndefaultTargets = ["Consumer"]\n'
        '[[lean_lib]]\nname = "Consumer"\n'
        '[[require]]\nname = "dependency"\n'
        f'git = "{dependency.as_uri()}"\nrev = "{revision}"\n'
    )
    (consumer / "lean-toolchain").write_text("leanprover/lean4:v4.35.0\n")
    (consumer / "Consumer.lean").write_text(
        "import Dependency\ntheorem dependencyWorks : dependencyValue = 42 := rfl\n"
    )
    record(consumer, env)
    assert (consumer / ".lake/packages/dependency/Dependency.lean").is_file()
    preferred = work / "preferred"
    preferred.mkdir()
    for name, executable in [("cc", cc), ("git", git)]:
        marker = work / f"{name}.used"
        script = preferred / name
        script.write_text(
            f'#!{shell}\nprintf used > "{marker}"\nexec "{executable}" "$@"\n'
        )
        script.chmod(0o755)
    shutil.rmtree(plugin / ".lake")
    shutil.rmtree(consumer / ".lake")
    (consumer / "lake-manifest.json").unlink()
    env["PATH"] = str(preferred)
    record(plugin, env)
    record(consumer, env)
    assert (work / "cc.used").read_text() == "used"
    assert (work / "git.used").read_text() == "used"
print("Installed wrapper: plugin compilation, Git dependency and caller PATH passed")
