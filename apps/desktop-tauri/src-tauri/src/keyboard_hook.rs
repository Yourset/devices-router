use anyhow::Result;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Mutex, OnceLock};
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyboardState, ToUnicode};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, SetWindowsHookExW, TranslateMessage,
    KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawKeyEvent {
    pub vk_code: u32,
    pub is_down: bool,
    pub text: Option<String>,
}

static KEY_SENDER: OnceLock<Mutex<Option<Sender<RawKeyEvent>>>> = OnceLock::new();
static SUPPRESS_KEYS: AtomicBool = AtomicBool::new(false);
static PANIC_REQUESTED: AtomicBool = AtomicBool::new(false);
static PANIC_CHORD: OnceLock<Mutex<PanicChordState>> = OnceLock::new();

#[derive(Default)]
struct PanicChordState {
    ctrl_down: bool,
    alt_down: bool,
}

impl PanicChordState {
    fn observe(&mut self, vk_code: u32, is_down: bool) -> bool {
        match vk_code {
            0x11 | 0xA2 | 0xA3 => self.ctrl_down = is_down,
            0x12 | 0xA4 | 0xA5 => self.alt_down = is_down,
            0x1B if is_down => return self.ctrl_down && self.alt_down,
            _ => {}
        }
        false
    }
}

pub fn set_key_suppression(enabled: bool) {
    SUPPRESS_KEYS.store(enabled, Ordering::SeqCst);
}

pub fn take_panic_request() -> bool {
    PANIC_REQUESTED.swap(false, Ordering::SeqCst)
}

pub fn run_keyboard_hook(sender: Sender<RawKeyEvent>) -> Result<()> {
    let slot = KEY_SENDER.get_or_init(|| Mutex::new(None));
    *slot.lock().expect("keyboard sender lock poisoned") = Some(sender);
    unsafe {
        let module = GetModuleHandleW(None)?;
        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), Some(module.into()), 0)?;
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
            let panic_triggered = PANIC_CHORD
                .get_or_init(|| Mutex::new(PanicChordState::default()))
                .lock()
                .expect("panic chord lock poisoned")
                .observe(info.vkCode, is_down);
            if panic_triggered {
                SUPPRESS_KEYS.store(false, Ordering::SeqCst);
                crate::mouse_hook::set_mouse_input_suppression(false);
                PANIC_REQUESTED.store(true, Ordering::SeqCst);
                return LRESULT(1);
            }
            if let Some(slot) = KEY_SENDER.get() {
                if let Some(sender) = slot.lock().expect("keyboard sender lock poisoned").as_ref() {
                    let _ = sender.send(RawKeyEvent {
                        vk_code: info.vkCode,
                        is_down,
                        text: if is_down { key_text(&info) } else { None },
                    });
                }
            }
            if SUPPRESS_KEYS.load(Ordering::SeqCst) {
                return LRESULT(1);
            }
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

fn key_text(info: &KBDLLHOOKSTRUCT) -> Option<String> {
    let mut state = [0_u8; 256];
    if unsafe { GetKeyboardState(&mut state) }.is_err() {
        return None;
    }
    normalize_text_keyboard_state(&mut state);
    if ctrl_or_alt_down(&state) {
        return None;
    }

    let mut buffer = [0_u16; 8];
    let count = unsafe { ToUnicode(info.vkCode, info.scanCode, Some(&state), &mut buffer, 0) };
    if count <= 0 {
        return None;
    }
    String::from_utf16(&buffer[..count as usize])
        .ok()
        .filter(|text| !text.chars().any(char::is_control))
}

fn ctrl_or_alt_down(state: &[u8; 256]) -> bool {
    key_is_down(state, 0x11) || key_is_down(state, 0x12)
}

fn key_is_down(state: &[u8; 256], vk: usize) -> bool {
    state[vk] & 0x80 != 0
}

fn normalize_text_keyboard_state(state: &mut [u8; 256]) {
    const VK_NUMLOCK: usize = 0x90;
    state[VK_NUMLOCK] = 0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_key_event_keeps_vk_and_direction() {
        let event = RawKeyEvent {
            vk_code: 65,
            is_down: true,
            text: Some("a".to_string()),
        };

        assert_eq!(event.vk_code, 65);
        assert!(event.is_down);
        assert_eq!(event.text.as_deref(), Some("a"));
    }

    #[test]
    fn key_suppression_can_be_toggled() {
        set_key_suppression(true);
        assert!(SUPPRESS_KEYS.load(Ordering::SeqCst));
        set_key_suppression(false);
        assert!(!SUPPRESS_KEYS.load(Ordering::SeqCst));
    }

    #[test]
    fn text_keyboard_state_ignores_num_lock_toggle() {
        let mut state = [0_u8; 256];
        state[0x90] = 0x81;

        normalize_text_keyboard_state(&mut state);

        assert_eq!(state[0x90], 0);
    }

    #[test]
    fn panic_chord_activates_only_for_ctrl_alt_escape() {
        let mut chord = PanicChordState::default();

        assert!(!chord.observe(0x11, true));
        assert!(!chord.observe(0x12, true));
        assert!(chord.observe(0x1B, true));
    }

    #[test]
    fn panic_chord_rejects_plain_escape_and_resets_modifiers() {
        let mut chord = PanicChordState::default();

        assert!(!chord.observe(0x1B, true));
        assert!(!chord.observe(0x11, true));
        assert!(!chord.observe(0x12, true));
        assert!(!chord.observe(0x12, false));
        assert!(!chord.observe(0x1B, true));
    }
}
