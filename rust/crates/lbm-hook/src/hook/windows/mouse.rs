//! Low-level mouse hook callback â€” port of `Hooker::MouseCallback`.
//!
//! The one genuinely hot, genuinely `unsafe` function. It dedups by previous
//! location (C++ `static previousLocation`), then hands the position to the
//! engine under a **non-blocking** `try_lock`: if the lock isn't free (a `Load`
//! is swapping the layout), the event passes straight through â€” never blocking,
//! so the callback stays well under the `LowLevelHooksTimeout`. The whole body
//! is wrapped in `catch_unwind` so a panic can't unwind across the FFI boundary.

use std::cell::Cell;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::Ordering;

use windows::Win32::Foundation::{LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetCursorPos, SetCursorPos, HHOOK, LLMHF_INJECTED, MSLLHOOKSTRUCT,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE,
};

use crate::geometry::Point;
use crate::hook::hot_path::{count_event, route_move, MoveDedup};
use crate::hook::touch::{is_pen, is_touch, TouchResume};
use crate::hook::touch_policy::{Modifier, TouchGesture};
use crate::platform::cursor::Win32Cursor;
use crate::shared::SHARED;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};

fn modifier_held(modifier: Modifier) -> bool {
    let held = |key: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY|
        unsafe { GetAsyncKeyState(key.0 as i32) } < 0;
    match modifier {
        Modifier::None => false,
        Modifier::Ctrl => held(VK_CONTROL),
        Modifier::Alt => held(VK_MENU),
        Modifier::Shift => held(VK_SHIFT),
        Modifier::Win => held(VK_LWIN) || held(VK_RWIN),
    }
}

thread_local! {
    /// C++ `static previousLocation`. Thread-local because the callback only ever
    /// runs on the pump thread. The dedup logic itself lives in the neutral
    /// `hot_path` core, so the branch below is exactly what the benchmark measures.
    static PREV: Cell<MoveDedup> = const { Cell::new(MoveDedup::new()) };
    static TOUCH: Cell<TouchResume> = const { Cell::new(TouchResume::new()) };
    static GENERATION: Cell<u32> = const { Cell::new(0) };
    static MOUSE_POSITION: Cell<Option<(i32, i32)>> = const { Cell::new(None) };
    static GESTURE: Cell<TouchGesture> = const { Cell::new(TouchGesture::new()) };
}

fn cursor_position() -> Option<(i32, i32)> {
    let mut point = POINT::default();
    unsafe { GetCursorPos(&mut point) }
        .ok()
        .map(|_| (point.x, point.y))
}

pub fn reset() {
    GESTURE.with(|v| v.set(TouchGesture::new()));
    super::focus_restore::reset();
    TOUCH.with(|state| state.set(TouchResume::new()));
    PREV.with(|prev| prev.set(MoveDedup::new()));
    MOUSE_POSITION.with(|position| position.set(cursor_position()));
}

/// # Safety
/// Invoked by Windows as a `WH_MOUSE_LL` hook procedure; never call it directly.
/// When `code >= 0`, `lparam` points to a valid `MSLLHOOKSTRUCT`, as guaranteed
/// by the OS.
pub unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let handled = catch_unwind(AssertUnwindSafe(|| process(code, wparam, lparam))).unwrap_or(false);

    if handled {
        LRESULT(1) // the engine or touch-resume code has positioned the cursor
    } else {
        unsafe { CallNextHookEx(HHOOK::default(), code, wparam, lparam) }
    }
}

