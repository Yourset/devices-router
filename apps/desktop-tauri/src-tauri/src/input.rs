#[cfg(windows)]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    MapVirtualKeyW, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
    KEYEVENTF_SCANCODE, MAPVK_VK_TO_VSC, VIRTUAL_KEY,
};

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

#[cfg(windows)]
pub fn send_key_event(key: &str, is_down: bool) -> anyhow::Result<()> {
    let Some(vk) = key_name_to_vk(key) else {
        anyhow::bail!("unsupported key: {key}");
    };
    let scan_code = unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC) };
    let flags = KEYEVENTF_SCANCODE
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
                wScan: scan_code as u16,
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
}
