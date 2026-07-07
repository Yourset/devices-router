import json

from flow_keyboard_bridge.protocol import KeyEvent, PingEvent, decode_message, encode_message


def test_key_event_round_trips_through_json_line():
    event = KeyEvent(action="down", key="a")

    payload = encode_message(event)

    assert payload.endswith(b"\n")
    assert decode_message(payload) == event


def test_decode_rejects_unknown_action():
    payload = (json.dumps({"type": "key", "action": "tap", "key": "a"}) + "\n").encode()

    try:
        decode_message(payload)
    except ValueError as exc:
        assert "Unsupported action" in str(exc)
    else:
        raise AssertionError("decode_message should reject unsupported actions")


def test_ping_event_round_trips_through_json_line():
    event = PingEvent()

    payload = encode_message(event)

    assert payload.endswith(b"\n")
    assert decode_message(payload) == event
