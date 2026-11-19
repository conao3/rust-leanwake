import copy
import json
import runpy
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

script = Path(__file__).with_name("report-mathlib.py")
render = runpy.run_path(str(script))["render"]


class ReportTests(unittest.TestCase):
    def setUp(self):
        self.position = {
            "Start": {"Line": 2, "Column": 3},
            "End": {"Line": 4, "Column": 5},
        }
        self.inside = {
            "Class": "Leanwake.tactic",
            "Kind": "simp",
            "Range": self.position,
            "Count": 3,
            "ElapsedNs": 1000000000,
            "Share": 0.5,
        }
        self.entry = {
            "Path": "Mathlib/A.lean",
            "Key": "a",
            "Name": "a|<b>`",
            "Kind": "declaration",
            "Range": self.position,
            "ElapsedNs": 2000000000,
            "Share": 0.5,
            "Heartbeats": 42,
            "Traced": {"Inside": [self.inside]},
        }
        self.hotspots = {
            "Hotspots": [self.entry],
            "ElapsedNs": 4000000000,
            "Commands": 2,
            "Files": 1,
            "FilesWithErrors": 0,
        }
        self.metadata = {
            "MathlibCommit": "mathlib-commit",
            "Toolchain": "Lean 4.35.0",
            "LeanwakeCommit": "leanwake-commit",
            "Jobs": 4,
            "CpuModel": "CPU",
            "Nproc": 8,
            "WallSeconds": 1.25,
            "ExitCode": 0,
            "RecordWallSeconds": 0.75,
            "RecordExitCode": 0,
            "TraceWallSeconds": 0.5,
            "TraceExitCode": 0,
        }
        self.profile = {"Files": []}

    def test_aggregation_and_escaping(self):
        second = copy.deepcopy(self.entry)
        second["Key"] = "b"
        second["Traced"]["Inside"].append(copy.deepcopy(self.inside))
        second["Traced"]["Inside"].append({**self.inside, "Class": "Meta.whnf"})
        self.hotspots["Hotspots"].append(second)
        result = render(self.hotspots, self.metadata, self.profile)
        self.assertIn("a&#124;&lt;b&gt;&#96;", result)
        self.assertIn("simp 2:3-4:5 x3 50.0%", result)
        self.assertIn("| simp | 3.000000 | 3000000000 | 9 | 2 |", result)
        self.hotspots["Hotspots"].reverse()
        self.assertEqual(result, render(self.hotspots, self.metadata, self.profile))

    def test_empty_and_missing_traces(self):
        self.inside["Range"] = None
        result = render(self.hotspots, self.metadata, self.profile)
        self.assertIn("No files with errors in the untraced recording.", result)
        self.assertIn(
            "Recording: 0.75 s, exit code 0; checkpoint tracing: 0.50 s, exit code 0.",
            result,
        )
        self.assertIn("simp x3 50.0%", result)
        self.assertNotIn("simp unknown", result)
        self.entry["Range"] = None
        self.entry["Traced"] = None
        result = render(self.hotspots, self.metadata, self.profile)
        self.assertIn("Mathlib/A.lean:unknown", result)
        self.assertIn("unavailable", result)
        self.hotspots["Hotspots"] = []
        self.assertIn(
            "Showing 0 listed declarations.",
            render(self.hotspots, self.metadata, self.profile),
        )

    def test_duplicate_identity_is_rejected(self):
        self.hotspots["Hotspots"].append(self.entry)
        with self.assertRaisesRegex(ValueError, "duplicate hotspot identity"):
            render(self.hotspots, self.metadata, self.profile)

    def test_limit_and_tactic_order(self):
        self.hotspots["Hotspots"] = [
            {**self.entry, "Key": str(index), "ElapsedNs": index} for index in range(51)
        ]
        self.entry["Traced"]["Inside"].append({**self.inside, "Kind": "aesop"})
        result = render(self.hotspots, self.metadata, self.profile)
        self.assertIn("Showing 50 listed declarations.", result)
        self.assertLess(result.index("| aesop |"), result.index("| simp |"))

    def test_cli_check_and_errors(self):
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory)
            (work / "hotspots.json").write_text(json.dumps(self.hotspots))
            (work / "metadata.json").write_text(json.dumps(self.metadata))
            (work / "profile.json").write_text(json.dumps(self.profile))
            command = [
                sys.executable,
                str(script),
                "--input-dir",
                str(work),
                "--output-dir",
                str(work),
            ]
            result = subprocess.run(
                command, capture_output=True, text=True, check=False
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            result = subprocess.run(
                [*command, "--check"], capture_output=True, text=True, check=False
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            (work / "README.md").write_text("stale\n")
            result = subprocess.run(
                [*command, "--check"], capture_output=True, text=True, check=False
            )
            self.assertEqual(result.returncode, 1)
            self.assertIn("Report differs:", result.stderr)
            (work / "hotspots.json").write_text("invalid")
            result = subprocess.run(
                command, capture_output=True, text=True, check=False
            )
            self.assertEqual(result.returncode, 1)
            self.assertIn("Cannot generate report:", result.stderr)
            self.assertEqual((work / "README.md").read_text(), "stale\n")

    def test_phase_metadata_and_error_files(self):
        command = {"Key": "bad|name", "Range": self.position, "HasErrors": True}
        self.profile["Files"] = [
            {"Path": "Mathlib/Z.lean", "HasErrors": True, "Commands": []},
            {"Path": "Mathlib/Clean.lean", "HasErrors": False, "Commands": []},
            {
                "Path": "Mathlib/A.lean",
                "HasErrors": True,
                "Commands": [
                    {**command, "HasErrors": False},
                    command,
                    command,
                ],
            },
            {"Path": "Mathlib/Missing.lean", "HasErrors": True},
        ]
        self.metadata.update(
            {"RecordExitCode": 254, "TraceExitCode": 254, "ExitCode": 254}
        )
        result = render(self.hotspots, self.metadata, self.profile)
        self.assertIn(
            "Recording: 0.75 s, exit code 254; checkpoint tracing: 0.50 s, exit code 254.",
            result,
        )
        self.assertIn("| Mathlib/A.lean | bad&#124;name (line 2) | 2/3 |", result)
        self.assertIn("| Mathlib/Z.lean | unavailable | 0/0 |", result)
        self.assertIn("| Mathlib/Missing.lean | unavailable | unavailable |", result)
        self.assertNotIn("Mathlib/Clean.lean", result)
        self.assertLess(
            result.index("| Mathlib/A.lean |"), result.index("| Mathlib/Z.lean |")
        )
        self.assertIn("lean -DElab.async=false --setup ...", result)
        self.profile["Files"].reverse()
        self.assertEqual(result, render(self.hotspots, self.metadata, self.profile))

    def test_missing_checkpoint_is_an_error(self):
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory)
            (work / "hotspots.json").write_text(json.dumps(self.hotspots))
            (work / "metadata.json").write_text(json.dumps(self.metadata))
            result = subprocess.run(
                [
                    sys.executable,
                    str(script),
                    "--input-dir",
                    str(work),
                    "--output-dir",
                    str(work),
                ],
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(result.returncode, 1)
            self.assertIn("profile.json", result.stderr)
            self.assertFalse((work / "README.md").exists())


if __name__ == "__main__":
    unittest.main()
