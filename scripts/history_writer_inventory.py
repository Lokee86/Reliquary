#!/usr/bin/env python3
"""Audit concrete REL/PHY durable record families and their physical writers."""
from __future__ import annotations

import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
# Exact known record identities, including persisted compatibility inputs.
# No prefix rule may silently classify a newly introduced record family.
FAMILIES = {
    "container_header": [r"CVA\0\r\n\x1a\n"],
    "global_ticket": ["CVAVERS1", "CVAVERS2"],
    "archive_publication": ["CVAAFMT2", "CVAAREC1"],
    "archive_content": ["CVACONT1"],
    "archive_node": ["CVANODE1", "CVANODE2"],
    "archive_branch": ["CVABRCH1"],
    "archive_conversation": ["CVACONV1", "CVACONV2", "CVACONV3"],
    "archive_terminal": ["CVACTMB1"],
    "archive_fragment": ["CVAFRAG1"],
    "archive_episode": ["CVAEPIS1"],
    "archive_file": ["CVAFILE1"],
    "archive_turn_attachment": ["CVATURN1", "CVATURN2"],
    "archive_file_memory_link": ["CVAFMEM1"],
    "archive_import_receipt": ["CVAIMPT1"],
    "archive_prune": ["CVAPRND1"],
    "conversation_compaction": [r"CVACMP1\0"],
    "interaction_stream": ["CVAISTR1"],
    "echo": ["CVAECHO1"],
    "project_correlation": ["PRJCOR01", "PRJCOR02"],
    "project_file_binding": ["PRJFILE1"],
    "memory_format": ["CVAMEMF2"],
    "memory_body": ["CVAMBDY1"],
    "memory_revision": [f"CVAMEMR{i}" for i in range(2, 9)],
    "memory_publication": ["CVAMEMV1"],
    "memory_terminal": ["CVAMEMT1"],
    "memory_routing": ["CVAMRTE1", "CVAMRTE2"],
    "memory_prune": ["CVAMPRN1"],
    "processing_epoch": ["CVAPEP01"],
    "entity": ["CVAENTF1", "CVAENTR1", "CVAENTR2", "CVAENTV1", "CVAENTT1", "CVAENTT2"],
    "entity_prune": ["CVAEPRN1"],
    "entity_resolution": ["CVAERS01"],
    "relationship": ["CVARELF1", "CVARELR1", "CVARELV1", "CVARELT1"],
    "relationship_prune": ["CVARPRN1"],
    "graph": ["CVAGFMT1", "CVAGNODE", "CVAGNOD2", "CVAGMUT1", "CVAGMUT2", "CVAGBAT1", "CVAGBAT2", "CVAGVER1"],
    "graph_rewrite_baseline": ["CVAGBL01"],
    "community_membership": ["CVACOMM1"],
    "community_name": ["CVACNAM1"],
    "dream_pairs": ["CVADRP01"],
    "dream_freshness": ["CVDREAM1"],
    "legacy_dream_cooldown": ["CVADREM1", "CVADREM2"],
    "freshness_policy": ["CVAFRP01"],
    "freshness_events": ["CVAFRS02"],
    "freshness_import_settlement": ["CVAFRS03", "CVAFRS04", "CVAFRS05", "CVAFRS06"],
    "freshness_legacy_activation": ["CVAFSA01"],
    "freshness_rewrite_baseline": ["CVAFRB01"],
    "import_epoch": ["CVAIEPS1", "CVAIEPC1"],
    "import_legacy_epoch": ["CVAILEG1"],
    "insomnia_format": ["CVAINSF1"],
    "insomnia_work": ["CVAINSW1", "CVAINSA1"],
    "insomnia_completion": [f"CVAINSC{i}" for i in range(1, 7)],
    "external_index": ["CVAXIDX1"],
    "external_publication": ["CVAXMEM1"],
    "ego_identity": ["CVAEIDN1", "CVAEIDN2", "CVAEIDA1"],
    "ego_personality": ["CVAEPER1"],
    "ego_anchor": ["CVAEANC1"],
    "ego_synthesis": ["CVAESYN1"],
    "packed_vectors": ["CVAPVFM1", "CVAPVEC1", "CVAPVRN1"],
    "memory_vectors": ["CVAMVFM1", "CVAMVEC1"],
    "archive_vectors": ["CVAAVFM1", "CVAAVEC1"],
    "compatibility_profile": ["CVACPFM1", "CVACPRO1", "CVACPRT1"],
    "vector_generation": ["CVAVGFM2", "CVAVGEN2", "CVAVGRC2"],
    "rel_metadata": ["CVARELM1"],
    "phy_profile": ["CVAPHYP1"],
    "owner_maintenance": ["CVAHPR01"],
    "rewrite_lineage": ["CVARGN01"],
    "legacy_workspace": ["CVAWKFM1", "CVAWKSP1"],
}
EXCLUDED = {"CVAXPLN1": "repository-external operational frozen plan, not REL/PHY storage"}


def production_text(path: Path) -> str:
    # Inline test modules follow production items in this repository. Dedicated
    # test/support and benchmark files are excluded by the caller.
    return re.split(r"#\[cfg\(test\)\]\s*mod\s+\w+\s*\{",
                    path.read_text(encoding="utf-8"))[0]


def main() -> None:
    known = {magic: family for family, magics in FAMILIES.items() for magic in magics}
    assert len(known) == sum(map(len, FAMILIES.values())), "duplicate family identity"
    found: dict[str, list[str]] = {}
    physical_sites = []
    for path in sorted((ROOT / "src").rglob("*.rs")):
        if "test" in path.name or "bench" in path.name:
            continue
        text = production_text(path)
        relative = path.relative_to(ROOT).as_posix()
        for literal in re.findall(r'b"((?:\\.|[^"\\])*)"', text):
            try:
                raw = literal.encode("ascii").decode("unicode_escape").encode("latin1")
            except (UnicodeError, ValueError):
                continue
            if len(raw) == 8 and raw.startswith((b"CVA", b"CVD", b"PRJ")):
                if literal not in known and literal not in EXCLUDED:
                    raise ValueError(f"unclassified record identity {literal}: {relative}")
                found.setdefault(literal, [])
                if relative not in found[literal]:
                    found[literal].append(relative)
        for match in re.finditer(r"\b(?:container|output)\s*\.\s*append\s*\(", text):
            physical_sites.append(dict(path=relative, line=text.count("\n", 0, match.start()) + 1))
    missing = set(known) - set(found)
    if missing:
        raise ValueError(f"inventory identities missing from current source: {sorted(missing)}")
    report = dict(schema=1, families=FAMILIES, occurrences=dict(sorted(found.items())),
                  direct_container_append_sites=physical_sites, excluded=EXCLUDED,
                  limitations=["Lexical direct-call evidence supplements the reviewed writer table; it is not a Rust call graph.",
                               "Delegating methods, multiline receiver chains and in-place compaction writes are covered by the execution evidence writer table.",
                               "Config, master-key files, gateway journals and frozen external plans are outside REL/PHY history."])
    out = ROOT / "benchmarks/history-durable-family-baseline-2026-10-19.json"
    out.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"classified {len(known)} record identities in {len(FAMILIES)} families; {len(physical_sites)} direct append sites")


if __name__ == "__main__":
    main()
