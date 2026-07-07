#[cfg(windows)]
use windows::Win32::Foundation::POINT;
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MousePosition {
    pub x: i32,
    pub y: i32,
}

#[cfg(windows)]
pub fn cursor_position() -> anyhow::Result<MousePosition> {
    let mut point = POINT::default();
    unsafe { GetCursorPos(&mut point)? };
    Ok(MousePosition {
        x: point.x,
        y: point.y,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mouse_position_compares_by_coordinates() {
        assert_eq!(
            MousePosition { x: 1, y: 2 },
            MousePosition { x: 1, y: 2 }
        );
    }
}
