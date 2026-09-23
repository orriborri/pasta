import importlib.util
import tempfile
import unittest
from pathlib import Path
import sys

SCRIPT = (
    Path(__file__).parent.parent
    / "skills"
    / "raw-inbox-memory"
    / "scripts"
    / "capture.py"
)
spec = importlib.util.spec_from_file_location("raw_inbox_capture", SCRIPT)
capture_mod = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = capture_mod
spec.loader.exec_module(capture_mod)


class RawInboxCaptureTests(unittest.TestCase):
    def test_writes_append_only_raw_markdown(self):
        with tempfile.TemporaryDirectory() as td:
            vault = Path(td)
            path = capture_mod.capture(
                vault,
                kind="preference",
                source="test-agent",
                text="I prefer short status updates.",
                context="personal assistant test",
            )
            self.assertTrue(path.exists())
            self.assertEqual(path.parent, vault / "0. Inbox" / "Raw")
            body = path.read_text(encoding="utf-8")
            self.assertIn('kind: "preference"', body)
            self.assertIn('source: "test-agent"', body)
            self.assertIn("I prefer short status updates.", body)
            self.assertIn('status: "raw"', body)

    def test_rejects_credential_like_capture_by_default(self):
        with tempfile.TemporaryDirectory() as td:
            with self.assertRaises(capture_mod.CaptureError):
                capture_mod.capture(
                    Path(td),
                    kind="note",
                    source="test-agent",
                    text="My API key is abc123",
                )

    def test_rejects_empty_capture(self):
        with tempfile.TemporaryDirectory() as td:
            with self.assertRaises(capture_mod.CaptureError):
                capture_mod.capture(
                    Path(td),
                    kind="note",
                    source="test-agent",
                    text="   ",
                )


if __name__ == "__main__":
    unittest.main()
