from __future__ import annotations

from dataclasses import dataclass
import json


@dataclass(frozen=True)
class KeyEvent:
    action: str
    key: str


@dataclass(frozen=True)
class PingEvent:
    message: str = "ok"


@dataclass(frozen=True)
class MouseActivityEvent:
    source: str


BridgeEvent = KeyEvent | PingEvent | MouseActivityEvent


def encode_message(event: BridgeEvent) -> bytes:
    if isinstance(event, PingEvent):
        payload = {"type": "ping", "message": event.message}
        return (json.dumps(payload, separators=(",", ":")) + "\n").encode("utf-8")
    if isinstance(event, MouseActivityEvent):
        payload = {"type": "mouse_activity", "source": event.source}
        return (json.dumps(payload, separators=(",", ":")) + "\n").encode("utf-8")
    if event.action not in {"down", "up"}:
        raise ValueError(f"Unsupported action: {event.action}")
    payload = {"type": "key", "action": event.action, "key": event.key}
    return (json.dumps(payload, separators=(",", ":")) + "\n").encode("utf-8")


def decode_message(payload: bytes) -> BridgeEvent:
    data = json.loads(payload.decode("utf-8").strip())
    message_type = data.get("type")
    if message_type == "ping":
        return PingEvent(str(data.get("message", "")))
    if message_type == "mouse_activity":
        source = data.get("source")
        if source not in {"host", "remote"}:
            raise ValueError(f"Unsupported mouse activity source: {source}")
        return MouseActivityEvent(source=source)
    if message_type != "key":
        raise ValueError(f"Unsupported message type: {message_type}")
    action = data.get("action")
    if action not in {"down", "up"}:
        raise ValueError(f"Unsupported action: {action}")
    key = data.get("key")
    if not isinstance(key, str) or not key:
        raise ValueError("Key must be a non-empty string")
    return KeyEvent(action=action, key=key)
