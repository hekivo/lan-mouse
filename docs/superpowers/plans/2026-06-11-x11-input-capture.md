# X11 Input Capture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the `X11InputCapture` stub with a working implementation so lan-mouse can capture mouse and keyboard on X11 sessions.

**Architecture:** Dedicated OS thread runs an X11 event loop. Phase 1 polls `XQueryPointer` at 1 ms intervals to detect screen-edge crossings. On crossing, `XGrabPointer + XGrabKeyboard` capture all input (phase 2) and forward events via a `tokio::sync::mpsc` channel to the async runtime. Requests from the async side (create/destroy/release/terminate) arrive via `std::sync::mpsc::sync_channel`.

**Tech Stack:** Rust, `x11` crate (xlib + xtest features, already in Cargo.toml), `tokio::sync::mpsc`, `std::sync::mpsc`

---

## Files

| File | Action |
|---|---|
| `input-capture/src/error.rs` | Add `OpenDisplayFailed` variant to `X11InputCaptureCreationError` |
| `input-capture/src/x11.rs` | Full implementation replacing stub |

---

### Task 1: Add `OpenDisplayFailed` error variant

**Files:**
- Modify: `input-capture/src/error.rs`

- [ ] **Step 1: Add the variant**

Open `input-capture/src/error.rs`. Find this block (around line 137):

```rust
#[cfg(all(unix, feature = "x11", not(target_os = "macos")))]
#[derive(Debug, Error)]
pub enum X11InputCaptureCreationError {
    #[error("X11 input capture is not yet implemented :(")]
    NotImplemented,
}
```

Replace with:

```rust
#[cfg(all(unix, feature = "x11", not(target_os = "macos")))]
#[derive(Debug, Error)]
pub enum X11InputCaptureCreationError {
    #[error("X11 input capture is not yet implemented :(")]
    NotImplemented,
    #[error("XOpenDisplay failed — is DISPLAY set?")]
    OpenDisplayFailed,
}
```

- [ ] **Step 2: Verify it compiles**

```bash
cd ~/work/lan-mouse
cargo check -p input-capture --features x11 2>&1
```

Expected: `Finished` or only unrelated warnings. No errors.

- [ ] **Step 3: Commit**

```bash
git -C ~/work/lan-mouse add input-capture/src/error.rs
git -C ~/work/lan-mouse commit -m "feat(x11-capture): add OpenDisplayFailed error variant"
```

---

### Task 2: Pure logic functions + unit tests

**Files:**
- Modify: `input-capture/src/x11.rs`

These are the three pure functions that drive edge detection. They have no X11 dependencies and can be tested without a display.

- [ ] **Step 1: Replace the stub with the skeleton + pure functions + tests**

Replace the entire contents of `input-capture/src/x11.rs` with:

```rust
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

    // crossed_boundary --------------------------------------------------------

    #[test]
    fn crosses_left_boundary() {
        assert_eq!(
            crossed_boundary((5, 100), (-1, 100), 1920, 1080),
            Some(Position::Left)
        );
    }

    #[test]
    fn crosses_right_boundary() {
        assert_eq!(
            crossed_boundary((1915, 100), (1920, 100), 1920, 1080),
            Some(Position::Right)
        );
    }

    #[test]
    fn crosses_top_boundary() {
        assert_eq!(
            crossed_boundary((100, 5), (100, -1), 1920, 1080),
            Some(Position::Top)
        );
    }

    #[test]
    fn crosses_bottom_boundary() {
        assert_eq!(
            crossed_boundary((100, 1075), (100, 1080), 1920, 1080),
            Some(Position::Bottom)
        );
    }

    #[test]
    fn no_crossing_interior_movement() {
        assert_eq!(crossed_boundary((100, 100), (200, 200), 1920, 1080), None);
    }

    #[test]
    fn no_crossing_already_at_left_edge() {
        // already at x=0, cursor stays there — not a new crossing
        assert_eq!(crossed_boundary((0, 100), (0, 100), 1920, 1080), None);
    }

    #[test]
    fn no_crossing_at_right_pixel() {
        // x=1919 is the last valid pixel; x=1919→1919 is not a crossing
        assert_eq!(crossed_boundary((1919, 100), (1919, 100), 1920, 1080), None);
    }

    // clamp_to_screen ---------------------------------------------------------

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

    // x11_button_to_evdev -----------------------------------------------------

    #[test]
    fn left_button_maps_to_btn_left() {
        use input_event::BTN_LEFT;
        assert_eq!(x11_button_to_evdev(1), Some(BTN_LEFT));
    }

    #[test]
    fn middle_button_maps_to_btn_middle() {
        use input_event::BTN_MIDDLE;
        assert_eq!(x11_button_to_evdev(2), Some(BTN_MIDDLE));
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
```

