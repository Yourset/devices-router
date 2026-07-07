from __future__ import annotations

import ctypes
from ctypes import wintypes


KEY_NAME_TO_VK = {
    "backspace": 0x08,
    "tab": 0x09,
    "enter": 0x0D,
    "shift": 0x10,
    "ctrl": 0x11,
    "alt": 0x12,
    "caps_lock": 0x14,
    "esc": 0x1B,
    "space": 0x20,
    "page_up": 0x21,
    "page_down": 0x22,
    "end": 0x23,
    "home": 0x24,
    "left": 0x25,
    "up": 0x26,
    "right": 0x27,
    "down": 0x28,
    "insert": 0x2D,
    "delete": 0x2E,
    "win": 0x5B,
    **{f"f{i}": 0x6F + i for i in range(1, 13)},
}

INPUT_KEYBOARD = 1
KEYEVENTF_SCANCODE = 0x0008
KEYEVENTF_UNICODE = 0x0004
KEYEVENTF_KEYUP = 0x0002
MAPVK_VK_TO_VSC = 0
ULONG_PTR = ctypes.c_ulonglong if ctypes.sizeof(ctypes.c_void_p) == 8 else ctypes.c_ulong


class KEYBDINPUT(ctypes.Structure):
    _fields_ = [
        ("wVk", wintypes.WORD),
        ("wScan", wintypes.WORD),
        ("dwFlags", wintypes.DWORD),
        ("time", wintypes.DWORD),
        ("dwExtraInfo", ULONG_PTR),
    ]


class MOUSEINPUT(ctypes.Structure):
    _fields_ = [
        ("dx", wintypes.LONG),
        ("dy", wintypes.LONG),
        ("mouseData", wintypes.DWORD),
        ("dwFlags", wintypes.DWORD),
        ("time", wintypes.DWORD),
        ("dwExtraInfo", ULONG_PTR),
    ]


class HARDWAREINPUT(ctypes.Structure):
    _fields_ = [
        ("uMsg", wintypes.DWORD),
        ("wParamL", wintypes.WORD),
        ("wParamH", wintypes.WORD),
    ]


class INPUT_UNION(ctypes.Union):
    _fields_ = [
        ("mi", MOUSEINPUT),
        ("ki", KEYBDINPUT),
        ("hi", HARDWAREINPUT),
    ]


class INPUT(ctypes.Structure):
    _fields_ = [("type", wintypes.DWORD), ("union", INPUT_UNION)]


ctypes.windll.user32.SendInput.argtypes = (wintypes.UINT, ctypes.POINTER(INPUT), ctypes.c_int)
ctypes.windll.user32.SendInput.restype = wintypes.UINT
ctypes.windll.user32.MapVirtualKeyW.argtypes = (wintypes.UINT, wintypes.UINT)
ctypes.windll.user32.MapVirtualKeyW.restype = wintypes.UINT


def key_name_to_vk(key: str) -> int | None:
    if key.startswith("<") and key.endswith(">"):
        try:
            return int(key[1:-1])
        except ValueError:
            return None
    if len(key) == 1:
        vk = ctypes.windll.user32.VkKeyScanW(ord(key))
        if vk == -1:
            return None
        return vk & 0xFF
    return KEY_NAME_TO_VK.get(key)


def send_key_event(key: str, is_down: bool) -> None:
    if len(key) == 1:
        flags = KEYEVENTF_UNICODE | (0 if is_down else KEYEVENTF_KEYUP)
        _send_input(0, ord(key), flags)
        return

    vk = key_name_to_vk(key)
    if vk is None:
        print(f"[client] unsupported key ignored: {key}")
        return

    scan_code = ctypes.windll.user32.MapVirtualKeyW(vk, MAPVK_VK_TO_VSC)
    flags = KEYEVENTF_SCANCODE | (0 if is_down else KEYEVENTF_KEYUP)
    _send_input(0, scan_code, flags)


def _send_input(vk: int, scan_code: int, flags: int) -> None:
    event = INPUT(
        type=INPUT_KEYBOARD,
        union=INPUT_UNION(ki=KEYBDINPUT(vk, scan_code, flags, 0, 0)),
    )
    sent = ctypes.windll.user32.SendInput(1, ctypes.byref(event), ctypes.sizeof(event))
    if sent != 1:
        raise ctypes.WinError()
