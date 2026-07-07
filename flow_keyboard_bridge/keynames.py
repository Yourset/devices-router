from __future__ import annotations


SPECIAL_ALIASES = {
    "alt_l": "alt",
    "alt_r": "alt",
    "backspace": "backspace",
    "caps_lock": "caps_lock",
    "cmd": "win",
    "cmd_l": "win",
    "cmd_r": "win",
    "ctrl_l": "ctrl",
    "ctrl_r": "ctrl",
    "delete": "delete",
    "down": "down",
    "end": "end",
    "enter": "enter",
    "esc": "esc",
    "f1": "f1",
    "f2": "f2",
    "f3": "f3",
    "f4": "f4",
    "f5": "f5",
    "f6": "f6",
    "f7": "f7",
    "f8": "f8",
    "f9": "f9",
    "f10": "f10",
    "f11": "f11",
    "f12": "f12",
    "home": "home",
    "insert": "insert",
    "left": "left",
    "page_down": "page_down",
    "page_up": "page_up",
    "right": "right",
    "shift": "shift",
    "shift_l": "shift",
    "shift_r": "shift",
    "space": "space",
    "tab": "tab",
    "up": "up",
}


def normalize_key_name(raw: str) -> str:
    value = str(raw)
    if len(value) >= 3 and value[0] == "'" and value[-1] == "'":
        return value[1:-1]
    if value.startswith("Key."):
        suffix = value[4:]
        return SPECIAL_ALIASES.get(suffix, suffix)
    return value

