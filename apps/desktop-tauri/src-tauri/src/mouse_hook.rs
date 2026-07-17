use anyhow::Result;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Mutex, OnceLock};
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, SetWindowsHookExW, TranslateMessage,
    LLMHF_INJECTED, MSG, MSLLHOOKSTRUCT, WH_MOUSE_LL, WM_LBUTTONDOWN, WM_LBUTTONUP,
    WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEHWHEEL, WM_MOUSEWHEEL, WM_RBUTTONDOWN, WM_RBUTTONUP,
};

use crate::protocol::{MouseButton, MouseButtonAction, MouseInputEvent};

static MOUSE_SENDER: OnceLock<Mutex<Option<Sender<MouseInputEvent>>>> = OnceLock::new();
static SUPPRESS_MOUSE_INPUT: AtomicBool = AtomicBool::new(false);

pub fn set_mouse_input_suppression(enabled: bool) {
    SUPPRESS_MOUSE_INPUT.store(enabled, Ordering::SeqCst);
}

pub fn run_mouse_hook(sender: Sender<MouseInputEvent>) -> Result<()> {
    let slot = MOUSE_SENDER.get_or_init(|| Mutex::new(None));
    *slot.lock().expect("mouse sender lock poisoned") = Some(sender);
    unsafe {
        let module = GetModuleHandleW(None)?;
        let hook = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), Some(module.into()), 0)?;
        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).into() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        let _ = hook;
    }
    Ok(())
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let info = *(lparam.0 as *const MSLLHOOKSTRUCT);
        let injected = info.flags & LLMHF_INJECTED != 0;
        if !injected {
            if let Some(event) = decode_mouse_message(wparam.0 as u32, info.mouseData) {
                if let Some(slot) = MOUSE_SENDER.get() {
                    if let Some(sender) = slot.lock().expect("mouse sender lock poisoned").as_ref() {
                        let _ = sender.send(event);
                    }
                }
                if SUPPRESS_MOUSE_INPUT.load(Ordering::SeqCst) {
                    return LRESULT(1);
                }
            }
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

fn decode_mouse_message(message: u32, mouse_data: u32) -> Option<MouseInputEvent> {
    let button_event = |button, action| MouseInputEvent::Button { button, action };
    match message {
        WM_LBUTTONDOWN => Some(button_event(MouseButton::Left, MouseButtonAction::Down)),
        WM_LBUTTONUP => Some(button_event(MouseButton::Left, MouseButtonAction::Up)),
        WM_RBUTTONDOWN => Some(button_event(MouseButton::Right, MouseButtonAction::Down)),
        WM_RBUTTONUP => Some(button_event(MouseButton::Right, MouseButtonAction::Up)),
        WM_MBUTTONDOWN => Some(button_event(MouseButton::Middle, MouseButtonAction::Down)),
        WM_MBUTTONUP => Some(button_event(MouseButton::Middle, MouseButtonAction::Up)),
        WM_MOUSEWHEEL => Some(MouseInputEvent::Wheel {
            delta: wheel_delta(mouse_data),
        }),
        WM_MOUSEHWHEEL => Some(MouseInputEvent::HWheel {
            delta: wheel_delta(mouse_data),
        }),
        _ => None,
    }
}

fn wheel_delta(mouse_data: u32) -> i32 {
    ((mouse_data >> 16) as u16 as i16) as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{MouseButton, MouseButtonAction, MouseInputEvent};
    use windows::Win32::UI::WindowsAndMessaging::{
        WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEHWHEEL,
        WM_MOUSEWHEEL, WM_RBUTTONDOWN, WM_RBUTTONUP,
    };

    #[test]
    fn button_messages_decode_to_remote_mouse_events() {
        let cases = [
            (
                WM_LBUTTONDOWN,
                MouseInputEvent::Button {
                    button: MouseButton::Left,
                    action: MouseButtonAction::Down,
                },
            ),
            (
                WM_LBUTTONUP,
                MouseInputEvent::Button {
                    button: MouseButton::Left,
                    action: MouseButtonAction::Up,
                },
            ),
            (
                WM_RBUTTONDOWN,
                MouseInputEvent::Button {
                    button: MouseButton::Right,
                    action: MouseButtonAction::Down,
                },
            ),
            (
                WM_RBUTTONUP,
                MouseInputEvent::Button {
                    button: MouseButton::Right,
                    action: MouseButtonAction::Up,
                },
            ),
            (
                WM_MBUTTONDOWN,
                MouseInputEvent::Button {
                    button: MouseButton::Middle,
                    action: MouseButtonAction::Down,
                },
            ),
            (
                WM_MBUTTONUP,
                MouseInputEvent::Button {
                    button: MouseButton::Middle,
                    action: MouseButtonAction::Up,
                },
            ),
        ];

        for (message, expected) in cases {
            assert_eq!(decode_mouse_message(message, 0), Some(expected));
        }
    }

    #[test]
    fn wheel_messages_keep_signed_delta() {
        let positive = (120_u32) << 16;
        let negative = ((-120_i16) as u16 as u32) << 16;

        assert_eq!(
            decode_mouse_message(WM_MOUSEWHEEL, positive),
            Some(MouseInputEvent::Wheel { delta: 120 })
        );
        assert_eq!(
            decode_mouse_message(WM_MOUSEWHEEL, negative),
            Some(MouseInputEvent::Wheel { delta: -120 })
        );
        assert_eq!(
            decode_mouse_message(WM_MOUSEHWHEEL, negative),
            Some(MouseInputEvent::HWheel { delta: -120 })
        );
    }
}
