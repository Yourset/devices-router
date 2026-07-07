from __future__ import annotations

import ctypes
from ctypes import wintypes


class POINT(ctypes.Structure):
    _fields_ = [("x", wintypes.LONG), ("y", wintypes.LONG)]


ctypes.windll.user32.GetCursorPos.argtypes = (ctypes.POINTER(POINT),)
ctypes.windll.user32.GetCursorPos.restype = wintypes.BOOL


def get_cursor_pos() -> tuple[int, int]:
    point = POINT()
    if not ctypes.windll.user32.GetCursorPos(ctypes.byref(point)):
        raise ctypes.WinError()
    return int(point.x), int(point.y)
