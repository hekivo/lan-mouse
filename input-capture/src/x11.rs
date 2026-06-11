use std::collections::HashSet;
use std::pin::Pin;
use std::task::{Context, Poll};

use async_trait::async_trait;
use futures_core::Stream;
use tokio::sync::mpsc as tokio_mpsc;

use super::{error::X11InputCaptureCreationError, Capture, CaptureError, CaptureEvent, Position};

// ── Request enum (async → thread) ────────────────────────────────────────────

enum Request {
    Create(Position),
    Destroy(Position),
    Release,
    Terminate,
}

// ── Public struct ─────────────────────────────────────────────────────────────

pub struct X11InputCapture {
    event_rx: tokio_mpsc::Receiver<(Position, CaptureEvent)>,
    request_tx: std::sync::mpsc::SyncSender<Request>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl X11InputCapture {
    pub fn new() -> Result<Self, X11InputCaptureCreationError> {
        Err(X11InputCaptureCreationError::NotImplemented) // replaced in Task 3
    }
}

// ── Async trait impl (stubs, replaced in Task 3) ──────────────────────────────

#[async_trait]
impl Capture for X11InputCapture {
    async fn create(&mut self, _pos: Position) -> Result<(), CaptureError> { Ok(()) }
    async fn destroy(&mut self, _pos: Position) -> Result<(), CaptureError> { Ok(()) }
    async fn release(&mut self) -> Result<(), CaptureError> { Ok(()) }
    async fn terminate(&mut self) -> Result<(), CaptureError> { Ok(()) }
}

impl Stream for X11InputCapture {
    type Item = Result<(Position, CaptureEvent), CaptureError>;
    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Pending
    }
}

// ── Pure logic functions ───────────────────────────────────────────────────────

/// Returns which boundary was crossed when the pointer moved from `prev` to `curr`,
/// given a virtual desktop of size `w` × `h`. Returns `None` if no boundary crossed.
pub(crate) fn crossed_boundary(
    prev: (i32, i32),
    curr: (i32, i32),
    w: i32,
    h: i32,
) -> Option<Position> {
    if prev.0 > 0 && curr.0 <= 0 {
        Some(Position::Left)
    } else if prev.0 < w - 1 && curr.0 >= w {
        Some(Position::Right)
    } else if prev.1 > 0 && curr.1 <= 0 {
        Some(Position::Top)
    } else if prev.1 < h - 1 && curr.1 >= h {
        Some(Position::Bottom)
    } else {
        None
    }
}

/// Clamps `pos` to the valid screen coordinate range `[0, w-1] × [0, h-1]`.
pub(crate) fn clamp_to_screen(pos: (i32, i32), w: i32, h: i32) -> (i32, i32) {
    (pos.0.clamp(0, w - 1), pos.1.clamp(0, h - 1))
}

/// Maps X11 button numbers (1=left, 2=middle, 3=right) to Linux evdev button codes.
pub(crate) fn x11_button_to_evdev(button: u32) -> Option<u32> {
    use input_event::{BTN_LEFT, BTN_MIDDLE, BTN_RIGHT};
    match button {
        1 => Some(BTN_LEFT),
        2 => Some(BTN_MIDDLE),
        3 => Some(BTN_RIGHT),
        _ => None,
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crosses_left_boundary() {
        assert_eq!(crossed_boundary((5, 100), (-1, 100), 1920, 1080), Some(Position::Left));
    }

    #[test]
    fn crosses_right_boundary() {
        assert_eq!(crossed_boundary((1915, 100), (1920, 100), 1920, 1080), Some(Position::Right));
    }

    #[test]
    fn crosses_top_boundary() {
        assert_eq!(crossed_boundary((100, 5), (100, -1), 1920, 1080), Some(Position::Top));
    }

    #[test]
    fn crosses_bottom_boundary() {
        assert_eq!(crossed_boundary((100, 1075), (100, 1080), 1920, 1080), Some(Position::Bottom));
    }

    #[test]
    fn no_crossing_interior_movement() {
        assert_eq!(crossed_boundary((100, 100), (200, 200), 1920, 1080), None);
    }

    #[test]
    fn no_crossing_already_at_left_edge() {
        assert_eq!(crossed_boundary((0, 100), (0, 100), 1920, 1080), None);
    }

    #[test]
    fn no_crossing_at_right_pixel() {
        assert_eq!(crossed_boundary((1919, 100), (1919, 100), 1920, 1080), None);
    }

    #[test]
    fn clamp_within_bounds_is_identity() {
        assert_eq!(clamp_to_screen((500, 300), 1920, 1080), (500, 300));
    }

    #[test]
    fn clamp_negative_coords() {
        assert_eq!(clamp_to_screen((-10, -5), 1920, 1080), (0, 0));
    }

    #[test]
    fn clamp_over_right_bottom_edge() {
        assert_eq!(clamp_to_screen((2000, 1200), 1920, 1080), (1919, 1079));
    }

    #[test]
    fn left_button_maps_to_btn_left() {
        use input_event::BTN_LEFT;
        assert_eq!(x11_button_to_evdev(1), Some(BTN_LEFT));
    }

    #[test]
    fn right_button_maps_to_btn_right() {
        use input_event::BTN_RIGHT;
        assert_eq!(x11_button_to_evdev(3), Some(BTN_RIGHT));
    }

    #[test]
    fn unknown_button_returns_none() {
        assert_eq!(x11_button_to_evdev(8), None);
    }
}
