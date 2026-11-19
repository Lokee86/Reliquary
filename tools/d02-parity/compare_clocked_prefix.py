"""Check the paired migration's physical publication order and every global clock."""
from collections import Counter
from itertools import zip_longest
import hashlib
import json

from authentic_witness import require, witness_batches
from compare_authentic_graph import current_frames


SOURCE_PUBLICATIONS = {
    b"CVATUR03": "archive", b"CVABRC02": "archive", b"CVAEPI02": "archive",
    b"CVAFRA02": "archive", b"CVACON04": "archive",
    b"CVAMC002": "memory", b"CVAMD002": "memory",
    b"CVAGCB02": "graph", b"CVAGCM02": "graph",
    b"CVAENC02": "entity", b"CVAEND02": "entity",
    b"CVAETD02": "entity", b"CVAETM02": "entity",
}
TARGET_PUBLICATIONS = {
    b"RELAPB01": "archive", b"RELMCRT1": "memory", b"RELMDLT1": "memory",
    b"RELGPB01": "graph", b"RELEPB01": "entity", b"RELVGRC1": "vector",
}
NEUTRAL_PAIRS = (
    (b"CVACOMM1", b"RELCOM02", "community"),
    (b"CVAERS03", b"RELERH01", "resolution"),
    (b"CVAERL01", b"RELERL01", "inert_resolution"),
    (b"CVARELM1", b"RELRELM1", "rel_metadata"),
    (b"PRJCOR02", b"RELPRC01", "project_correlation"),
    (b"CVADRM01", b"RELDRM01", "dream_source"),
    (b"CVADRP02", b"RELDRP02", "dream_pair"),
    (b"CVDREAM1", b"CVDREAM1", "dream_event"),
    (b"CVDREAM2", b"CVDREAM2", "dream_settlement"),
    (b"CVAINSW1", b"RELINSW1", "insomnia_work"),
    (b"CVAMBDY1", b"RELMBOD1", "memory_body"),
    (b"CVAMRTE2", b"RELMRTE2", "routing"),
    (b"CVACPRO1", b"RELCPRO1", "embedding_profile"),
    (b"CVACPRT1", b"RELCPRT1", "profile_retirement"),
    (b"CVAPVEC1", b"RELPVEC1", "packed_vector"),
    (b"CVAMVEC1", b"RELMVEC1", "memory_vector"),
    (b"CVAAVEC1", b"RELAVEC1", "archive_vector"),
    (b"CVAVGEN2", b"RELVGEN1", "vector_generation"),
)


def old_frames(path, source, coverage_dir, artifacts):
    from f03_verify_independent_repeat import open_header, read_frame, regular, snapshot

    before = regular(path)
    kind = 1 if source["header_observation"]["owner_kind_observed_from_header"] == "REL" else 2
    with path.open("rb") as file:
        digest = open_header(file, kind, source)
        for frame in witness_batches(coverage_dir, source, artifacts, "frames", source["frame_stream"]):
            yield read_frame(file, frame, digest, before.st_size), int(frame["latest_global"])
        require(file.tell() == before.st_size and digest.hexdigest() == source["source_sha256"]
                and snapshot(before) == snapshot(regular(path)), "historical copy changed during comparison")


def events(frames, clock, legacy):
    from f03_legacy_frame_clock import integer
    from f03_verify_prefix_availability import file_binding

    prefix = b"CVA" if legacy else b"REL"
    neutral = {pair[0 if legacy else 1]: pair[2] for pair in NEUTRAL_PAIRS}
    publications = SOURCE_PUBLICATIONS if legacy else TARGET_PUBLICATIONS
    for raw, witnessed_head in frames:
        prior = clock.head
        visible = clock.consume(raw) if legacy else clock.consume_current(raw)
        require(witnessed_head is None or witnessed_head == visible,
                "source frame clock differs from its frozen witness")
        marker = raw[:8]
        if marker in (prefix + b"VERS1", prefix + b"VERS2"):
            yield ("reservation", visible, clock.times.get(visible))
        elif marker == prefix + b"PUB01":
            version = integer(raw, 8)
            mode = raw[16]
            family = publications.get(raw[25 if mode == 1 else 17:][:8])
            require(family is not None, "unknown clocked publication family")
            yield ("publication", family, visible, version, mode, clock.times.get(version))
        elif marker == b"CVAVGRC2" and legacy:
            version = integer(raw, 8)
            require(version in clock.reservations, "source vector activation lacks reservation")
            clock.reservations.remove(version)
            yield ("publication", "vector", visible, version, 2, clock.times.get(version))
        elif marker == prefix + b"INSC8":
            yield ("insomnia_group", raw[8], prior, visible,
                   clock.times.get(visible) if prior != visible else None)
        elif marker == b"PRJFILE1" and legacy:
            identity, (content, _) = file_binding(raw)
            yield ("project_backing", visible, identity, content)
        elif marker == b"RELPRB01" and not legacy:
            require(len(raw) == 80 and integer(raw, 8) == visible,
                    "project backing differs from its physical clock")
            yield ("project_backing", visible, raw[16:48].hex(), raw[48:80].hex())
        elif marker in neutral:
            yield (neutral[marker], visible)


