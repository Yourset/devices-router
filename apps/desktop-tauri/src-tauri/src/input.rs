#[cfg(windows)]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    MapVirtualKeyW, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT,
    KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE, KEYEVENTF_UNICODE, MAPVK_VK_TO_VSC,
    MOUSEEVENTF_HWHEEL, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN,
    MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP,
    MOUSEEVENTF_WHEEL, MOUSEINPUT, VIRTUAL_KEY,
};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::SetCursorPos;

use crate::mouse::virtual_screen_rect;
use crate::protocol::{MouseButton, MouseButtonAction, MouseInputEvent};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MouseFlagSet(u32);

impl MouseFlagSet {
    pub const MOVE: Self = Self(0x0001);
    pub const LEFTDOWN: Self = Self(0x0002);
    pub const LEFTUP: Self = Self(0x0004);
    pub const RIGHTDOWN: Self = Self(0x0008);
    pub const RIGHTUP: Self = Self(0x0010);
    pub const MIDDLEDOWN: Self = Self(0x0020);
    pub const MIDDLEUP: Self = Self(0x0040);
    pub const WHEEL: Self = Self(0x0800);
    pub const HWHEEL: Self = Self(0x01000);
}

pub fn text_payload(key: &str) -> Option<&str> {
    key.strip_prefix("text:").filter(|text| !text.is_empty())
}

pub fn key_name_to_vk(key: &str) -> Option<u16> {
    if let Some(raw) = key
        .strip_prefix('<')
        .and_then(|value| value.strip_suffix('>'))
    {
        return raw.parse::<u16>().ok();
    }
    match key {
        "backspace" => Some(0x08),
        "tab" => Some(0x09),
        "enter" => Some(0x0D),
        "shift" => Some(0x10),
        "ctrl" => Some(0x11),
        "alt" => Some(0x12),
        "esc" => Some(0x1B),
        "space" => Some(0x20),
        "left" => Some(0x25),
        "up" => Some(0x26),
        "right" => Some(0x27),
        "down" => Some(0x28),
        "delete" => Some(0x2E),
        _ => None,
    }
}

pub fn mouse_input_payload(event: &MouseInputEvent) -> Option<(i32, i32, i32, MouseFlagSet)> {
    match event {
        MouseInputEvent::MoveRelative { dx, dy } => Some((*dx, *dy, 0, MouseFlagSet::MOVE)),
        MouseInputEvent::MoveAbsolute { .. } | MouseInputEvent::MoveToLeftEdge { .. } => None,
        MouseInputEvent::Wheel { delta } => Some((0, 0, *delta, MouseFlagSet::WHEEL)),
        MouseInputEvent::HWheel { delta } => Some((0, 0, *delta, MouseFlagSet::HWHEEL)),
        MouseInputEvent::Button { button, action } => {
            Some((0, 0, 0, mouse_button_flag(button, action)))
        }
    }
}

fn mouse_button_flag(button: &MouseButton, action: &MouseButtonAction) -> MouseFlagSet {
    match (button, action) {
        (MouseButton::Left, MouseButtonAction::Down) => MouseFlagSet::LEFTDOWN,
        (MouseButton::Left, MouseButtonAction::Up) => MouseFlagSet::LEFTUP,
        (MouseButton::Right, MouseButtonAction::Down) => MouseFlagSet::RIGHTDOWN,
        (MouseButton::Right, MouseButtonAction::Up) => MouseFlagSet::RIGHTUP,
        (MouseButton::Middle, MouseButtonAction::Down) => MouseFlagSet::MIDDLEDOWN,
        (MouseButton::Middle, MouseButtonAction::Up) => MouseFlagSet::MIDDLEUP,
    }
}

pub fn is_extended_key(vk: u16) -> bool {
    matches!(
        vk,
        0x21..=0x28 // PageUp, PageDown, End, Home, arrow keys
            | 0x2D..=0x2E // Insert, Delete
            | 0x5B..=0x5C // Left/Right Windows
            | 0x6F // Numpad divide
            | 0x90 // NumLock
            | 0xA3 // Right Ctrl
            | 0xA5 // Right Alt
    )
}

pub fn should_use_scan_code(vk: u16) -> bool {
    !is_text_like_virtual_key(vk) || is_extended_key(vk)
}