- [ ] **Step 2: Run the tests**

```bash
cd ~/work/lan-mouse
cargo test -p input-capture --features x11 -- x11 2>&1
```

Expected output — 14 tests, all passing:
```
running 14 tests
test x11::tests::crosses_left_boundary ... ok
test x11::tests::crosses_right_boundary ... ok
test x11::tests::crosses_top_boundary ... ok
test x11::tests::crosses_bottom_boundary ... ok
test x11::tests::no_crossing_interior_movement ... ok
test x11::tests::no_crossing_already_at_left_edge ... ok
test x11::tests::no_crossing_at_right_pixel ... ok
test x11::tests::clamp_within_bounds_is_identity ... ok
test x11::tests::clamp_negative_coords ... ok
test x11::tests::clamp_over_right_bottom_edge ... ok
test x11::tests::left_button_maps_to_btn_left ... ok
test x11::tests::middle_button_maps_to_btn_middle ... ok
test x11::tests::right_button_maps_to_btn_right ... ok
test x11::tests::unknown_button_returns_none ... ok

test result: ok. 14 passed
```

- [ ] **Step 3: Commit**

```bash
git -C ~/work/lan-mouse add input-capture/src/x11.rs
git -C ~/work/lan-mouse commit -m "feat(x11-capture): pure logic functions + unit tests"
```

---

### Task 3: Full X11 implementation

**Files:**
- Modify: `input-capture/src/x11.rs` (replace skeleton with full implementation)

This replaces the `new()` stub and async stubs with working code. The pure functions from Task 2 stay unchanged at the bottom.

- [ ] **Step 1: Replace `x11.rs` with the full implementation**

Replace the entire contents of `input-capture/src/x11.rs` with:

