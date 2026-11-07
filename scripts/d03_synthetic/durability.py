from __future__ import annotations

import os
from pathlib import Path

from .files import D03Error, reject_reparse_chain


def sync_directory(path: Path) -> None:
    reject_reparse_chain(path, "directory path")
    if not path.is_dir():
        raise D03Error("durability target is not a directory")
    try:
        if os.name == "nt":
            _sync_windows_directory(path)
        else:
            descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
            try:
                os.fsync(descriptor)
            finally:
                os.close(descriptor)
    except (OSError, ValueError, TypeError) as error:
        raise D03Error(f"directory durability sync is unavailable for {path}: {error}") from error


def _sync_windows_directory(path: Path) -> None:
    import ctypes

    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    create_file = kernel.CreateFileW
    create_file.argtypes = [
        ctypes.c_wchar_p, ctypes.c_uint32, ctypes.c_uint32, ctypes.c_void_p,
        ctypes.c_uint32, ctypes.c_uint32, ctypes.c_void_p,
    ]
    create_file.restype = ctypes.c_void_p
    flush = kernel.FlushFileBuffers
    flush.argtypes = [ctypes.c_void_p]
    flush.restype = ctypes.c_int
    close = kernel.CloseHandle
    close.argtypes = [ctypes.c_void_p]
    handle = create_file(
        str(path), 0x80000000 | 0x40000000, 0x1 | 0x2 | 0x4,
        None, 3, 0x02000000, None,
    )
    if handle in (None, ctypes.c_void_p(-1).value):
        raise OSError(ctypes.get_last_error(), "could not open directory for durable sync")
    try:
        if not flush(handle):
            raise OSError(ctypes.get_last_error(), "directory flush failed")
    finally:
        close(handle)


def make_directory(path: Path) -> None:
    try:
        path.mkdir()
    except OSError as error:
        raise D03Error(f"could not create synthetic directory {path.name}: {error}") from error
    sync_directory(path.parent)
    sync_directory(path)


def durable_replace(source: Path, destination: Path) -> None:
    if source.parent != destination.parent:
        raise D03Error("synthetic atomic replacement must stay within one directory")
    reject_reparse_chain(source, "replacement source")
    reject_reparse_chain(destination, "replacement destination")
    try:
        os.replace(source, destination)
    except (OSError, ValueError) as error:
        raise D03Error(f"atomic replacement failed: {error}") from error
    sync_directory(destination.parent)
