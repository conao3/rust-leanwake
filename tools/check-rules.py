import json
import os
import re
import subprocess
import tempfile
import textwrap
from pathlib import Path


def run(args, **kwargs):
    return subprocess.run(
        args, check=True, text=True, capture_output=True, **kwargs
    ).stdout


def main():
    source = Path("src/rules.rs").read_text()
    body = textwrap.dedent(source.split('r#"\n', 1)[1].split('"#', 1)[0]).strip()
    modules = list(Path("lean/LeanRustGen/Embed/leanwake").glob("*/core.lean"))
    caches = list(Path(".leanfran/cache").glob("*.rs"))
    assert len(modules) == len(caches) == 1, "expected one comparison module and cache"
    expected = f"import Std\nnamespace core\n\n{body}\n\nend core\n"
    assert modules[0].read_text() == expected, "embedded Lean and extracted Lean differ"
    run(["lake", "-d", "lean", "build"])
    module = ".".join(modules[0].relative_to("lean").with_suffix("").parts)
    scratch = Path(".claude-dev/scratch")
    scratch.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=scratch) as directory:
        directory = Path(directory).resolve()
        regenerated = directory / "core.rs"
        batch = directory / "batch.json"
        batch.write_text(
            json.dumps(
                [
                    {
                        "module": module,
                        "namespace": "core",
                        "type_paths": {},
                        "model_namespace": "leanwake",
                        "output_file": str(regenerated),
                    }
                ]
            )
        )
        search = run(["lake", "env", "printenv", "LEAN_PATH"], cwd="lean").strip()
        report = json.loads(
            run(
                [
                    os.environ["LEANFRAN_CLI"],
                    "module",
                    "generate",
                    "--batch",
                    str(batch),
                    "--output",
                    "json",
                    "--search-path",
                    *search.split(":"),
                ],
                cwd="lean",
            )
        )
        assert not report[0].get("skipped"), report
        run(["rustfmt", "--edition", "2021", str(regenerated)])
        assert regenerated.read_bytes() == caches[0].read_bytes(), (
            "generated Rust cache differs"
        )
        theorems = re.findall(r"^theorem (\w+)", body, re.MULTILINE)
        audit = directory / "Audit.lean"
        audit.write_text(
            expected
            + "\n"
            + "\n".join(f"#print axioms core.{name}" for name in theorems)
            + "\n"
        )
        output = run(["lean", str(audit)])
        assert "warning:" not in output, output
        declarations = re.findall(
            r"'core\.(\w+)' (?:depends on axioms: \[([^\]]*)\]|does not depend on any axioms)",
            output,
        )
        assert len(declarations) == len(theorems), output
        allowed = {"propext", "Classical.choice", "Quot.sound"}
        for name, axioms in declarations:
            assert set(filter(None, map(str.strip, axioms.split(",")))) <= allowed, (
                name,
                axioms,
            )
        mutations = {
            "classification": expected.replace("else .unchanged", "else .regressed", 1),
            "file conservation": expected.replace(
                "files.flatMap (fun (bs, hs) => matchEntries bs hs a r)",
                "files.tail.flatMap (fun (bs, hs) => matchEntries bs hs a r)",
                1,
            ),
        }
        for name, source in mutations.items():
            assert source != expected, f"mutation did not change {name}"
            mutant = directory / "Mutant.lean"
            mutant.write_text(source)
            rejected = subprocess.run(
                ["lean", str(mutant)], check=False, text=True, capture_output=True
            )
            assert rejected.returncode != 0, f"incorrect {name} was accepted"
            assert "error:" in rejected.stdout, rejected.stdout
        print(
            f"Checked {len(theorems)} theorems, generated Rust, "
            "and rejected classification and file-conservation mutations"
        )


if __name__ == "__main__":
    main()
