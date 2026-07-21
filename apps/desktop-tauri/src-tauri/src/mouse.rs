#![allow(dead_code)]

#[cfg(windows)]
use windows::Win32::Foundation::POINT;
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetSystemMetrics, SetCursorPos, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
    SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MousePosition {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScreenRect {
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
}

impl ScreenRect {
    pub fn right(self) -> i32 {
        self.left + self.width - 1
    }

    pub fn vertical_ratio(self, y: i32) -> f32 {
        if self.height <= 1 {
            return 0.0;
        }
        ((y - self.top) as f32 / (self.height - 1) as f32).clamp(0.0, 1.0)
    }

    pub fn y_from_ratio(self, ratio: f32) -> i32 {
        self.top + (ratio.clamp(0.0, 1.0) * (self.height - 1) as f32).round() as i32
    }

    pub fn y_permille(self, y: i32) -> u16 {
        (self.vertical_ratio(y) * 1000.0).round().clamp(0.0, 1000.0) as u16
    }

    pub fn y_from_permille(self, permille: u16) -> i32 {
        self.y_from_ratio((permille.min(1000) as f32) / 1000.0)
    }
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

#[cfg(windows)]
pub fn set_cursor_position(pos: MousePosition) -> anyhow::Result<()> {
    unsafe { SetCursorPos(pos.x, pos.y)? };
    Ok(())
}

#[cfg(windows)]
pub fn virtual_screen_rect() -> ScreenRect {
    ScreenRect {
        left: unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) },
        top: unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) },
        width: unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) },
        height: unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) },
    }
}

pub fn at_right_edge(pos: MousePosition, screen: ScreenRect, threshold: i32) -> bool {
    pos.x >= screen.right() - threshold.max(0)
}

pub fn at_left_edge(pos: MousePosition, screen: ScreenRect, threshold: i32) -> bool {
    pos.x <= screen.left + threshold.max(0)
}

pub fn screen_center(screen: ScreenRect) -> MousePosition {
    MousePosition {
        x: screen.left + screen.width / 2,
        y: screen.top + screen.height / 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mouse_position_compares_by_coordinates() {
        assert_eq!(MousePosition { x: 1, y: 2 }, MousePosition { x: 1, y: 2 });
    }

    #[test]
    fn screen_rect_maps_vertical_ratio() {
        let screen = ScreenRect {
            left: 0,
            top: 10,
            width: 1920,
            height: 100,
        };

        assert_eq!(screen.vertical_ratio(10), 0.0);
        assert_eq!(screen.y_from_ratio(1.0), 109);
        assert_eq!(screen.y_permille(60), 505);
        assert_eq!(screen.y_from_permille(1000), 109);
    }

    #[test]
    fn edge_detection_uses_threshold() {
        let screen = ScreenRect {
            left: 0,
            top: 0,
            width: 100,
            height: 100,
        };

        assert!(at_right_edge(MousePosition { x: 98, y: 50 }, screen, 2));
        assert!(at_left_edge(MousePosition { x: 2, y: 50 }, screen, 2));
        assert!(!at_right_edge(MousePosition { x: 96, y: 50 }, screen, 2));
    }

    #[test]
    fn screen_center_uses_virtual_screen_origin() {
        let screen = ScreenRect {
            left: -100,
            top: 20,
            width: 200,
            height: 100,
        };

        assert_eq!(screen_center(screen), MousePosition { x: 0, y: 70 });
    }
}
