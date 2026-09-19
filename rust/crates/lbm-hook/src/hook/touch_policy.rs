//! Validated touchscreen policy. Parsed on layout load, never in a mouse callback.
use crate::zones::ZonesLayout;

#[derive(Clone, Debug)]
pub struct TouchPolicy {
    pub delay_ms: u64,
    pub on_mouse_move: bool,
    pub all_displays: bool,
    pub bounds: Vec<[f64; 4]>,
    pub modifier: Modifier,
    keep_apps: Vec<String>,
    restore_apps: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Modifier {
    None,
    Ctrl,
    Alt,
    Shift,
    Win,
}

/// Keep an override or unselected display bypassed for the entire gesture,
/// including trailing promoted moves after finger-up.
#[derive(Clone, Copy, Default)]
pub struct TouchGesture {
    bypass: bool,
    pub contact: bool,
}

impl TouchGesture {
    pub const fn new() -> Self {
        Self {
            bypass: false,
            contact: false,
        }
    }
    pub fn accepts(&mut self, allowed: bool, down: bool, up: bool) -> bool {
        if !self.contact && (down || !self.bypass) {
            self.bypass = !allowed;
        }
        if down {
            self.contact = true;
        }
        if up {
            self.contact = false;
        }
        !self.bypass
    }
}

fn apps(value: &str) -> Vec<String> {
    value
        .split([';', '\r', '\n'])
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_ascii_lowercase)
        .collect()
}

impl TouchPolicy {
    pub fn from_layout(layout: &ZonesLayout) -> Self {
        Self {
            delay_ms: layout.focus_restore_delay.clamp(0, 5000) as u64,
            on_mouse_move: layout.focus_restore_on_mouse_move,
            all_displays: layout.touch_all_displays,
            bounds: layout
                .touch_display_bounds
                .split(';')
                .filter_map(|rect| {
                    let values: Vec<f64> = rect
                        .split(',')
                        .map(str::parse)
                        .collect::<Result<_, _>>()
                        .ok()?;
                    if values.len() != 4
                        || values.iter().any(|v| !v.is_finite())
                        || values[2] <= 0.0
                        || values[3] <= 0.0
                    {
                        return None;
                    }
                    Some([values[0], values[1], values[2], values[3]])
                })
                .collect(),
            modifier: match layout.touch_override_modifier.to_ascii_lowercase().as_str() {
                "ctrl" => Modifier::Ctrl,
                "alt" => Modifier::Alt,
                "shift" => Modifier::Shift,
                "win" => Modifier::Win,
                _ => Modifier::None,
            },
            keep_apps: apps(&layout.focus_keep_apps),
            restore_apps: apps(&layout.focus_restore_apps),
        }
    }

    pub fn includes(&self, x: i32, y: i32) -> bool {
        self.all_displays
            || self.bounds.iter().any(|b| {
                x as f64 >= b[0]
                    && (x as f64) < b[0] + b[2]
                    && y as f64 >= b[1]
                    && (y as f64) < b[1] + b[3]
            })
    }

    /// Exact executable basenames, case-insensitive. Keep wins over restore.
    /// An unresolved process is safe only when there are no app rules to enforce.
    pub fn restores_app(&self, path: Option<&str>) -> bool {
        if self.keep_apps.is_empty() && self.restore_apps.is_empty() {
            return true;
        }
        let Some(path) = path else {
            return false;
        };
        let name = path
            .rsplit(['\\', '/'])
            .next()
            .unwrap_or(path)
            .to_ascii_lowercase();
        !self.keep_apps.contains(&name)
            && (self.restore_apps.is_empty() || self.restore_apps.contains(&name))
    }
}

impl Default for TouchPolicy {
    fn default() -> Self {
        Self::from_layout(&ZonesLayout::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_survives_modifier_release_and_trailing_moves() {
        let mut gesture = TouchGesture::new();
        assert!(!gesture.accepts(false, true, false));
        assert!(!gesture.accepts(true, false, false));
        assert!(!gesture.accepts(true, false, true));
        assert!(!gesture.accepts(true, false, false));
        assert!(gesture.accepts(true, true, false));
        // Moving across a display boundary mid-drag does not change its policy.
        assert!(gesture.accepts(false, false, false));
        assert!(gesture.contact);
    }

    #[test]
    fn defaults_and_delay_limits() {
        let mut layout = ZonesLayout::default();
        let p = TouchPolicy::from_layout(&layout);
        assert_eq!(p.delay_ms, 120);
        assert!(!p.on_mouse_move);
        assert!(p.includes(-10000, 50));
        assert_eq!(p.modifier, Modifier::None);
        layout.focus_restore_delay = -1;
        assert_eq!(TouchPolicy::from_layout(&layout).delay_ms, 0);
        layout.focus_restore_delay = 99999;
        assert_eq!(TouchPolicy::from_layout(&layout).delay_ms, 5000);
    }

    #[test]
    fn selected_displays_and_empty_selection() {
        let mut l = ZonesLayout::default();
        l.touch_all_displays = false;
        assert!(!TouchPolicy::from_layout(&l).includes(0, 0));
        l.touch_display_bounds = "-1920,0,1920,1080;bad;0,0,NaN,100".into();
        let p = TouchPolicy::from_layout(&l);
        assert!(p.includes(-1920, 0));
        assert!(p.includes(-1, 1079));
        assert!(!p.includes(0, 0));
        assert!(!p.includes(-1, 1080));
    }

    #[test]
    fn app_rules_are_exact_and_keep_takes_precedence() {
        let mut l = ZonesLayout::default();
        l.focus_keep_apps = " chrome.exe ; Editor.exe".into();
        l.focus_restore_apps = "control.exe;editor.exe".into();
        let p = TouchPolicy::from_layout(&l);
        assert!(p.restores_app(Some(r"C:\Tools\CONTROL.EXE")));
        for path in [
            None,
            Some("editor.exe"),
            Some("chrome.exe"),
            Some("mycontrol.exe"),
            Some("other.exe"),
        ] {
            assert!(!p.restores_app(path));
        }
        l.focus_restore_apps.clear();
        assert!(TouchPolicy::from_layout(&l).restores_app(Some("other.exe")));
    }
}
