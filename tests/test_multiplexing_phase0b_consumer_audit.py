"""Negative verification of Phase 0B's source-backed Warlock consumer audit."""
from __future__ import annotations

import pathlib
import shutil
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import verify_multiplexing_phase0b_consumers as audit  # noqa: E402


class WarlockSourceAuditTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory(prefix="g6_consumer_")
        self.addCleanup(self.tmp.cleanup)
        self.warlock = pathlib.Path(self.tmp.name)
        source = ROOT.parent / "Warlock-v2"
        if not (source / "src-tauri/Cargo.toml").is_file():
            self.fail("G6 requires a mounted Warlock-v2 checkout for pinned source verification")
        manifest = self.warlock / "src-tauri/Cargo.toml"
        manifest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source / "src-tauri/Cargo.toml", manifest)
        target = self.warlock / "src-tauri/src"
        target.mkdir(parents=True, exist_ok=True)
        inventory = set(audit.DIRECT_RELIQUARY_IMPORT_FILES)
        for files in audit.WARLOCK_SEAMS.values():
            inventory.update(files)
        for filename in inventory:
            shutil.copyfile(source / "src-tauri/src" / filename, target / filename)

    def test_frozen_current_consumer_source_matches_its_pins(self) -> None:
        result = audit.run(self.warlock)
        self.assertEqual(result["consumer"], "Warlock-v2")
        self.assertEqual(result["direct_arcana_revision"], result["transitive_arcana_revision"])

    def test_mismatched_direct_arcana_pin_fails_instead_of_hiding_duplicate_graph(self) -> None:
        manifest = self.warlock / "src-tauri/Cargo.toml"
        contents = manifest.read_text(encoding="utf-8")
        real_pin = audit.pin(contents, "arcana")
        manifest.write_text(contents.replace(real_pin, "0" * 40), encoding="utf-8")
        with self.assertRaisesRegex(AssertionError, "dependency conflict"):
            audit.run(self.warlock)

    def test_removing_old_host_seam_forces_consumer_inventory_refresh(self) -> None:
        path = self.warlock / "src-tauri/src/workspace.rs"
        path.write_text(
            path.read_text(encoding="utf-8").replace("ReliquaryRuntimeHost", "MigratedHost"),
            encoding="utf-8",
        )
        with self.assertRaisesRegex(AssertionError, "missing inspected integration landmarks"):
            audit.run(self.warlock)

    def test_new_direct_import_forces_a_complete_consumer_reaudit(self) -> None:
        (self.warlock / "src-tauri/src/new_reliquary_consumer.rs").write_text(
            "use reliquary_memory::Cva;\n", encoding="utf-8"
        )
        with self.assertRaisesRegex(AssertionError, "direct Reliquary import inventory changed"):
            audit.run(self.warlock)

    def test_missing_downstream_checkout_does_not_pass_the_source_audit(self) -> None:
        (self.warlock / "src-tauri/src/workspace.rs").unlink()
        with self.assertRaises(FileNotFoundError):
            audit.run(self.warlock)


if __name__ == "__main__":
    unittest.main()
