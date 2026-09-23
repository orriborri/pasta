import importlib.util
import tempfile
import unittest
from pathlib import Path

spec = importlib.util.spec_from_file_location("wiki_toolkit", Path(__file__).parent.parent / "wiki.py")
wiki = importlib.util.module_from_spec(spec)
import sys
sys.modules[spec.name] = wiki
spec.loader.exec_module(wiki)


class WikiToolkitTests(unittest.TestCase):
    def test_plan_validate_apply_and_audit(self):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td) / "Knowledge"
            root.mkdir()
            existing = root / "Projects" / "Alpha.md"
            existing.parent.mkdir()
            existing.write_text(
                "---\nentities:\n  - project:alpha\nevidence:\n  - old-1\n---\n\n# Alpha\n\nOld fact [src](pasta:evidence:old-1)\n",
                encoding="utf-8",
            )
            changes = {
                "records": [{
                    "record_id": "slack-C1-1",
                    "source": "slack",
                    "kind": "message",
                    "title": "alpha update",
                    "entities": ["project:alpha", "linear:AB-1"],
                    "author": "Alice",
                    "participants": [],
                    "thread_id": "T1",
                    "created_at": "2026-09-23T10:00:00Z",
                    "updated_at": "2026-09-23T10:00:00Z",
                    "url": "https://example",
                }],
                "next_cursor": "2026-09-23T10:00:00+00:00|slack-C1-1",
                "has_more": False,
            }
            plan = wiki.build_plan(root, changes)
            alpha = next(j for j in plan["jobs"] if j["entity"] == "project:alpha")
            self.assertEqual(alpha["candidate_pages"], ["Projects/Alpha.md"])
            self.assertIn("old-1", alpha["existing_evidence_record_ids"])

            patch = {
                "version": 1,
                "job_id": alpha["job_id"],
                "operation": "upsert",
                "page": "Projects/Alpha.md",
                "evidence_record_ids": ["old-1", "slack-C1-1"],
                "content": (
                    "---\nentities:\n  - project:alpha\nevidence:\n"
                    "  - old-1\n  - slack-C1-1\n---\n\n# Alpha\n\n"
                    "Old fact [src](pasta:evidence:old-1). New fact "
                    "[src](pasta:evidence:slack-C1-1).\n"
                ),
            }
            self.assertEqual(wiki.validate_patch(root, plan, patch), [])
            target = wiki.apply_patch(root, plan, patch)
            self.assertTrue(target.exists())
            report = wiki.audit(root)
            self.assertIn("slack-C1-1", report["evidence_record_ids"])
            self.assertEqual(report["uncited_pages"], [])

    def test_rejects_unsupported_evidence(self):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            changes = {
                "records": [{"record_id": "r1", "entities": ["project:x"]}],
                "next_cursor": "x|r1",
                "has_more": False,
            }
            plan = wiki.build_plan(root, changes)
            job = plan["jobs"][0]
            patch = {
                "version": 1,
                "job_id": job["job_id"],
                "operation": "upsert",
                "page": "Projects/X.md",
                "evidence_record_ids": ["made-up"],
                "content": "---\nentities:\n  - project:x\n---\n\n# X\n[p](pasta:evidence:made-up)\n",
            }
            self.assertTrue(wiki.validate_patch(root, plan, patch))

    def test_rejects_path_traversal(self):
        with self.assertRaises(wiki.WikiError):
            wiki.safe_page_path("../oops.md")


if __name__ == "__main__":
    unittest.main()
