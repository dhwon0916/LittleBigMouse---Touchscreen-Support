# AppBar swipe loses the saved mouse position

Reported: 2026-09-23. Affected build: touchscreen 1.0.1.

Reproduction: leave the physical mouse on the main monitor, swipe the widget
panel on the touchscreen, release, then move the physical mouse. Expected: the
mouse resumes at its original position. Observed: it resumes on the touchscreen.

The first local input recording contains native touchscreen HID contact/release
reports and no promoted touch mouse messages during the swipes. The first
physical movement after the first swipe remains at the touch location instead
of returning to the saved main-monitor position. No keyboard input was recorded.

Local diagnostic recordings (ignored build artifacts):
- `artifacts/appbar-swipe-first-capture.log`
- `artifacts/appbar-swipe-capture.log`

Root cause: the widget's swipe handlers hide, expand, or reveal its AppBar,
which changes its work-area reservation. LittleBigMouse currently tears down
the hook on SPI_SETWORKAREA, even when monitor bounds do not change. Reinstalling
the hook clears touch restoration state. The follow-up recording captured
WM_SETTINGCHANGE / SPI_SETWORKAREA (message 0x1A, parameter 0x2F) at
21:21:17.723Z and 21:21:24.443Z; the latter followed the raw finger-up report at
21:21:24.432Z. The second recording also includes promoted touch events, so the
reset affects both native and promoted restoration paths.

Fix: skip the work-area-triggered teardown/reload only while touch restoration
is pending, the touch feature and hook are active, and the attached monitors'
full bounds exactly match the loaded main zones. Routing uses full monitor
bounds, so an AppBar reservation alone does not invalidate that layout. Actual
display-change messages, changed/unknown bounds, disabled touch, and explicit
layout reloads retain their reset behavior.

Regression coverage checks native and promoted pending states, moved/resized/
missing/unknown monitors, disabled touch, and explicit layout invalidation.
Validation: 120 hook/engine tests passed, as did Clippy with warnings denied,
formatting, whitespace checks, and the release build. The user confirmed that
the corrected preview restores the mouse after swiping the AppBar on 2026-09-23.

## Typing-focus follow-up

The user then reported that typing focus still failed to return. Focus logging
showed an unchanged input timestamp and active feature, but the request's touched
window was Explorer's Progman while the foreground window was an Electron panel.
The panel had moved away from the release point before the native settling timer
hit-tested it, so the exact foreground guard correctly rejected the wrong target.

Native release now prefers the activated foreground window when it differs from
the captured original window; nonactivating touches still use hit-testing. The
worker retains its exact identity, input timestamp, generation, sequence, and
per-app rule checks. Tests cover both target selection and real cross-process
editor restoration with a release point outside the panel. The interactive test
passed, with native handles remaining at 138 across 500 restorations. The user
confirmed typing restoration after swiping with preview `swipe.2` on 2026-09-23.

## Cleanup validation

The final cleanup removes the unused mouse timestamp argument and defers touched
window lookup until focus restoration is enabled. Temporary diagnostic launchers
were removed; the local recordings remain available. Regression checks also cover
paused/inactive hooks, virtual layouts, and a contended engine lock.

442 Rust tests and 728 C# tests passed, along with Clippy (warnings denied),
formatting, whitespace checks, and the release build. The interactive check passed
on retry after Windows refused the fixture's initial foreground activation;
500 restorations kept native handle usage at 140. Build: `swipe.3`.
