#[cfg(windows)]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    MapVirtualKeyW, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY,
    KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE, KEYEVENTF_UNICODE, MAPVK_VK_TO_VSC, VIRTUAL_KEY,
};

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
    if is_extended_key(vk) {
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
    let sent = unsafe { SendInput(&mut [input], std::mem::size_of::<INPUT>() as i32) };
    if sent != 1 {
        anyhow::bail!("SendInput failed");
    }
    Ok(())
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
    let mut flags = KEYEVENTF_SCANCODE | KEYEVENTF_EXTENDEDKEY;
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
    let sent = unsafe { SendInput(&mut [input], std::mem::size_of::<INPUT>() as i32) };
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
    let sent = unsafe { SendInput(&mut [input], std::mem::size_of::<INPUT>() as i32) };
    if sent != 1 {
        anyhow::bail!("SendInput text failed");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
