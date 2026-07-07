from flow_keyboard_bridge.target_state import TargetState


def test_target_state_defaults_to_local():
    state = TargetState()

    assert state.remote_enabled is False


def test_target_state_switches_between_local_and_remote():
    state = TargetState()

    state.enable_remote()
    assert state.remote_enabled is True

    state.enable_local()
    assert state.remote_enabled is False
