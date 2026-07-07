from flow_keyboard_bridge.keynames import normalize_key_name


def test_normalize_character_key():
    assert normalize_key_name("'a'") == "a"


def test_normalize_special_key_aliases():
    assert normalize_key_name("Key.space") == "space"
    assert normalize_key_name("Key.ctrl_l") == "ctrl"
    assert normalize_key_name("Key.alt_r") == "alt"


def test_unknown_special_key_keeps_suffix():
    assert normalize_key_name("Key.media_volume_up") == "media_volume_up"