fn is_text_like_virtual_key(vk: u16) -> bool {
    matches!(
        vk,
        0x30..=0x39 // 0-9
            | 0x41..=0x5A // A-Z
            | 0xBA..=0xC0 // OEM punctuation
            | 0xDB..=0xDF // OEM punctuation
            | 0xE2 // OEM backslash / angle bracket on some layouts
    )
}

#[cfg(windows)]
pub fn send_key_event(key: &str, is_down: bool) -> anyhow::Result<()> {
    if let Some(text) = text_payload(key) {
        if is_down {
            send_text(text)?;
        }
        return Ok(());
    }

    let Some(vk) = key_name_to_vk(key) else {
        anyhow::bail!("unsupported key: {key}");
    };
    if should_use_scan_code(vk) {
        return send_scan_key(vk, is_down);
    }

    let flags = if is_down {
        Default::default()
    } else {
        KEYEVENTF_KEYUP
    };
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let sent = unsafe { SendInput(&[input], std::mem::size_of::<INPUT>() as i32) };
    if sent != 1 {
        anyhow::bail!("SendInput failed");
    }
    Ok(())
}

#[cfg(windows)]
pub fn send_mouse_input_event(event: &MouseInputEvent) -> anyhow::Result<()> {
    if let MouseInputEvent::MoveAbsolute { x, y } = event {
        unsafe { SetCursorPos(*x, *y)? };
        return Ok(());
    }
    if let MouseInputEvent::MoveToLeftEdge { y_permille } = event {
        let screen = virtual_screen_rect();
        unsafe { SetCursorPos(screen.left + 2, screen.y_from_permille(*y_permille))? };
        return Ok(());
    }
    let Some((dx, dy, mouse_data, flags)) = mouse_input_payload(event) else {
        anyhow::bail!("unsupported mouse input event");
    };
    let input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx,
                dy,
                mouseData: mouse_data as u32,
                dwFlags: mouse_flags(flags),
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let sent = unsafe { SendInput(&[input], std::mem::size_of::<INPUT>() as i32) };
    if sent != 1 {
        anyhow::bail!("SendInput mouse event failed");
    }
    Ok(())
}

#[cfg(windows)]
fn mouse_flags(
    flags: MouseFlagSet,
) -> windows::Win32::UI::Input::KeyboardAndMouse::MOUSE_EVENT_FLAGS {
    match flags {
        MouseFlagSet::MOVE => MOUSEEVENTF_MOVE,
        MouseFlagSet::LEFTDOWN => MOUSEEVENTF_LEFTDOWN,
        MouseFlagSet::LEFTUP => MOUSEEVENTF_LEFTUP,
        MouseFlagSet::RIGHTDOWN => MOUSEEVENTF_RIGHTDOWN,
        MouseFlagSet::RIGHTUP => MOUSEEVENTF_RIGHTUP,
        MouseFlagSet::MIDDLEDOWN => MOUSEEVENTF_MIDDLEDOWN,
        MouseFlagSet::MIDDLEUP => MOUSEEVENTF_MIDDLEUP,
        MouseFlagSet::WHEEL => MOUSEEVENTF_WHEEL,
        MouseFlagSet::HWHEEL => MOUSEEVENTF_HWHEEL,
        _ => MOUSEEVENTF_MOVE,
    }
}