```rust
use std::collections::HashSet;
use std::pin::Pin;
use std::sync::mpsc;
use std::task::{Context, Poll};
use std::thread;
use std::time::Duration;

use async_trait::async_trait;
use futures_core::Stream;
use tokio::sync::mpsc as tokio_mpsc;

use x11::xlib::{
    ButtonMotionMask, ButtonPress, ButtonPressMask, ButtonRelease, ButtonReleaseMask, CurrentTime,
    Display, False, GrabModeAsync, GrabSuccess, KeyPress, KeyRelease, MotionNotify,
    PointerMotionMask, Window, XButtonEvent, XCloseDisplay, XDefaultRootWindow, XDefaultScreen,
    XDisplayHeight, XDisplayWidth, XEvent, XFlush, XGrabKeyboard, XGrabPointer, XKeyEvent,
    XMotionEvent, XNextEvent, XPending, XQueryPointer, XUngrabKeyboard, XUngrabPointer,
    XWarpPointer,
};

use input_event::{Event, KeyboardEvent, PointerEvent};

use super::{error::X11InputCaptureCreationError, Capture, CaptureError, CaptureEvent, Position};

// ── Request enum (async → thread) ────────────────────────────────────────────

enum Request {
    Create(Position),
    Destroy(Position),
    Release,
    Terminate,
}

// ── Internal thread state ─────────────────────────────────────────────────────

struct X11State {
    display: *mut Display,
    root: Window,
    screen_w: i32,
    screen_h: i32,
    clients: HashSet<Position>,
    active_client: Option<Position>,
    entry_point: (i32, i32),
    prev_pos: (i32, i32),
    event_tx: tokio_mpsc::Sender<(Position, CaptureEvent)>,
    request_rx: mpsc::Receiver<Request>,
}

// Safety: display is only accessed from the dedicated X11 thread.
unsafe impl Send for X11State {}

// ── Public struct ─────────────────────────────────────────────────────────────

pub struct X11InputCapture {
    event_rx: tokio_mpsc::Receiver<(Position, CaptureEvent)>,
    request_tx: mpsc::SyncSender<Request>,
    thread: Option<thread::JoinHandle<()>>,
}

impl X11InputCapture {
    pub fn new() -> Result<Self, X11InputCaptureCreationError> {
        let display = unsafe { x11::xlib::XOpenDisplay(std::ptr::null()) };
        if display.is_null() {
            return Err(X11InputCaptureCreationError::OpenDisplayFailed);
        }

        let screen = unsafe { XDefaultScreen(display) };
        let root = unsafe { XDefaultRootWindow(display) };
        let screen_w = unsafe { XDisplayWidth(display, screen) };
        let screen_h = unsafe { XDisplayHeight(display, screen) };

        let (event_tx, event_rx) = tokio_mpsc::channel(64);
        let (request_tx, request_rx) = mpsc::sync_channel(16);
        let (ready_tx, ready_rx) = mpsc::channel::<()>();

        let state = X11State {
            display,
            root,
            screen_w,
            screen_h,
            clients: HashSet::new(),
            active_client: None,
            entry_point: (0, 0),
            prev_pos: (0, 0),
            event_tx,
            request_rx,
        };

        let thread = thread::spawn(move || {
            ready_tx.send(()).expect("ready channel closed");
            run_event_loop(state);
        });

        ready_rx.recv().expect("ready channel closed");

        Ok(Self {
            event_rx,
            request_tx,
            thread: Some(thread),
        })
    }
}

impl Drop for X11InputCapture {
    fn drop(&mut self) {
        let _ = self.request_tx.send(Request::Terminate);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

// ── Async trait impl ──────────────────────────────────────────────────────────

#[async_trait]
impl Capture for X11InputCapture {
    async fn create(&mut self, pos: Position) -> Result<(), CaptureError> {
        let _ = self.request_tx.send(Request::Create(pos));
        Ok(())
    }

    async fn destroy(&mut self, pos: Position) -> Result<(), CaptureError> {
        let _ = self.request_tx.send(Request::Destroy(pos));
        Ok(())
    }

    async fn release(&mut self) -> Result<(), CaptureError> {
        let _ = self.request_tx.send(Request::Release);
        Ok(())
    }

    async fn terminate(&mut self) -> Result<(), CaptureError> {
        let _ = self.request_tx.send(Request::Terminate);
        Ok(())
    }
}

impl Stream for X11InputCapture {
    type Item = Result<(Position, CaptureEvent), CaptureError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.event_rx.poll_recv(cx) {
            Poll::Ready(Some(e)) => Poll::Ready(Some(Ok(e))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

// ── Event loop ────────────────────────────────────────────────────────────────

fn run_event_loop(mut state: X11State) {
    loop {
        if drain_requests(&mut state) {
            return;
        }
        if state.active_client.is_none() {
            // Phase 1: poll position, detect edge crossing
            let curr = query_pointer(&state);
            if let Some(pos) = crossed_boundary(state.prev_pos, curr, state.screen_w, state.screen_h) {
                if state.clients.contains(&pos) {
                    let entry = clamp_to_screen(curr, state.screen_w, state.screen_h);
                    do_grab(&mut state, pos, entry);
                }
            }
            state.prev_pos = curr;
            thread::sleep(Duration::from_millis(1));
        } else {
            // Phase 2: drain X11 event queue (populated by XGrabPointer)
            let pending = unsafe { XPending(state.display) };
            if pending > 0 {
                let mut ev = unsafe { std::mem::zeroed::<XEvent>() };
                unsafe { XNextEvent(state.display, &mut ev) };
                handle_event(&mut state, ev);
            } else {
                thread::sleep(Duration::from_millis(1));
            }
        }
    }
}

/// Drains pending requests. Returns `true` if the thread should terminate.
fn drain_requests(state: &mut X11State) -> bool {
    loop {
        match state.request_rx.try_recv() {
            Ok(Request::Create(pos)) => {
                state.clients.insert(pos);
            }
            Ok(Request::Destroy(pos)) => {
                state.clients.remove(&pos);
                if state.active_client == Some(pos) {
                    do_release(state);
                }
            }
            Ok(Request::Release) => do_release(state),
            Ok(Request::Terminate) => {
                unsafe { XCloseDisplay(state.display) };
                return true;
            }
            Err(_) => return false,
        }
    }
}

fn query_pointer(state: &X11State) -> (i32, i32) {
    let mut root_return: Window = 0;
    let mut child_return: Window = 0;
    let mut root_x: i32 = 0;
    let mut root_y: i32 = 0;
    let mut win_x: i32 = 0;
    let mut win_y: i32 = 0;
    let mut mask: u32 = 0;
    unsafe {
        XQueryPointer(
            state.display,
            state.root,
            &mut root_return,
            &mut child_return,
            &mut root_x,
            &mut root_y,
            &mut win_x,
            &mut win_y,
            &mut mask,
        )
    };
    (root_x, root_y)
}

fn do_grab(state: &mut X11State, pos: Position, entry: (i32, i32)) {
    let grab_mask =
        (PointerMotionMask | ButtonPressMask | ButtonReleaseMask | ButtonMotionMask) as u32;
    let result = unsafe {
        XGrabPointer(
            state.display,
            state.root,
            False,
            grab_mask,
            GrabModeAsync,
            GrabModeAsync,
            0,           // no confinement
            0,           // no cursor change
            CurrentTime,
        )
    };
    if result != GrabSuccess {
        log::warn!("x11: XGrabPointer failed with code {result}");
        return;
    }
    unsafe {
        XGrabKeyboard(
            state.display,
            state.root,
            False,
            GrabModeAsync,
            GrabModeAsync,
            CurrentTime,
        );
        XWarpPointer(state.display, 0, state.root, 0, 0, 0, 0, entry.0, entry.1);
        XFlush(state.display);
    }
    state.entry_point = entry;
    state.active_client = Some(pos);
    let _ = state.event_tx.try_send((pos, CaptureEvent::Begin));
    log::debug!("x11: grabbed pointer for client {pos:?} at {entry:?}");
}

fn do_release(state: &mut X11State) {
    unsafe {
        XUngrabPointer(state.display, CurrentTime);
        XUngrabKeyboard(state.display, CurrentTime);
        XFlush(state.display);
    }
    log::debug!("x11: released pointer (was {:?})", state.active_client);
    state.active_client = None;
}

fn handle_event(state: &mut X11State, ev: XEvent) {
    match unsafe { ev.type_ } {
        MotionNotify => {
            let m: XMotionEvent = unsafe { ev.xmotion };
            handle_motion(state, m);
        }
        ButtonPress => {
            if let Some(pos) = state.active_client {
                let b: XButtonEvent = unsafe { ev.xbutton };
                if let Some(button) = x11_button_to_evdev(b.button) {
                    let _ = state.event_tx.try_send((
                        pos,
                        CaptureEvent::Input(Event::Pointer(PointerEvent::Button {
                            time: 0,
                            button,
                            state: 1,
                        })),
                    ));
                }
            }
        }
        ButtonRelease => {
            if let Some(pos) = state.active_client {
                let b: XButtonEvent = unsafe { ev.xbutton };
                if let Some(button) = x11_button_to_evdev(b.button) {
                    let _ = state.event_tx.try_send((
                        pos,
                        CaptureEvent::Input(Event::Pointer(PointerEvent::Button {
                            time: 0,
                            button,
                            state: 0,
                        })),
                    ));
                }
            }
        }
        KeyPress => {
            if let Some(pos) = state.active_client {
                let k: XKeyEvent = unsafe { ev.xkey };
                let _ = state.event_tx.try_send((
                    pos,
                    CaptureEvent::Input(Event::Keyboard(KeyboardEvent::Key {
                        time: 0,
                        key: k.keycode.saturating_sub(8),
                        state: 1,
                    })),
                ));
            }
        }
        KeyRelease => {
            if let Some(pos) = state.active_client {
                let k: XKeyEvent = unsafe { ev.xkey };
                let _ = state.event_tx.try_send((
                    pos,
                    CaptureEvent::Input(Event::Keyboard(KeyboardEvent::Key {
                        time: 0,
                        key: k.keycode.saturating_sub(8),
                        state: 0,
                    })),
                ));
            }
        }
        _ => {}
    }
}

fn handle_motion(state: &mut X11State, m: XMotionEvent) {
    let curr = (m.x_root, m.y_root);
    let entry = state.entry_point;
    let dx = (curr.0 - entry.0) as f64;
    let dy = (curr.1 - entry.1) as f64;
    // Skip warp-back echo events (dx/dy == 0)
    if dx == 0.0 && dy == 0.0 {
        return;
    }
    unsafe {
        XWarpPointer(state.display, 0, state.root, 0, 0, 0, 0, entry.0, entry.1);
        XFlush(state.display);
    }
    if let Some(pos) = state.active_client {
        let _ = state.event_tx.try_send((
            pos,
            CaptureEvent::Input(Event::Pointer(PointerEvent::Motion {
                time: 0,
                dx,
                dy,
            })),
        ));
    }
}

// ── Pure logic functions ───────────────────────────────────────────────────────

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

pub(crate) fn clamp_to_screen(pos: (i32, i32), w: i32, h: i32) -> (i32, i32) {
    (pos.0.clamp(0, w - 1), pos.1.clamp(0, h - 1))
}

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
```

