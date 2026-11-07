"""Synthetic-only D01 framing helper; this is not an FRS07 decoder."""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass
from hashlib import sha256
import json
import struct


_MAGIC = b"D01SYN01"
_VERSION = 1
_HEADER = struct.Struct(">8sHHI")
_TRAILER_BYTES = 32
_MAX_FAMILY_BYTES = 128
_MAX_PAYLOAD_BYTES = 1_048_576

_FAMILY_FIELDS: dict[str, frozenset[str]] = {
    "synthetic.archive_turn.v1": frozenset({"conversation_id", "node_id", "text"}),
    "synthetic.memory_revision.v1": frozenset(
        {"memory_id", "revision", "title", "body"}
    ),
}


class SyntheticFrameError(ValueError):
    """A test envelope is malformed or outside its closed synthetic schema."""


class UnsupportedSyntheticFamily(SyntheticFrameError):
    """A test envelope names a family that this synthetic harness does not know."""


@dataclass(frozen=True)
class SyntheticFrame:
    family: str
    payload: dict[str, str]


def _reject_number(_token: str) -> None:
    raise SyntheticFrameError("numeric JSON tokens are not allowed in this fixture")


def _object_without_duplicates(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise SyntheticFrameError("duplicate JSON object key")
        result[key] = value
    return result


def _decode_payload(raw: bytes) -> dict[str, object]:
    try:
        text = raw.decode("utf-8", errors="strict")
    except UnicodeDecodeError as error:
        raise SyntheticFrameError("payload is not strict UTF-8") from error
    try:
        value = json.loads(
            text,
            object_pairs_hook=_object_without_duplicates,
            parse_int=_reject_number,
            parse_float=_reject_number,
            parse_constant=_reject_number,
        )
    except SyntheticFrameError:
        raise
    except json.JSONDecodeError as error:
        raise SyntheticFrameError("payload is not valid JSON") from error
    if not isinstance(value, dict):
        raise SyntheticFrameError("payload must be a JSON object")
    return value


def _validated_payload(family: str, payload: Mapping[str, object]) -> dict[str, str]:
    fields = _FAMILY_FIELDS.get(family)
    if fields is None:
        raise UnsupportedSyntheticFamily("family is not in the synthetic test registry")
    if not isinstance(payload, Mapping) or any(
        not isinstance(key, str) for key in payload
    ):
        raise SyntheticFrameError("payload keys must be strings")
    if set(payload) != fields:
        raise SyntheticFrameError("payload fields do not match the closed test schema")
    if any(not isinstance(payload[key], str) for key in fields):
        raise SyntheticFrameError("all fixture payload values must be strings")
    return {key: payload[key] for key in sorted(fields)}


def _frame_bytes_unchecked_for_tests(
    family: str, raw_payload: bytes, *, version: int = _VERSION
) -> bytes:
    """Pack a valid-checksum envelope for negative unit tests only."""
    family_bytes = family.encode("utf-8", errors="strict")
    if len(family_bytes) > 0xFFFF or len(raw_payload) > 0xFFFFFFFF:
        raise ValueError("test envelope is too large to frame")
    body = (
        _HEADER.pack(_MAGIC, version, len(family_bytes), len(raw_payload))
        + family_bytes
        + raw_payload
    )
    return body + sha256(body).digest()


def build_frame(family: str, payload: Mapping[str, object]) -> bytes:
    """Build one closed synthetic fixture frame; never builds a source owner."""
    normalized = _validated_payload(family, payload)
    try:
        family_bytes = family.encode("utf-8", errors="strict")
        payload_bytes = json.dumps(
            normalized, ensure_ascii=False, sort_keys=True, separators=(",", ":")
        ).encode("utf-8", errors="strict")
    except (UnicodeEncodeError, TypeError, ValueError) as error:
        raise SyntheticFrameError("fixture cannot be encoded") from error
    if len(family_bytes) > _MAX_FAMILY_BYTES:
        raise SyntheticFrameError("family identifier exceeds the test bound")
    if len(payload_bytes) > _MAX_PAYLOAD_BYTES:
        raise SyntheticFrameError("payload exceeds the test bound")
    body = _HEADER.pack(_MAGIC, _VERSION, len(family_bytes), len(payload_bytes))
    body += family_bytes + payload_bytes
    return body + sha256(body).digest()


def parse_frame(data: bytes) -> SyntheticFrame:
    """Parse only D01SYN01 fixtures; actual FRS07 input always fails closed."""
    if not isinstance(data, bytes) or len(data) < _HEADER.size + _TRAILER_BYTES:
        raise SyntheticFrameError("synthetic frame is truncated")
    magic, version, family_length, payload_length = _HEADER.unpack_from(data)
    if magic != _MAGIC:
        raise SyntheticFrameError("input is not a D01 synthetic frame")
    if version != _VERSION:
        raise SyntheticFrameError("unsupported synthetic frame version")
    if not 1 <= family_length <= _MAX_FAMILY_BYTES:
        raise SyntheticFrameError("family length is outside the test bound")
    if payload_length > _MAX_PAYLOAD_BYTES:
        raise SyntheticFrameError("payload length is outside the test bound")
    expected_length = _HEADER.size + family_length + payload_length + _TRAILER_BYTES
    if len(data) != expected_length:
        raise SyntheticFrameError("synthetic frame length does not match its header")
    body, supplied_digest = data[:-_TRAILER_BYTES], data[-_TRAILER_BYTES:]
    if supplied_digest != sha256(body).digest():
        raise SyntheticFrameError("synthetic frame digest mismatch")
    family_start = _HEADER.size
    payload_start = family_start + family_length
    try:
        family = data[family_start:payload_start].decode("utf-8", errors="strict")
    except UnicodeDecodeError as error:
        raise SyntheticFrameError("family identifier is not strict UTF-8") from error
    if family not in _FAMILY_FIELDS:
        raise UnsupportedSyntheticFamily("family is not in the synthetic test registry")
    payload = _validated_payload(family, _decode_payload(data[payload_start:-_TRAILER_BYTES]))
    return SyntheticFrame(family=family, payload=payload)
