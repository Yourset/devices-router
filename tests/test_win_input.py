import ctypes

from flow_keyboard_bridge.win_input import INPUT, key_name_to_vk


def test_virtual_key_number_in_angle_brackets_is_supported():
    assert key_name_to_vk("<50>") == 50


def test_character_key_maps_to_virtual_key():
    assert key_name_to_vk("a") is not None


def test_input_structure_has_windows_64_bit_size():
    if ctypes.sizeof(ctypes.c_void_p) == 8:
        assert ctypes.sizeof(INPUT) == 40
