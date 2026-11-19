import json
import runpy
import tempfile
import unittest
from pathlib import Path
from subprocess import CompletedProcess
from unittest.mock import patch

measure = runpy.run_path(str(Path(__file__).with_name("profile-mathlib.py")))["measure"]


class MeasurementTests(unittest.TestCase):
    def test_phases(self):
        for codes in [(0, 0), (254, 0), (0, 254), (254, 254), (252,), (0, 252)]:
            with self.subTest(codes=codes), tempfile.TemporaryDirectory() as directory:
                output = Path(directory)
                metadata = {}
                calls = []
                paths = ["Mathlib/A.lean"] if codes[0] == 254 else []

                def run(args, calls=calls, output=output, codes=codes, **kwargs):
                    calls.append(args)
                    if len(calls) == 2:
                        self.assertEqual(
                            (output / "profile.json").read_text(), "checkpoint"
                        )
                        saved = json.loads((output / "metadata.json").read_text())
                        self.assertEqual(saved["RecordExitCode"], codes[0])
                    kwargs["stdout"].write(
                        "checkpoint" if len(calls) == 1 else "listing"
                    )
                    kwargs["stderr"].write("diagnostic\n")
                    return CompletedProcess(args, codes[len(calls) - 1])

                with patch("subprocess.run", side_effect=run):
                    code = measure(
                        ["leanwake", "profile"],
                        output,
                        output,
                        metadata,
                        {},
                        paths,
                        50,
                        2,
                    )
                self.assertEqual(code, max(codes))
                self.assertEqual(len(calls), len(codes))
                self.assertEqual(metadata["RecordExitCode"], codes[0])
                self.assertEqual(
                    metadata["TraceExitCode"], codes[1] if len(codes) > 1 else None
                )
                self.assertEqual("--paths" in calls[0], bool(paths))
                if len(codes) > 1:
                    self.assertIn("--profile", calls[-1])
                    self.assertNotIn("--paths", calls[-1])
                self.assertEqual(
                    (output / "stderr.log").read_text(), "diagnostic\n" * len(codes)
                )


if __name__ == "__main__":
    unittest.main()
