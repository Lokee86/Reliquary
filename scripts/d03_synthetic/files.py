from __future__ import annotations

import hashlib
import json
import os
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
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise D03Error(f"invalid UTF-8 JSON: {error}") from error


def checked_root(raw_path: str | Path, *, create: bool = False) -> Path:
    path = Path(raw_path)
    if not path.is_absolute():
        raise D03Error("synthetic root must be an absolute path")
    temp_root = Path(tempfile.gettempdir()).resolve()
    if path.parent.resolve() != temp_root or not path.name.startswith("d03-synthetic-"):
        raise D03Error("synthetic root must be a new d03-synthetic-* child of the OS temp directory")
    if path.exists() and path.is_symlink():
        raise D03Error("synthetic root must not be a symlink")
    if create:
        if path.exists():
            raise D03Error("synthetic root already exists; stage is create-only")
    else:
        try:
            valid = path.is_dir() and (path / ROOT_MARKER).read_bytes() == ROOT_MARKER_BYTES
        except OSError:
            valid = False
        if not valid:
            raise D03Error("synthetic root marker missing or invalid")
    return path


def write_new(path: Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        with path.open("xb") as stream:
            stream.write(data)
            stream.flush()
    except FileExistsError as error:
        raise D03Error(f"create-only destination already exists: {path.name}") from error


def atomic_write(path: Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temp_path = path.with_name(f".{path.name}.{uuid.uuid4().hex}.tmp")
    try:
        with temp_path.open("xb") as stream:
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temp_path, path)
    finally:
        if temp_path.exists():
            temp_path.unlink()


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
        if current.is_symlink():
            raise D03Error(f"symlink path component is forbidden: {relative}")
    try:
        resolved = current.resolve(strict=True)
        generation_root = generation.resolve(strict=True)
    except OSError as error:
        raise D03Error(f"missing generation member: {relative}") from error
    if generation_root not in resolved.parents or not resolved.is_file():
        raise D03Error(f"generation member escaped or is not a file: {relative}")
    return resolved