def compare(source_path, source, owner, coverage_dir, artifacts, candidate, candidate_digest):
    from f03_compare_current_entity_community import CurrentClock
    from f03_legacy_frame_clock import LegacyFrameClock

    old = LegacyFrameClock()
    new = CurrentClock()
    expected = events(old_frames(source_path, source, coverage_dir, artifacts), old, True)
    observed = events(((frame, None) for frame in current_frames(candidate, owner, candidate_digest)), new, False)
    counts = Counter()
    archive_frontiers = [0]
    digest = hashlib.sha256()
    for ordinal, (left, right) in enumerate(zip_longest(expected, observed)):
        require(left == right, f"physical publication order or coordinate differs at event {ordinal}")
        counts[left[0]] += 1
        if right[:2] == ("publication", "archive"):
            archive_frontiers.append(right[2])
        digest.update(json.dumps(left, separators=(",", ":")).encode() + b"\n")
    require(old.head == new.head and old.times == new.times and old.reservations == new.reservations,
            "allocated clocks, times or unbound reservations differ")

    native = source["native"]
    points = 0
    for coordinate in witness_batches(coverage_dir, source, artifacts, "global", native["global_stream"]):
        require(int(coordinate["global_version"]) == points
                and coordinate["transaction_time_ns"] == new.times.get(points),
                f"candidate clock differs from frozen global cut {points}")
        points += 1
    require(points == new.head + 1, "candidate clock does not cover the frozen global population")
    return {
        "status": "PASS", "global_cuts": points, "unbound_reservations": len(new.reservations),
        "ordered_events_by_family": dict(counts), "ordered_event_sha256": digest.hexdigest(),
    }, archive_frontiers


def pinned_rows(path, expected):
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for line in handle:
            digest.update(line)
            yield json.loads(line)
    require(digest.hexdigest() == expected, "source availability witness changed")


def compare_archive_backing(directory, source_receipt, observed, archive_frontiers):
    target = {}
    for row in observed:
        key = hashlib.sha256(bytes.fromhex(row["attachment_id"])).hexdigest()
        require(key not in target, "duplicate native project binding")
        target[key] = row
    copy = source_receipt["copy_id"]
    intervals = pinned_rows(directory / f"{copy}-archive-reference-intervals.jsonl",
                            source_receipt["reference_intervals_sha256"])
    changes = Counter()
    bindings = turns = 0
    for source in intervals:
        local = int(source["archive_version"])
        visible = archive_frontiers[local]
        require(str(visible) == source["first_visible_global"],
                "native Archive reference publication differs from source frontier")
        if source["kind"] == "node_content":
            require(source["missing_backing_cuts_inclusive"] is None
                    and int(source["first_inline_global"]) <= visible,
                    "historical turn backing requires another native mapping")
            turns += 1
            continue
        require(source["kind"] == "file_content" and source["first_inline_global"] is None
                and source["invalid_file_identity_cuts_inclusive"] is None,
                "unrecognized retained Archive backing state")
        actual = target.pop(source["stable_key_sha256"], None)
        require(actual is not None and actual["archive_version"] == str(local)
                and actual["content_sha256"] == source["content_sha256"]
                and actual["global_version"] == source["first_project_binding_global"],
                "native Archive project binding differs from retained identity or clock")
        bind = int(actual["global_version"])
        missing = [str(visible), str(bind - 1)] if visible < bind else None
        require(missing == source["missing_backing_cuts_inclusive"],
                "native Archive project availability interval differs from source")
        if missing:
            changes[visible] += 1
            changes[bind] -= 1
        bindings += 1
    require(not target and bindings == source_receipt["archive_file_references"]
            and turns == source_receipt["archive_node_references"],
            "native Archive backing reference population differs")

    cuts = pinned_rows(directory / f"{copy}-prefix-availability.jsonl", source_receipt["cut_rows_sha256"])
    blocked = missing = total_missing = points = 0
    for point, row in enumerate(cuts):
        missing += changes[point]
        require(row["global_version"] == str(point)
                and row["missing_file_backing"] == str(missing)
                and row["missing_node_backing"] == row["invalid_file_identity"] == "0"
                and row["archive_backing_gate"] == ("blocked" if missing else "satisfied"),
                "native Archive backing gate differs from the frozen global cut")
        blocked += missing > 0
        total_missing += missing
        points += 1
    require(missing == 0 and points == source_receipt["global_cuts"]
            and blocked == source_receipt["archive_backing_blocked_cuts"],
            "native Archive backing gate does not cover the frozen cuts")
    return {
        "status": "PASS", "turn_references": turns, "project_attachment_references": bindings,
        "global_cuts": points, "backing_blocked_cuts": blocked,
        "missing_backing_reference_cuts": total_missing,
        "source_reference_intervals_sha256": source_receipt["reference_intervals_sha256"],
        "source_backing_cuts_sha256": source_receipt["cut_rows_sha256"],
    }
