# X11 Input Capture — Design Spec

**Date:** 2026-06-11  
**File:** `input-capture/src/x11.rs`  
**Status:** Ready for implementation

## Problem

`X11InputCapture::new()` always returns `Err(X11InputCaptureCreationError::NotImplemented)`.  
`poll_next` always returns `Poll::Pending`.  
When lan-mouse runs on an X11 session (e.g. Linux Mint), the capture backend silently falls
through to `Dummy`, so the mouse never returns to the host after crossing the screen edge.

## Approach

**MotionNotify on root window + XGrabPointer.**  
A dedicated OS thread runs the X11 event loop. It subscribes to `PointerMotionMask` on the root
window to detect edge crossings, then grabs pointer + keyboard on entry. Events are forwarded to
the async runtime via a `tokio::sync::mpsc` channel. Requests from the async side (create, destroy,
release, terminate) arrive via a `std::sync::mpsc` channel; the thread drains it between X11 events
using `XPending` + `try_recv` polling at ~1 ms to avoid blocking `XNextEvent` forever.

This mirrors the Windows backend architecture exactly.

## Architecture

```
async runtime (tokio)
  InputCapture::poll_next()
    await event_rx  ←──────────────────────────────────┐
                                                        │ send()
OS thread (X11 event loop)                             │
  state:                                               │
    display:       *mut Display                        │
    root:          Window                              │
    screen_w/h:    i32                                 │
    clients:       HashSet<Position>                   │
    active_client: Option<Position>                    │
    entry_point:   (i32, i32)                          │
    prev_pos:      (i32, i32)                          │
    event_tx:      Sender<(Position, CaptureEvent)> ───┘
    request_rx:    std::sync::mpsc::Receiver<Request>
```

## Request Protocol

```rust
enum Request {
    Create(Position),
    Destroy(Position),
    Release,
    Terminate,
}
```

`create()` / `destroy()` / `release()` / `terminate()` on the async side send the appropriate
`Request` variant and return immediately (`Ok(())`).

## Thread Event Loop

```
XSelectInput(display, root, PointerMotionMask | ButtonPressMask | ButtonReleaseMask)

loop:
  // drain pending requests (non-blocking)
  while let Ok(req) = request_rx.try_recv():
    match req:
      Create(pos)  → clients.insert(pos)
      Destroy(pos) → clients.remove(pos); if active == pos { do_release() }
      Release      → do_release()
      Terminate    → break

  if XPending(display) > 0:
    XNextEvent(display, &mut event)
    match event.type:
      MotionNotify  → handle_motion(event.xmotion)
      ButtonPress   → if active: send(Input(PointerEvent::Button { state:1 }))
      ButtonRelease → if active: send(Input(PointerEvent::Button { state:0 }))
      KeyPress      → if active: send(Input(KeyboardEvent::Key { state:1 }))
      KeyRelease    → if active: send(Input(KeyboardEvent::Key { state:0 }))
  else:
    thread::sleep(Duration::from_millis(1))
```

## Edge Detection (handle_motion)

```
fn handle_motion(x, y):
  curr = (x, y)

  if active_client.is_none():
    pos = crossed_boundary(prev_pos, curr, screen_w, screen_h)
    if let Some(pos) = pos && clients.contains(pos):
      entry_point = clamp(curr, screen_w, screen_h)
      active_client = Some(pos)
      XGrabPointer(display, root, PointerMotionMask | ButtonMask, GrabModeAsync)
      XGrabKeyboard(display, root, GrabModeAsync)
      XWarpPointer(display, None, root, entry_point)
      XFlush(display)
      event_tx.try_send((pos, CaptureEvent::Begin))
  else:
    dx = (curr.x - entry_point.x) as f64
    dy = (curr.y - entry_point.y) as f64
    if dx != 0.0 || dy != 0.0:
      XWarpPointer(display, None, root, entry_point)  // lock cursor at edge
      XFlush(display)
      event_tx.try_send((active_client, CaptureEvent::Input(PointerEvent::Motion { dx, dy })))

  prev_pos = curr

fn crossed_boundary(prev, curr, w, h) -> Option<Position>:
  Left   if prev.x >  0   && curr.x <= 0
  Right  if prev.x < w-1  && curr.x >= w
  Top    if prev.y >  0   && curr.y <= 0
  Bottom if prev.y < h-1  && curr.y >= h

fn clamp(pos, w, h):
  x = pos.x.clamp(0, w-1)
  y = pos.y.clamp(0, h-1)
```

## Release (do_release)

```
fn do_release():
  XUngrabPointer(display, CurrentTime)
  XUngrabKeyboard(display, CurrentTime)
  XFlush(display)
  active_client = None
```

Cursor is left at `entry_point` (already warped there during capture). No extra warp on release —
the host will re-position via its own emulation.

## Keycode Translation

X11 `KeyPress`/`KeyRelease` events carry a `keycode` (hardware scancode + 8 offset).  
Linux evdev scancodes = X11 keycode - 8.  
The `input-event` crate's `scancode::Linux` enum uses Linux evdev values, so:

```rust
let linux_keycode = event.xkey.keycode - 8;
// use as key field in KeyboardEvent::Key { key: linux_keycode, state }
```

No symbol lookup needed — lan-mouse forwards raw scancodes.

## Struct Definition

```rust
pub struct X11InputCapture {
    event_rx:   tokio::sync::mpsc::Receiver<(Position, CaptureEvent)>,
    request_tx: std::sync::mpsc::SyncSender<Request>,
    thread:     Option<std::thread::JoinHandle<()>>,
}
```

`new()` spawns the thread, passes `event_tx` + `request_rx` into it, waits for a ready signal
(same pattern as Windows), returns `Ok(Self { ... })`.

## Error Handling

- `XOpenDisplay` returns null → `X11InputCaptureCreationError::OpenDisplayFailed`
- `XGrabPointer` returns non-zero → log warn, do not send `Begin` (another client may have grab)
- `event_tx.try_send` fails (full channel) → log warn, drop event (same as Windows)
- Thread exits unexpectedly → `poll_next` returns `None` (stream closed)

`X11InputCaptureCreationError` needs a new variant:

```rust
#[error("XOpenDisplay failed")]
OpenDisplayFailed,
```

## Multi-monitor

`XDisplayWidth`/`XDisplayHeight` return the combined virtual desktop size (e.g. 3840×1080 for two
1920×1080 side by side). Edge detection uses these total dimensions. This is correct for the
common case. Per-monitor barriers (for gaps between monitors) are out of scope for this
implementation — that is a Approach C (XFixesBarrier) concern.

## Files Changed

| File | Change |
|---|---|
| `input-capture/src/x11.rs` | Full implementation replacing the stub |
| `input-capture/src/error.rs` | Add `OpenDisplayFailed` variant to `X11InputCaptureCreationError` |

No other files need changes. Dependencies and feature flags are already wired up.

## Testing Plan

1. SSH into Mint (9JRKNG4 / 192.168.86.68), install Rust + `libx11-dev`
2. Clone this repo branch onto Mint, `cargo build --features x11`
3. Run `lan-mouse` on Mint, add Zorin (this machine) as client to the **right**
4. Run `lan-mouse` on Zorin, confirm Mint shows up as left client
5. Move mouse to left edge of Zorin → verify mouse appears on Mint
6. Move mouse to right edge of Mint → verify mouse returns to Zorin
7. Test keyboard forwarding while on Mint
8. Test button clicks while on Mint
9. Test release keybind (Ctrl+Shift+Super+Alt) as fallback
