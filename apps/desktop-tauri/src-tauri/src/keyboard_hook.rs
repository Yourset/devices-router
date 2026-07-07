use anyhow::Result;
use std::sync::mpsc::Sender;
use std::sync::{Mutex, OnceLock};
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, SetWindowsHookExW, TranslateMessage,
    KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawKeyEvent {
    pub vk_code: u32,
    pub is_down: bool,
}

static KEY_SENDER: OnceLock<Mutex<Option<Sender<RawKeyEvent>>>> = OnceLock::new();

pub fn run_keyboard_hook(sender: Sender<RawKeyEvent>) -> Result<()> {
    let slot = KEY_SENDER.get_or_init(|| Mutex::new(None));
    *slot.lock().expect("keyboard sender lock poisoned") = Some(sender);
    unsafe {
        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), None, 0)?;
        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).into() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        let _ = hook;
    }
    Ok(())
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let message = wparam.0 as u32;
        let is_down = message == WM_KEYDOWN || message == WM_SYSKEYDOWN;
        let is_up = message == WM_KEYUP || message == WM_SYSKEYUP;
        if is_down || is_up {
            let info = *(lparam.0 as *const KBDLLHOOKSTRUCT);
            if let Some(slot) = KEY_SENDER.get() {
                if let Some(sender) = slot.lock().expect("keyboard sender lock poisoned").as_ref() {
                    let _ = sender.send(RawKeyEvent {
                        vk_code: info.vkCode,
                        is_down,
                    });
                }
            }
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_key_event_keeps_vk_and_direction() {
        let event = RawKeyEvent {
            vk_code: 65,
            is_down: true,
        };

        assert_eq!(event.vk_code, 65);
        assert!(event.is_down);
    }
}
