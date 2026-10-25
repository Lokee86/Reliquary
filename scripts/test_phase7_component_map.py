"""Negative fixtures for the Phase 7 reachable-source ownership ratchet."""
from __future__ import annotations

import sys
import unittest
from pathlib import Path, PurePosixPath

sys.path.insert(0, str(Path(__file__).resolve().parent))
from check_phase7_component_map import validate_map
from check_owner_boundaries import relative_module_path


class Phase7ComponentMapTests(unittest.TestCase):
    def test_complete_reachable_production_map_passes(self) -> None:
        errors = validate_map(
            {"src/lib.rs", "src/memory/store.rs"},
            [
                {"path": "src/lib.rs", "component": "public", "classification": "production"},
                {"path": "src/memory/store.rs", "component": "memory-store", "classification": "production"},
            ],
            {"public", "memory-store"},
        )
        self.assertEqual([], errors)

    def test_new_reachable_file_must_be_registered(self) -> None:
        errors = validate_map({"src/new_owner.rs"}, [], {"owner"})
        self.assertEqual(
            [{"path": "src/new_owner.rs", "problem": "reachable-production-file-unmapped"}],
            errors,
        )

    def test_nonproduction_and_unassigned_reachable_rows_fail(self) -> None:
        errors = validate_map(
            {"src/a.rs", "src/b.rs"},
            [
                {"path": "src/a.rs", "component": "owner", "classification": "test"},
                {"path": "src/b.rs", "classification": "production"},
            ],
            {"owner"},
        )
        self.assertEqual(
            [
                {"path": "src/a.rs", "problem": "reachable-file-not-classified-production"},
                {"path": "src/b.rs", "problem": "reachable-production-file-unassigned"},
            ],
            errors,
        )

    def test_unknown_component_and_duplicate_paths_fail(self) -> None:
        rows = [
            {"path": "src/a.rs", "component": "not-in-dag", "classification": "production"},
            {"path": "src/a.rs", "component": "owner", "classification": "production"},
        ]
        errors = validate_map({"src/a.rs"}, rows, {"owner"})
        self.assertEqual(
            [
                {"path": "src/a.rs", "problem": "duplicate-map-entry"},
                {"path": "src/a.rs", "problem": "unknown-component"},
            ],
            errors,
        )

    def test_module_path_accepts_extended_drive_and_unc_paths_safely(self) -> None:
        self.assertEqual(
            "src/archive/conversation_compaction_store.rs",
            relative_module_path(
                Path(r"\\?\C:\repo\src\archive\conversation_compaction_store.rs"),
                Path(r"C:\repo"),
            ),
        )
        self.assertEqual(
            "src/lib.rs",
            relative_module_path(
                Path(r"\\?\UNC\server\share\repo\src\lib.rs"),
                Path(r"\\server\share\repo"),
            ),
        )
        with self.assertRaises(ValueError):
            relative_module_path(Path(r"D:\repo\src\lib.rs"), Path(r"C:\repo"))
        with self.assertRaises(ValueError):
            relative_module_path(Path(r"C:\other\lib.rs"), Path(r"C:\repo"))

    def test_posix_absolute_paths_keep_case_when_ntpath_also_calls_them_absolute(self) -> None:
        self.assertEqual(
            "src/CaseSensitive.rs",
            relative_module_path(
                PurePosixPath("/home/User/Repo/src/CaseSensitive.rs"),
                PurePosixPath("/home/User/Repo"),
            ),
        )


if __name__ == "__main__":
    unittest.main()
