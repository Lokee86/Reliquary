#!/usr/bin/env python3
"""Supplement graph enforcement with physical execution/capability guards."""
from pathlib import Path
import re
import sys
from check_multiplexing_phase4_coverage import masked, closing

ROOT = Path(__file__).resolve().parents[1]
LOWER = {"container", "container_version", "Container", "ContainerError", "ObjectRef"}


def check(source):
    code = masked(source)
    errors = []
    for name in re.findall(r"\bcrate\s*::\s*(\w+)", code):
        if name not in LOWER:
            errors.append(f"unlisted physical dependency: {name}")
    for group in re.findall(r"\buse\s+crate\s*::\s*\{([^}]+)\}", code):
        for name in re.findall(r"\b\w+\b", group):
            if name not in LOWER:
                errors.append(f"unlisted physical contract: {name}")
    if re.search(r"\bFn(?:Mut|Once)?\b", code):
        errors.append("physical boundary cannot invoke caller semantic callbacks")
    if re.search(r"\bpub(?:\([^)]*\))?\s+(?:file\s*:|fn\s+\w+[^\n]*->\s*&(?:mut\s+)?File)", code):
        errors.append("physical file handle escapes its capability")
    for declaration in re.finditer(r"\bpub(?:\([^)]*\))?\s+(?:async\s+)?fn\s+\w+[^;{]*", code):
        signature = declaration.group()
        if "->" in signature and re.search(
            r"\b(?:File|RawHandle|RawFd|OwnedHandle|OwnedFd|BorrowedHandle|BorrowedFd)\b",
            signature.split("->", 1)[1],
        ):
            errors.append("physical file handle escapes its capability")
    if re.search(r"\bpub(?:\([^)]*\))?\s+\w+\s*:\s*[^,;\n]*\bFile\b", code):
        errors.append("physical file handle escapes its capability")
    if re.search(r"\b(?:open_scanned|recover_truncated_tail)\b", code):
        errors.append("obsolete implicit recovery or callback scanner")
    for function in re.finditer(r"\bfn\s+\w+[^;{}]*\{", code):
        start = function.end() - 1
        body = code[start:closing(code, start) + 1]
        if re.search(r"\bself\s*\.\s*file\s*\.\s*(?:write_all|set_len)\s*\(", body):
            if not re.search(r"\bself\s*\.\s*require_(?:write_admission|writer_lease)\s*\(", body):
                errors.append("physical mutation lacks writer admission")
            if not re.search(r"\bself\s*\.\s*mutation_gate\s*\(", body):
                errors.append("physical mutation bypasses held-read exclusion")
    return errors


def main():
    # Noncyclic upward dependencies must fail just like cyclic ones.
    assert check("fn f() { crate::memory_store::read(); }")
    assert check("use crate::{Container, Cva};")
    assert check("fn visit(callback: impl FnMut()) { callback(); }")
    assert check("pub(crate) file: File,")
    assert not check('use crate::{Container, ObjectRef}; // crate::Cva\n')
    assert not check('fn f() { let text = "crate::Cva"; }')
    assert check("fn bad() { self.file.set_len(0); }")
    assert not check("fn good() { self.require_write_admission(); self.mutation_gate(); self.file.set_len(0); }")
    assert check("pub fn into_file(self) -> File { self.file }")
    assert check("pub(crate) fn clone_file(&self)\n -> Result<std::fs::File, Error> { todo!() }")
    assert check("pub backing: std::sync::Arc<std::fs::File>,")
    assert check("pub fn raw(&self) -> std::os::windows::io::RawHandle { todo!() }")
    assert not check("pub fn metadata(&self) -> Result<std::fs::Metadata, Error> { todo!() }")
    errors = []
    for path in (ROOT / "src/container").glob("*.rs"):
        if "tests" in path.stem:
            continue
        errors.extend(f"{path.relative_to(ROOT)}: {error}" for error in check(path.read_text(encoding="utf-8")))
    for name in ("cva_lifecycle.rs", "phylactery_lifecycle.rs"):
        source = masked((ROOT / "src/facade" / name).read_text(encoding="utf-8"))
        if re.search(r"\.(?:recover|repair_retired_entity_references)\s*\(", source):
            errors.append(f"{name}: implicit open-time maintenance")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Physical boundary: allowlist, callback, handle escape and open-purity guards passed; thirteen fixtures passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