fn process(code: i32, wparam: WPARAM, lparam: LPARAM) -> bool {
    if code < 0 || lparam.0 == 0 {
        return false;
    }
    let ms = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
    if let Some(shared) = SHARED.get() {
        let generation = shared.touch_generation.load(Ordering::SeqCst);
        GENERATION.with(|old| {
            if old.replace(generation) != generation {
                reset();
            }
        });
        if shared.touch_mouse_independent.load(Ordering::SeqCst)
            && shared.want_hook.load(Ordering::SeqCst)
            && !shared.paused.load(Ordering::SeqCst)
            && !shared.suspended.load(Ordering::SeqCst)
        {
            if is_touch(ms.dwExtraInfo) {
                let down = wparam.0 == WM_LBUTTONDOWN as usize;
                let up = wparam.0 == WM_LBUTTONUP as usize;
                let allowed = shared.touch_policy.try_lock().ok().is_some_and(|policy| {
                    policy.includes(ms.pt.x, ms.pt.y) && !modifier_held(policy.modifier)
                });
                let accepted = GESTURE.with(|cell| {
                    let mut gesture = cell.get();
                    let accepted = gesture.accepts(allowed, down, up);
                    cell.set(gesture);
                    accepted
                });
                if !accepted {
                    super::focus_restore::reset();
                    TOUCH.with(|v| v.set(TouchResume::new()));
                    MOUSE_POSITION.with(|v| v.set(Some((ms.pt.x, ms.pt.y))));
                    count_event();
                    return false;
                }
                super::focus_restore::on_touch(shared, wparam.0 as u32, ms.pt, ms.time);
                // Use the last mouse position, including LBM's border warps.
                // The OS cursor may already reflect a preceding touch message.
                let cursor = MOUSE_POSITION.with(Cell::get);
                TOUCH.with(|cell| {
                    let mut state = cell.get();
                    state.touch(
                        cursor,
                        wparam.0 == WM_LBUTTONDOWN as usize,
                        wparam.0 == WM_LBUTTONUP as usize,
                    );
                    cell.set(state);
                });
                count_event(); // touch activity must not trip the hook watchdog
                return false;
            }
            if is_pen(ms.dwExtraInfo) {
                GESTURE.with(|v| v.set(TouchGesture::new()));
                super::focus_restore::reset();
                TOUCH.with(|cell| cell.set(TouchResume::new()));
                MOUSE_POSITION.with(|position| position.set(Some((ms.pt.x, ms.pt.y))));
                return false;
            }
            // Programmatic cursor motion must not consume the pending restore.
            if ms.flags & LLMHF_INJECTED == 0
                && shared.restore_keyboard_focus.load(Ordering::SeqCst)
            {
                super::focus_restore::on_physical_input(
                    shared,
                    wparam.0 == WM_MOUSEMOVE as usize,
                    GESTURE.with(|v| v.get().contact),
                );
            }
            if ms.flags & LLMHF_INJECTED != 0 && TOUCH.with(|cell| cell.get().pending()) {
                return false;
            }
            if wparam.0 == WM_MOUSEMOVE as usize {
                if TOUCH.with(|cell| cell.get().in_contact()) {
                    count_event();
                    return false;
                }
                let target = TOUCH.with(|cell| {
                    let mut state = cell.get();
                    let target = state.resume();
                    cell.set(state); // clear before SetCursorPos can re-enter
                    target
                });
                if let Some((x, y)) = target {
                    PREV.with(|prev| prev.set(MoveDedup::new()));
                    if unsafe { SetCursorPos(x, y) }.is_ok() {
                        MOUSE_POSITION.with(|position| position.set(cursor_position()));
                        count_event();
                        return true;
                    }
                }
            } else {
                // A physical click/wheel takes ownership at the current position.
                // Do not relocate an in-progress physical drag on its next move.
                TOUCH.with(|cell| cell.set(TouchResume::new()));
                MOUSE_POSITION.with(|position| position.set(Some((ms.pt.x, ms.pt.y))));
            }
        } else {
            TOUCH.with(|cell| cell.set(TouchResume::new()));
        }
    }
    if !is_mouse_move_message(code, wparam, lparam) {
        return false;
    }

    let loc = (ms.pt.x, ms.pt.y);

    let changed = PREV.with(|prev| {
        let mut dedup = prev.get();
        let changed = dedup.accept(loc);
        prev.set(dedup);
        changed
    });
    if !changed {
        return false;
    }
    count_event();

    let Some(shared) = SHARED.get() else {
        return false;
    };

    // The whole non-blocking route â€” contended lock passes through, poisoned lock
    // is recovered, crossing counted â€” lives in the neutral `hot_path` core, so
    // the Windows callback and the benchmark exercise the same decision.
    let mut env = Win32Cursor;
    let handled = route_move(&shared.engine, &mut env, Point::new(loc.0, loc.1)).handled();
    MOUSE_POSITION.with(|position| {
        position.set(if handled {
            cursor_position()
        } else {
            Some(loc)
        });
    });
    handled
}

// The C++ hook filtered with `(wParam & WM_MOUSEMOVE) != 0`, but every mouse
// message carries bit 0x200 (WM_LBUTTONDOWN = 0x201, WM_MOUSEWHEEL = 0x20Aâ€¦):
// clicks and wheel events entered the engine as moves, and a click landing on
// a border mid-crossing could be swallowed by the LRESULT(1) return.
fn is_mouse_move_message(code: i32, wparam: WPARAM, lparam: LPARAM) -> bool {
    code >= 0 && lparam.0 != 0 && wparam.0 == WM_MOUSEMOVE as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::UI::WindowsAndMessaging::{
        WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEHWHEEL, WM_MOUSEWHEEL,
        WM_RBUTTONDOWN, WM_RBUTTONUP, WM_XBUTTONDOWN, WM_XBUTTONUP,
    };

    #[test]
    fn only_mouse_move_can_enter_the_routing_engine() {
        let pointer = LPARAM(1);
        assert!(is_mouse_move_message(
            0,
            WPARAM(WM_MOUSEMOVE as usize),
            pointer
        ));

        for message in [
            WM_LBUTTONDOWN,
            WM_LBUTTONUP,
            WM_RBUTTONDOWN,
            WM_RBUTTONUP,
            WM_MBUTTONDOWN,
            WM_MBUTTONUP,
            WM_MOUSEWHEEL,
            WM_MOUSEHWHEEL,
            WM_XBUTTONDOWN,
            WM_XBUTTONUP,
        ] {
            assert!(!is_mouse_move_message(0, WPARAM(message as usize), pointer));
        }
    }
}
