#!/usr/bin/env python3
"""Audit concrete REL/PHY durable record families and their physical writers."""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
# Exact known record identities, including persisted compatibility inputs.
# No prefix rule may silently classify a newly introduced record family.
FAMILIES = {
    "container_header": [r"CVA\0\r\n\x1a\n"],
    "global_ticket": ["CVAVERS1", "CVAVERS2"],
    "archive_publication": ["CVAAFMT2", "CVAAREC1", "CVAARP02"],
    "publication_envelope": ["CVAPUB01"],
    "archive_content": ["CVACONT1"],
    "archive_node": ["CVANODE1", "CVANODE2", "CVANOD02"],
    "archive_branch": ["CVABRCH1", "CVABRC02"],
    "archive_conversation": ["CVACONV1", "CVACONV2", "CVACONV3", "CVACON04"],
    "archive_terminal": ["CVACTMB1", "CVACTM02"],
    "archive_fragment": ["CVAFRAG1", "CVAFRA02"],
    "archive_episode": ["CVAEPIS1", "CVAEPI02"],
    "archive_file": ["CVAFILE1", "CVAFIL02"],
    "archive_turn_attachment": ["CVATURN1", "CVATURN2", "CVATUR03"],
    "archive_file_memory_link": ["CVAFMEM1", "CVAFME02"],
    "archive_import_receipt": ["CVAIMPT1", "CVAIMP02"],
    "archive_prune": ["CVAPRND1"],
    "conversation_compaction": [r"CVACMP1\0"],
    "interaction_stream": ["CVAISTR1", "CVAISTR2"],
    "echo": ["CVAECHO1"],
    "project_correlation": ["PRJCOR01", "PRJCOR02"],
    "project_file_binding": ["PRJFILE1"],
    "memory_format": ["CVAMEMF2"],
    "memory_body": ["CVAMBDY1"],
    "memory_revision": [*[f"CVAMEMR{i}" for i in range(2, 9)], "CVAMC001", "CVAMD001", "CVAMC002", "CVAMD002", "CVAMT002"],
    "memory_publication": ["CVAMEMV1"],
    "memory_terminal": ["CVAMEMT1"],
    "memory_routing": ["CVAMRTE1", "CVAMRTE2"],
    "memory_prune": ["CVAMPRN1"],
    "processing_epoch": ["CVAPEP01"],
    "entity": ["CVAENTF1", "CVAENTR1", "CVAENTR2", "CVAENTV1", "CVAENTT1", "CVAENTT2", "CVAENC01", "CVAEND01", "CVAENC02", "CVAEND02", "CVAETD02", "CVAETM02"],
    "entity_prune": ["CVAEPRN1"],
    "entity_resolution": ["CVAERS01", "CVAERS02", "CVAERS03", "CVAERL01"],
    "relationship": ["CVARELF1", "CVARELR1", "CVARELV1", "CVARELT1", "CVARNC01", "CVARND01", "CVARNC02", "CVARND02", "CVARTD02"],
    "relationship_prune": ["CVARPRN1"],
    "graph": ["CVAGFMT1", "CVAGNODE", "CVAGNOD2", "CVAGMUT1", "CVAGMUT2", "CVAGBAT1", "CVAGBAT2", "CVAGVER1", "CVAGCM01", "CVAGCB01", "CVAGCM02", "CVAGCB02"],
    "graph_rewrite_baseline": ["CVAGBL01"],
    "community_membership": ["CVACOMM1"],
    "community_name": ["CVACNAM1"],
    "dream_pairs": ["CVADRP01", "CVADRM01", "CVADRP02"],
    "dream_freshness": ["CVDREAM1", "CVDREAM2"],
    "legacy_dream_cooldown": ["CVADREM1", "CVADREM2"],
    "freshness_policy": ["CVAFRP01"],
    "freshness_events": ["CVAFRS02"],
    "freshness_import_settlement": ["CVAFRS03", "CVAFRS04", "CVAFRS05", "CVAFRS06"],
    "freshness_legacy_activation": ["CVAFSA01"],
    "freshness_rewrite_baseline": ["CVAFRB01"],
    "import_epoch": ["CVAIEPS1", "CVAIEPC1"],
    "import_legacy_epoch": ["CVAILEG1", "CVAILEG2"],
    "insomnia_format": ["CVAINSF1"],
    "insomnia_work": ["CVAINSW1", "CVAINSA1"],
    "insomnia_completion": [f"CVAINSC{i}" for i in range(1, 9)],
    "external_index": ["CVAXIDX1", "CVAXIDX2"],
    "external_publication": ["CVAXMEM1", "CVAXMEM2"],
    "ego_compact_history": ["CVAECR01", "CVAEDL01", "CVAETM01"],
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
EXCLUDED = {"CVAXPLN1": "repository-external operational frozen plan, not REL/PHY storage",
            "CVAINSC0": "encoder template; version byte replaced with 1..8 before persistence"}


def production_text(path: Path) -> str:
    # Inline test modules follow production items in this repository. Dedicated
    # test/support and benchmark files are excluded by the caller.
    return re.split(r"#\[cfg\(test\)\]\s*mod\s+\w+\s*\{",
                    path.read_text(encoding="utf-8"))[0]


# Exact identities deleted by the canonical-format hard cut. Reappearance in
# production source is an audit failure, even if no writer has yet been added.
RETIRED_IDENTITIES = {
    "CVADREM1", "CVADREM2", "CVADRP01", "CVAEANC1", "CVAEIDN1", "CVAEIDN2",
    "CVAEPER1", "CVAEPRN1", "CVAESYN1", "CVAFRS03", "CVAFRS04", "CVAFRS05",
    "CVAILEG1", "CVAILEG2", "CVARPRN1", "CVAXIDX1", "CVAXMEM1", "PRJCOR01",
}
FAMILIES = {family: [magic for magic in magics if magic not in RETIRED_IDENTITIES]
            for family, magics in FAMILIES.items()}
FAMILIES = {family: magics for family, magics in FAMILIES.items() if magics}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT / "benchmarks/history-durable-family-current-2026-10-21.json")
    args = parser.parse_args()
    known = {magic: family for family, magics in FAMILIES.items() for magic in magics}
    assert len(known) == sum(map(len, FAMILIES.values())), "duplicate family identity"
    found: dict[str, list[str]] = {}
    physical_sites = []
    for path in sorted((ROOT / "src").rglob("*.rs")):
        if "test" in path.name or "bench" in path.name or path.name == "authority_admission.rs":
            continue
        text = production_text(path)
        relative = path.relative_to(ROOT).as_posix()
        for literal in re.findall(r'b"((?:\\.|[^"\\])*)"', text):
            try:
                raw = literal.encode("ascii").decode("unicode_escape").encode("latin1")
            except (UnicodeError, ValueError):
                continue
            if len(raw) == 8 and raw.startswith((b"CVA", b"CVD", b"PRJ")):
                if literal in RETIRED_IDENTITIES:
                    raise ValueError(f"retired record identity reintroduced: {literal}: {relative}")
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
    report = dict(schema=2, retired_identities=sorted(RETIRED_IDENTITIES), families=FAMILIES, occurrences=dict(sorted(found.items())),
                  direct_container_append_sites=physical_sites, excluded=EXCLUDED,
                  limitations=["Lexical direct-call evidence supplements the reviewed writer table; it is not a Rust call graph.",
                               "Presence includes stable comparison schemas and test-only fixture helpers; it does not imply runtime format admission.",
                               "Delegating methods, multiline receiver chains and in-place compaction writes are covered by the execution evidence writer table.",
                               "Config, master-key files, gateway journals and frozen external plans are outside REL/PHY history."])
    out = args.output
    out.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"classified {len(known)} record identities in {len(FAMILIES)} families; {len(physical_sites)} direct append sites")


if __name__ == "__main__":
    main()
