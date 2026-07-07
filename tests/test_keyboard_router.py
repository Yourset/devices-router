from flow_keyboard_bridge.keyboard_router import KeyboardRouter, RawKeyEvent


def test_ctrl_alt_2_switches_to_remote_and_suppresses_hotkey():
    router = KeyboardRouter()

    assert router.handle(RawKeyEvent("down", 0x11)) is False
    assert router.handle(RawKeyEvent("down", 0x12)) is False
    assert router.handle(RawKeyEvent("down", 0x32)) is True

    assert router.remote_enabled is True


def test_ctrl_alt_1_switches_to_local_and_suppresses_hotkey():
    router = KeyboardRouter(remote_enabled=True)

    assert router.handle(RawKeyEvent("down", 0x11)) is True
    assert router.handle(RawKeyEvent("down", 0x12)) is True
    assert router.handle(RawKeyEvent("down", 0x31)) is True

    assert router.remote_enabled is False


def test_remote_mode_suppresses_normal_keys():
    router = KeyboardRouter(remote_enabled=True)

    assert router.handle(RawKeyEvent("down", 0x41)) is True


def test_local_mode_does_not_suppress_normal_keys():
    router = KeyboardRouter(remote_enabled=False)

    assert router.handle(RawKeyEvent("down", 0x41)) is False