- [ ] **Step 2: Run unit tests (no display needed)**

```bash
cd ~/work/lan-mouse
cargo test -p input-capture --features x11 -- x11 2>&1
```

Expected: 12 tests, all passing.

- [ ] **Step 3: Build the full workspace**

```bash
cd ~/work/lan-mouse
cargo build --release 2>&1
```

Expected: `Finished release profile`. No errors. Warnings about unused imports/variables are acceptable.

- [ ] **Step 4: Commit**

```bash
git -C ~/work/lan-mouse add input-capture/src/x11.rs
git -C ~/work/lan-mouse commit -m "feat(x11-capture): implement X11 input capture backend"
```

---

### Task 4: Set up Mint machine + integration test

**Machine:** `leonardo.gobatto@192.168.86.68` (Linux Mint, X11 session on `:0`)

- [ ] **Step 1: Install Rust on Mint**

```bash
ssh leonardo.gobatto@192.168.86.68 \
  "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y"
```

Expected: `Rust is installed now.`

- [ ] **Step 2: Install dev dependencies on Mint**

```bash
ssh leonardo.gobatto@192.168.86.68 \
  "sudo apt install -y libadwaita-1-dev libgtk-4-dev libx11-dev libxtst-dev pkg-config"
```

Expected: packages installed without errors.

