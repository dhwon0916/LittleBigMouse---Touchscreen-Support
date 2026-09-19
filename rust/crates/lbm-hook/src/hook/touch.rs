//! State for resuming the mouse after Windows promotes touch to mouse messages.
//! Touch events are never swallowed or routed through the monitor-crossing engine.
//! No allocation, OS access, or locks: the Windows adapter owns cursor restoration.

pub fn is_touch(extra: usize) -> bool {
    // https://learn.microsoft.com/windows/win32/tablet/system-events-and-mouse-messages
    extra & 0xffff_ff80 == 0xff51_5780
}

pub fn is_pen(extra: usize) -> bool {
    extra & 0xffff_ff80 == 0xff51_5700
}

#[derive(Clone, Copy, Default)]
pub struct TouchResume {
    saved: Option<(i32, i32)>,
    contact: bool,
}

impl TouchResume {
    pub const fn new() -> Self {
        Self {
            saved: None,
            contact: false,
        }
    }

    pub fn touch(&mut self, cursor_before_touch: Option<(i32, i32)>, down: bool, up: bool) {
        if self.saved.is_none() {
            self.saved = cursor_before_touch;
        }
        if down {
            self.contact = true;
        }
        if up {
            self.contact = false;
        }
    }

    pub fn resume(&mut self) -> Option<(i32, i32)> {
        if self.contact {
            None
        } else {
            self.saved.take()
        }
    }

    pub fn in_contact(&self) -> bool {
        self.contact
    }

    pub fn pending(&self) -> bool {
        self.saved.is_some() || self.contact
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signatures_distinguish_mouse_pen_and_touch() {
        for id in 0..128 {
            assert!(is_touch(0xff51_5780 | id));
            assert!(!is_pen(0xff51_5780 | id));
            assert!(is_pen(0xff51_5700 | id));
            assert!(!is_touch(0xff51_5700 | id));
        }
        assert!(!is_touch(0));
        assert!(!is_touch(0x80));
    }

    #[test]
    fn repeated_taps_preserve_original_position_and_restore_once() {
        let mut state = TouchResume::new();
        state.touch(Some((-500, 200)), true, false);
        assert_eq!(state.resume(), None); // never warp during a touch drag
        state.touch(Some((1500, 700)), false, true);
        state.touch(Some((1600, 800)), true, false);
        state.touch(Some((1600, 800)), false, true);
        assert_eq!(state.resume(), Some((-500, 200)));
        assert_eq!(state.resume(), None);
    }

    #[test]
    fn reset_discards_pending_restore_and_contact() {
        let mut state = TouchResume::new();
        state.touch(Some((10, 20)), true, false);
        state.reset();
        assert_eq!(state.resume(), None);
        state.touch(Some((30, 40)), false, false);
        assert_eq!(state.resume(), Some((30, 40)));
    }

    #[test]
    fn failed_cursor_read_does_not_invent_a_position() {
        let mut state = TouchResume::new();
        state.touch(None, true, false);
        state.touch(None, false, true);
        assert_eq!(state.resume(), None);
    }
}
