"""Exercise custody rejection without changing any source or positive copy."""
import argparse
import json
from pathlib import Path
import shutil
import subprocess

from f03_verify_archived_oracle import PINS, require


def flip_at(path, offset):
    with path.open("r+b") as file:
        file.seek(offset)
        value = file.read(1)
        require(len(value) == 1, "negative fixture offset exceeds copied file")
        file.seek(offset)
        file.write(bytes((value[0] ^ 1,)))


def mutate_frs07(path):
    with path.open("rb") as file:
        file.seek(40)
        while prefix := file.read(8):
            size = int.from_bytes(prefix, "little")
            require(len(prefix) == 8 and 8 <= size <= 16 * 1024 * 1024,
                    "negative fixture source has invalid frame length")
            marker = file.read(8)
            if marker == b"CVAFRS07":
                offset = file.tell() + 16
                break
            file.seek(size - 8, 1)
        else:
            raise ValueError("negative fixture source has no FRS07 authority frame")
    flip_at(path, offset)


def mutate_intermediate_archive(path):
    with path.open("rb") as file:
        file.seek(40)
        while prefix := file.read(8):
            size = int.from_bytes(prefix, "little")
            require(len(prefix) == 8 and 8 <= size <= 16 * 1024 * 1024,
                    "negative fixture source has invalid frame length")
            start = file.tell()
            marker = file.read(8)
            if marker == b"CVAPUB01":
                coordinates = file.read(9)
                if coordinates[8] == 1:
                    file.seek(8, 1)
                body = file.read(8)
                if body == b"CVATUR03":
                    offset = file.tell()
                    require(file.tell() < path.stat().st_size // 2,
                            "expected a retained intermediate Archive turn")
                    break
            # Seek using the frame's beginning instead of interpreting any record body.
            file.seek(start + size)
        else:
            raise ValueError("negative fixture source has no intermediate Archive turn")
    flip_at(path, offset)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--exporter", type=Path, required=True)
    parser.add_argument("--custody", type=Path, required=True)
    parser.add_argument("--positive-copies", type=Path, required=True)
    parser.add_argument("--private-root", type=Path, required=True)
    args = parser.parse_args()
    root = args.private_root.resolve()
    positive = args.positive_copies.resolve()
    require(root.is_dir() and root != positive and positive not in root.parents,
            "negative root must be a private directory outside the positive copy tree")
    cases = {
        "unsupported-format": lambda path: flip_at(path, 8),
        "forged-owner-binding": lambda path: flip_at(path, 24),
        "truncated-tail": lambda path: truncate(path),
        "changed-frs07-authority": mutate_frs07,
        "changed-intermediate-archive-turn": mutate_intermediate_archive,
    }
    results = []
    for name, mutate in cases.items():
        cohort = root / name
        cohort.mkdir()
        for relative in PINS:
            copy = cohort / relative
            copy.parent.mkdir(exist_ok=True)
            shutil.copyfile(positive / relative, copy)
        mutate(cohort / "chatgpt-first14d/project.prj.rel")
        result = subprocess.run([
            str(args.exporter), "--source-root", str(cohort), "--expected-custody", str(args.custody),
            "--output", str(root / f"{name}-output"), "--verify-only",
        ], check=False, capture_output=True)
        require(result.returncode == 1 and not (root / f"{name}-output").exists(),
                f"negative {name} was not rejected by the exporter")
        with (root / f"{name}.stderr.log").open("xb") as log:
            log.write(result.stderr)
        results.append({"case": name, "exit": result.returncode})
    print(json.dumps({"status": "PASS", "scope": "negative copy custody rejection",
                      "negative_cases": results}, indent=2))


def truncate(path):
    with path.open("r+b") as file:
        file.truncate(path.stat().st_size - 1)


if __name__ == "__main__":
    main()
