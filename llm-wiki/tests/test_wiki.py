import importlib.util
import tempfile
import unittest
from unittest.mock import patch as mock_patch
from pathlib import Path

spec = importlib.util.spec_from_file_location("wiki_toolkit", Path(__file__).parent.parent / "wiki.py")
wiki = importlib.util.module_from_spec(spec)
import sys
sys.modules[spec.name] = wiki
spec.loader.exec_module(wiki)


class WikiToolkitTests(unittest.TestCase):
    def make_plan(self, root, has_more=False):
        changes = {"records": [{"record_id": "r1", "entities": ["project:x"]}],
                   "next_cursor": "v2|2026-10-01T00:00:00Z|r1", "has_more": has_more}
        plan = wiki.build_plan(root, changes)
        job = plan["jobs"][0]
        patch = {"version": 1, "job_id": job["job_id"], "operation": "upsert",
                 "page": "Projects/X.md", "evidence_record_ids": ["r1"],
                 "content": "---\nentities:\n  - project:x\n---\n\nFact [source](pasta:evidence:r1)\n"}
        return changes, plan, patch

    def test_symlink_escape_is_rejected_without_writing(self):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td) / "wiki"
            root.mkdir()
            outside = Path(td) / "outside"
            outside.mkdir()
            (root / "Projects").symlink_to(outside, target_is_directory=True)
            _, plan, patch = self.make_plan(root)
            self.assertTrue(wiki.validate_patch(root, plan, patch))
            with self.assertRaises(wiki.WikiError):
                wiki.apply_patch(root, plan, patch)
            self.assertFalse((outside / "X.md").exists())

    def test_unrelated_page_cannot_be_overwritten(self):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            target = root / "Other.md"
            target.write_text("---\nentities:\n  - project:y\n---\nUnrelated page\n")
            _, plan, patch = self.make_plan(root)
            patch["page"] = "Other.md"
            self.assertTrue(wiki.validate_patch(root, plan, patch))
            self.assertIn("Unrelated page", target.read_text())

    def test_edit_since_planning_is_preserved(self):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            target = root / "Projects/X.md"
            target.parent.mkdir()
            target.write_text("---\nentities:\n  - project:x\n---\nOriginal\n")
            _, plan, patch = self.make_plan(root)
            target.write_text("My newer manual edit\n")
            with self.assertRaises(wiki.WikiError):
                wiki.apply_patch(root, plan, patch)
            self.assertEqual(target.read_text(), "My newer manual edit\n")

    def test_cursor_requires_all_jobs_completed_and_supports_pages(self):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            changes, plan, patch = self.make_plan(root, has_more=True)
            with self.assertRaises(wiki.WikiError):
                wiki.advance(root, plan, changes)
            wiki.apply_patch(root, plan, patch)
            self.assertEqual(wiki.advance(root, plan, changes), changes["next_cursor"])

    def test_page_write_is_recoverable_if_receipt_write_fails(self):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            changes, plan, patch = self.make_plan(root)
            with mock_patch.object(wiki, "save_state", side_effect=OSError("disk failure")):
                with self.assertRaises(OSError):
                    wiki.apply_patch(root, plan, patch)
            wiki.apply_patch(root, plan, patch)
            wiki.advance(root, plan, changes)

    def test_advance_rejects_different_change_page_or_modified_output(self):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            changes, plan, patch = self.make_plan(root)
            target = wiki.apply_patch(root, plan, patch)
            with self.assertRaises(wiki.WikiError):
                wiki.advance(root, plan, dict(changes, next_cursor="different"))
            target.write_text("Modified after apply")
            with self.assertRaises(wiki.WikiError):
                wiki.advance(root, plan, changes)

    def test_skip_requires_reason_and_records_completion(self):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            changes, plan, patch = self.make_plan(root)
            with self.assertRaises(wiki.WikiError):
                wiki.skip_job(root, plan, patch["job_id"], " ")
            wiki.skip_job(root, plan, patch["job_id"], "No durable change")
            wiki.advance(root, plan, changes)

    def test_removed_mention_still_schedules_existing_page(self):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            (root / "Old.md").write_text("---\nentities:\n  - project:old\n---\n[source](pasta:evidence:r1)\n")
            _, plan, _ = self.make_plan(root)
            job = next(job for job in plan["jobs"] if job["entity"] == "project:old")
            self.assertEqual(job["candidate_pages"], ["Old.md"])

    def test_state_directory_symlink_is_rejected(self):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td) / "wiki"
            root.mkdir()
            (root / wiki.STATE_DIR).symlink_to(Path(td), target_is_directory=True)
            with self.assertRaises(wiki.WikiError):
                wiki.load_state(root)

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
