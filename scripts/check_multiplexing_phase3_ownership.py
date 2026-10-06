"""Enforce the concrete P3 conversation owners and reject processing bypasses.

This source guard complements runtime recovery tests; it is not a Rust parser or
a substitute for the structural architecture check.
"""
import argparse
import json
import re
from pathlib import Path

PARENT = "runtime_host_gateway_conversation.rs"
CONTINUATION = "runtime_host_gateway_conversation_continuation.rs"
FINALIZATION = "runtime_host_gateway_conversation_finalization.rs"
# Strip comments and string bodies before inspecting structural source tokens.
LEXICAL = re.compile(r'//[^\n]*|/\*[\s\S]*?\*/|r(?P<hash>\#*)"[\s\S]*?"(?P=hash)|"(?:\\.|[^"\\])*"', re.M)


def production_source(text):
    masked = LEXICAL.sub(lambda m: re.sub(r"[^\n]", " ", m.group()), text)
    # Remove explicitly test-only items, including uses, functions and modules.
    for match in reversed(list(re.finditer(r"#\[cfg\(test\)\]", masked))):
        start = match.start()
        field = re.match(r"\s*(?:pub(?:\([^)]*\))?\s+)?[a-z_]\w*\s*:", masked[match.end():])
        if field:
            end = match.end() + field.end()
            depth = 0
            while end < len(masked):
                token = masked[end]
                if token in "(<[": depth += 1
                elif token in ")>]":
                    if token != ">" or masked[end - 1] != "-": depth -= 1
                elif token == "," and depth == 0:
                    end += 1
                    break
                end += 1
            masked = masked[:start] + re.sub(r"[^\n]", " ", masked[start:end]) + masked[end:]
            continue
        variant = re.match(r"\s*[A-Z]\w*\s*,", masked[match.end():])
        if variant:
            end = match.end() + variant.end()
            masked = masked[:start] + re.sub(r"[^\n]", " ", masked[start:end]) + masked[end:]
            continue
        boundary = re.search(r"[{;]", masked[match.end():])
        if boundary is None:
            continue
        end = match.end() + boundary.end()
        if masked[end - 1] == "{":
            depth = 1
            while end < len(masked) and depth:
                depth += (masked[end] == "{") - (masked[end] == "}")
                end += 1
        masked = masked[:start] + re.sub(r"[^\n]", " ", masked[start:end]) + masked[end:]
    return masked


# Exact owner paths are intentionally concrete, without another registry.
RULES = {
    "coordinator_definition": (r"\bstruct\s+ConversationCoordinator\b", CONTINUATION, 1),
    "registry_definition": (r"\bstruct\s+ConversationRegistry\b", PARENT, 1),
    "registry_construction": (r"\bConversationRegistry\s*::\s*new\s*\(", "runtime_host_gateway.rs", 1),
    "coordinator_construction": (r"\bConversationCoordinator\s*::\s*new\s*\(", PARENT, 1),
    "journal_open": (r"\bRelJournal\s*::\s*open\s*\(", PARENT, 1),
    "finalize_transition": (r"\.\s*claim_finalize\s*\(", PARENT, 1),
    "branch_policy": (r"\bConversationPolicy\s*::", FINALIZATION, None),
    "exact_test_bypass": (r"\b(?:process_exact_conversation_submission_for_test|process_accepted_conversation_submission|acquire_exact_for_test|FreshForTest)\b", None, 0),
    "old_processing_api": (r"\bprocess_conversation_submission\s*\(", None, 0),
}


def inventory(sources):
    references = []
    violations = []
    for rule, (pattern, owner, count) in RULES.items():
        rows = []
        for name, text in sorted(sources.items()):
            for match in re.finditer(pattern, production_source(text)):
                row = {"file": name, "line": text.count("\n", 0, match.start()) + 1, "rule": rule}
                rows.append(row)
                if owner is None or name != owner:
                    violations.append(row)
        references.extend(rows)
        if count is not None and len(rows) != count:
            violations.append({"rule": rule, "expected_count": count, "actual_count": len(rows)})
    return {"references": references, "violations": violations}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--enforce", action="store_true")
    args = parser.parse_args()
    sources = {p.name: p.read_text(encoding="utf-8")
               for p in (args.repo / "src/runtime_inference").glob("runtime_host_gateway*.rs")
               if "tests" not in p.name and "test_support" not in p.name}
    result = inventory(sources)
    print(json.dumps(result, indent=2))
    return int(args.enforce and bool(result["violations"]))


if __name__ == "__main__":
    raise SystemExit(main())