- [ ] **Step 3: Copy the repo to Mint**

```bash
rsync -av --exclude=target ~/work/lan-mouse/ \
  leonardo.gobatto@192.168.86.68:~/lan-mouse/
```

Expected: files synced.

- [ ] **Step 4: Build on Mint**

```bash
ssh leonardo.gobatto@192.168.86.68 \
  "source ~/.cargo/env && cd ~/lan-mouse && cargo build --release 2>&1 | tail -5"
```

Expected: `Finished release profile`.

- [ ] **Step 5: Run lan-mouse on Mint (X11 session)**

On Mint, open a terminal and run:

```bash
DISPLAY=:0 ~/lan-mouse/target/release/lan-mouse
```

The GUI should open. Add this machine (Zorin, `192.168.86.X`) as a client positioned to the **right**.

Log should show:
```
INFO  input_capture] using capture backend: X11
```

(Not `dummy` — that confirms the X11 backend initialized successfully.)

- [ ] **Step 6: Run lan-mouse on Zorin**

On this machine:

```bash
~/work/lan-mouse/target/release/lan-mouse
```

Confirm Mint (9JRKNG4) is already configured as left client.

- [ ] **Step 7: Test mouse traversal**

1. Move mouse to the **left edge** of this Zorin screen
2. Mouse should appear on the Mint screen ✓
3. Move mouse to the **right edge** of the Mint screen
4. Mouse should return to Zorin ✓

If step 4 works — X11 capture is working.

- [ ] **Step 8: Test keyboard forwarding**

While mouse is on Mint, type a few keys in a text editor on Mint. Characters should appear. ✓

- [ ] **Step 9: Test release keybind**

While on Mint, press `Ctrl+Shift+Super+Alt` simultaneously. Mouse should return to Zorin immediately. ✓

- [ ] **Step 10: Commit test notes**

If the integration test passes, add a note to the spec:

```bash
# Add a one-liner to the Testing Plan section of the spec
echo "" >> ~/work/lan-mouse/docs/superpowers/specs/2026-06-11-x11-input-capture-design.md
echo "**Integration test result:** Passed on Linux Mint $(ssh leonardo.gobatto@192.168.86.68 'cat /etc/linuxmint/info | grep RELEASE' 2>/dev/null || echo 'unknown version') with DISPLAY=:0" >> ~/work/lan-mouse/docs/superpowers/specs/2026-06-11-x11-input-capture-design.md
git -C ~/work/lan-mouse add docs/
git -C ~/work/lan-mouse commit -m "docs: mark X11 capture integration test as passed"
```
