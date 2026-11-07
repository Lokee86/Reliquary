from __future__ import annotations

import hashlib
import json
import os
import stat
import tempfile
import uuid
from pathlib import Path, PurePosixPath
from typing import Any

ROOT_MARKER = ".d03-synthetic-root"
ROOT_MARKER_BYTES = b"reliquary-d03-synthetic-root-v1\n"


class D03Error(ValueError):
    pass


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(
        value, ensure_ascii=False, allow_nan=False, sort_keys=True, separators=(",", ":")
    ).encode("utf-8")


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _pairs_without_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise D03Error(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def parse_json(data: bytes) -> Any:
    try:
        return json.loads(
            data.decode("utf-8"),
            object_pairs_hook=_pairs_without_duplicates,
            parse_constant=lambda value: (_ for _ in ()).throw(
                D03Error(f"non-finite JSON value: {value}")
            ),
        )
    except D03Error:
        raise
    except (UnicodeDecodeError, ValueError, RecursionError) as error:
        raise D03Error(f"invalid UTF-8 JSON: {error}") from error


def read_bytes(path: Path, label: str) -> bytes:
    try:
        return path.read_bytes()
    except (OSError, ValueError) as error:
        raise D03Error(f"{label} could not be read: {error}") from error


def is_reparse_point(path: Path) -> bool:
    try:
        info = os.lstat(path)
    except FileNotFoundError:
        return False
    except (OSError, ValueError) as error:
        raise D03Error(f"path metadata could not be read: {error}") from error
    reparse_flag = getattr(stat, "FILE_ATTRIBUTE_REPARSE_POINT", 0x400)
    attributes = getattr(info, "st_file_attributes", 0)
    return stat.S_ISLNK(info.st_mode) or bool(attributes & reparse_flag)


def reject_reparse(path: Path, label: str) -> None:
    if is_reparse_point(path):
        raise D03Error(f"{label} must not be a symlink, junction, or reparse point")


def reject_reparse_chain(path: Path, label: str) -> None:
    current = path
    while True:
        reject_reparse(current, label)
        if current.parent == current:
            return
        current = current.parent


def sync_directory(path: Path) -> None:
    from .durability import sync_directory as sync

    sync(path)


def make_directory(path: Path) -> None:
    from .durability import make_directory as make

    make(path)


def durable_replace(source: Path, destination: Path) -> None:
    from .durability import durable_replace as replace

    replace(source, destination)


def checked_root(raw_path: str | Path, *, create: bool = False) -> Path:
    path = Path(raw_path)
    if not path.is_absolute():
        raise D03Error("synthetic root must be an absolute path")
    reject_reparse_chain(path, "synthetic root path")
    try:
        temp_root = Path(tempfile.gettempdir()).resolve(strict=True)
        parent = path.parent.resolve(strict=True)
    except (OSError, RuntimeError, ValueError) as error:
        raise D03Error(f"synthetic root parent could not be resolved: {error}") from error
    if parent != temp_root or not path.name.startswith("d03-synthetic-"):
        raise D03Error("synthetic root must be a new d03-synthetic-* child of the OS temp directory")
    if path.exists() and path.is_symlink():
        raise D03Error("synthetic root must not be a symlink")
    if create:
        if path.exists():
            raise D03Error("synthetic root already exists; stage is create-only")
    else:
        try:
            marker = path / ROOT_MARKER
            reject_reparse(marker, "synthetic root marker")
            valid = path.is_dir() and read_bytes(marker, "synthetic root marker") == ROOT_MARKER_BYTES
        except OSError:
            valid = False
        if not valid:
            raise D03Error("synthetic root marker missing or invalid")
    return path


def write_new(path: Path, data: bytes) -> None:
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        reject_reparse(path.parent, "synthetic file parent")
        reject_reparse(path, "synthetic file")
        with path.open("xb") as stream:
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())
        sync_directory(path.parent)
    except FileExistsError as error:
        raise D03Error(f"create-only destination already exists: {path.name}") from error
    except (OSError, ValueError) as error:
        raise D03Error(f"could not durably create synthetic file {path.name}: {error}") from error


def atomic_write(path: Path, data: bytes) -> None:
    temp_path = path.with_name(f".{path.name}.{uuid.uuid4().hex}.tmp")
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        reject_reparse(path.parent, "synthetic file parent")
        reject_reparse(path, "synthetic file")
        with temp_path.open("xb") as stream:
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())
        durable_replace(temp_path, path)
    except (OSError, ValueError) as error:
        raise D03Error(f"atomic durable replacement failed for {path.name}: {error}") from error
    finally:
        if temp_path.exists():
            try:
                temp_path.unlink()
            except OSError as error:
                raise D03Error(f"temporary synthetic file cleanup failed: {error}") from error


def member(generation: Path, relative: str) -> Path:
    if not isinstance(relative, str):
        raise D03Error("generation paths must be strings")
    parsed = PurePosixPath(relative)
    if (
        not relative
        or "\\" in relative
        or ":" in relative
        or parsed.is_absolute()
        or parsed.as_posix() != relative
        or any(part in ("", ".", "..") for part in parsed.parts)
    ):
        raise D03Error(f"noncanonical or escaped relative path: {relative!r}")
    current = generation
    for part in parsed.parts:
        current = current / part
        reject_reparse(current, f"generation path component {relative}")
    try:
        resolved = current.resolve(strict=True)
        generation_root = generation.resolve(strict=True)
    except (OSError, RuntimeError, ValueError) as error:
        raise D03Error(f"missing generation member: {relative}") from error
    if generation_root not in resolved.parents or not resolved.is_file():
        raise D03Error(f"generation member escaped or is not a file: {relative}")
    return resolved