#[cfg(windows)]
pub fn release_local_modifiers() -> anyhow::Result<()> {
    for vk in [
        0x10, 0xA0, 0xA1, 0x11, 0xA2, 0xA3, 0x12, 0xA4, 0xA5, 0x5B, 0x5C,
    ] {
        send_key_event(&format!("<{vk}>"), false)?;
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn release_local_modifiers() -> anyhow::Result<()> {
    Ok(())
}

#[cfg(windows)]
fn send_scan_key(vk: u16, is_down: bool) -> anyhow::Result<()> {
    let scan_code = unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC) };
    if scan_code == 0 {
        anyhow::bail!("unsupported scan code for virtual key: {vk}");
    }
    let mut flags = KEYEVENTF_SCANCODE;
    if is_extended_key(vk) {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    if !is_down {
        flags |= KEYEVENTF_KEYUP;
    }
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: scan_code as u16,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let sent = unsafe { SendInput(&[input], std::mem::size_of::<INPUT>() as i32) };
    if sent != 1 {
        anyhow::bail!("SendInput scan key failed");
    }
    Ok(())
}

#[cfg(windows)]
fn send_text(text: &str) -> anyhow::Result<()> {
    for code_unit in text.encode_utf16() {
        send_unicode_unit(code_unit, true)?;
        send_unicode_unit(code_unit, false)?;
    }
    Ok(())
}

#[cfg(windows)]
fn send_unicode_unit(code_unit: u16, is_down: bool) -> anyhow::Result<()> {
    let flags = KEYEVENTF_UNICODE
        | if is_down {
            Default::default()
        } else {
            KEYEVENTF_KEYUP
        };
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: code_unit,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let sent = unsafe { SendInput(&[input], std::mem::size_of::<INPUT>() as i32) };
    if sent != 1 {
        anyhow::bail!("SendInput text failed");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{MouseButton, MouseButtonAction, MouseInputEvent};

    #[test]
    fn parses_bracketed_virtual_key() {
        assert_eq!(key_name_to_vk("<65>"), Some(65));
    }

    #[test]
    fn rejects_invalid_virtual_key() {
        assert_eq!(key_name_to_vk("<abc>"), None);
    }

    #[test]
    fn maps_named_key() {
        assert_eq!(key_name_to_vk("enter"), Some(0x0D));
    }

    #[test]
    fn text_payloads_are_not_virtual_keys() {
        assert_eq!(text_payload("text:a"), Some("a"));
        assert_eq!(text_payload("enter"), None);
    }

    #[test]
    fn navigation_keys_are_extended_keys() {
        assert!(is_extended_key(0x25));
        assert!(is_extended_key(0x26));
        assert!(is_extended_key(0x27));
        assert!(is_extended_key(0x28));
        assert!(is_extended_key(0x2E));
        assert!(!is_extended_key(0x41));
        assert!(!is_extended_key(0x0D));
    }

    #[test]
    fn functional_keys_use_scan_codes() {
        for vk in [
            0x08, // Backspace
            0x09, // Tab
            0x0D, // Enter
            0x14, // CapsLock
            0x1B, // Escape
            0x20, // Space
            0x2C, // PrintScreen
            0x2D, // Insert
            0x2E, // Delete
        ] {
            assert!(should_use_scan_code(vk), "vk {vk} should use scan code");
        }
        for vk in 0x70..=0x7B {
            assert!(
                should_use_scan_code(vk),
                "F-key vk {vk} should use scan code"
            );
        }
    }

    #[test]
    fn modifiers_use_scan_codes_without_being_extended() {
        assert!(should_use_scan_code(0x10));
        assert!(should_use_scan_code(0xA0));
        assert!(should_use_scan_code(0xA1));
        assert!(should_use_scan_code(0x11));
        assert!(should_use_scan_code(0x12));
        assert!(!is_extended_key(0x10));
        assert!(!is_extended_key(0xA0));
        assert!(!is_extended_key(0xA1));
    }

    #[test]
    fn regular_letters_stay_on_virtual_key_path() {
        assert!(!should_use_scan_code(0x41));
        assert!(!should_use_scan_code(0x31));
    }

    #[test]
    fn mouse_button_events_map_to_down_and_up_flags() {
        let down = MouseInputEvent::Button {
            button: MouseButton::Left,
            action: MouseButtonAction::Down,
        };
        let up = MouseInputEvent::Button {
            button: MouseButton::Left,
            action: MouseButtonAction::Up,
        };

        assert_eq!(
            mouse_input_payload(&down),
            Some((0, 0, 0, MouseFlagSet::LEFTDOWN))
        );
        assert_eq!(
            mouse_input_payload(&up),
            Some((0, 0, 0, MouseFlagSet::LEFTUP))
        );
    }

    #[test]
    fn mouse_motion_and_wheel_events_keep_signed_values() {
        assert_eq!(
            mouse_input_payload(&MouseInputEvent::MoveRelative { dx: -5, dy: 9 }),
            Some((-5, 9, 0, MouseFlagSet::MOVE))
        );
        assert_eq!(
            mouse_input_payload(&MouseInputEvent::Wheel { delta: -120 }),
            Some((0, 0, -120, MouseFlagSet::WHEEL))
        );
    }

    #[test]
    fn mouse_absolute_move_uses_cursor_position_path() {
        assert_eq!(
            mouse_input_payload(&MouseInputEvent::MoveAbsolute { x: 10, y: 20 }),
            None
        );
        assert_eq!(
            mouse_input_payload(&MouseInputEvent::MoveToLeftEdge { y_permille: 500 }),
            None
        );
    }
}
