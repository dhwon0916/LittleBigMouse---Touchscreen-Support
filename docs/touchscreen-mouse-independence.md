# Touchscreen mouse independence (Windows)

In Options, enable **Keep mouse position after touch**, then apply the configuration.
Tap or drag on a touchscreen, lift your finger, then move the physical mouse.
That first movement returns the pointer to its previous mouse position; subsequent
movement uses LittleBigMouse's normal monitor crossing and DPI behavior.

This first implementation is integrated into the UI and Rust daemon. Both must be
rebuilt together; it is not a DLL that can be copied into an existing installation.
The option defaults to off. Older configuration files continue to work.

## Behavior and limits

- Touch-promoted mouse messages pass through without monitor remapping or suppression.
- Repeated taps preserve the original mouse position until the mouse resumes.
- A touch drag delays restoration until its promoted left-button release.
- The first physical mouse movement is consumed to restore the saved position.
- Move the mouse before clicking or scrolling after touch. A physical button or
  wheel event instead accepts the current position and cancels restoration.
- Pen input cancels restoration. Injected mouse events do not trigger restoration.
- Disabling the option, reloading a layout, reinstalling the hook, or switching
  desktops discards any pending restoration. Existing stop/rescue behavior applies.
- Keyboard focus is unchanged unless **Restore keyboard focus after touch** is also
  enabled. This is sequential touch/mouse use, not multiple simultaneous pointers.

## Optional keyboard-focus restoration

Enable both switches and apply. After lifting your finger, wait briefly before
typing: a background worker attempts to reactivate the previously foreground app
after the configured delay (120 ms by default; adjustable from 0 to 5000 ms). The touched button or control still receives its touch.
The option defaults to off and is persisted with the other global options.

The attempt is cancelled by newer keyboard/mouse input, another touch, a configuration
reload, stop/pause, desktop switch, or a switch to a different foreground window.
Newer touch releases can schedule a fresh attempt. A closed, hidden, or minimized
original window is not activated. This restores the foreground app, not the specific
text field when touching two controls inside the same app.

The second preview captures the native editor control as well as its parent window.
Its worker briefly connects the relevant input queues with `AttachThreadInput`,
calls `SetForegroundWindow` and `SetFocus`, verifies the focus, and detaches on every
exit path. It skips windows with active menus/capture, held modifiers/buttons, or
an unresponsive app. No synthetic keystrokes or global keyboard hooks are used.
Custom UI frameworks without a native editor handle still rely on their own focus
restoration when activated. Windows can still refuse an activation.

The worker writes bounded diagnostic status to
`%LOCALAPPDATA%\\Mgth\\LittleBigMouse\\TouchFocus.log`. It records numeric window
handles and idle timestamps, never text, titles, or keystrokes. A
`touch-options worker started` entry identifies this version; subsequent entries
distinguish skipped, refused, partial, and successful restorations.

To test, leave a caret in Notepad on the main display, tap a control in a different
app on the touchscreen, lift your finger, wait half a second, and type. Text should
go to Notepad. Also verify that disabling the switch leaves typing in the touched
app, and that closing Notepad or deliberately switching elsewhere cancels restoration.

The keyboard-focus feature has compiled and passed its automated guard, persistence,
and protocol tests. The second preview also passes an interactive Windows test
that restores a native editor from a separate background process. The touchscreen
event timing still needs user verification.

Detection uses Windows' documented touch signature in promoted mouse events:
[System Events and Mouse Messages](https://learn.microsoft.com/en-us/windows/win32/tablet/system-events-and-mouse-messages).
Drivers, remote sessions, or applications that do not produce these tagged events
need hardware testing and may require a different input backend.

## Touchscreen preferences

All preferences are under Options, in the touchscreen card. Apply/save the layout
after editing. Old layouts retain the previous behavior: all displays, 120 ms,
restoration after finger-up, no override key, and no app rules.

- **Restore when the mouse moves:** retains the touched app's keyboard focus
  until a physical mouse move after finger-up. The return attempt is immediate;
  the touch-release delay does not apply. Typing in the touched app while waiting
  is allowed. A physical click/wheel, pen, stop, or layout reload cancels the pending
  return. Injected movements and mouse motion during a touch drag do not trigger it.
- **Use on all displays:** turn off and check the desired displays. An empty
  selection enables neither touch feature anywhere. Selections use display source
  IDs and are retained for disconnected displays. Bounds are recomputed with each
  layout, including panels excluded from ordinary cursor routing. Cloned/mirrored
  displays with identical bounds cannot be distinguished by promoted mouse events.
- **Temporary override:** choose None, Ctrl, Alt, Shift, or Win. Hold the key
  before tapping to bypass both touch features through that gesture, even if the
  modifier is released before finger-up. The modifier is not swallowed and may
  also affect the touched app's normal behavior.
- **Per-app focus rules:** semicolon-separated executable basenames, matched
  exactly without case sensitivity (e.g. chrome.exe; notepad.exe). The keep-focus
  list wins. The restore-only list restricts restoration to listed apps; leaving
  it empty restores after touching any app not in the keep list. These rules
  leave mouse-position independence active and are separate from Excluded.txt.
  They match the process owning the touched top-level window; apps using a shared
  host process cannot be distinguished by title. If process resolution fails
  while rules are configured, focus is left alone.

The native Windows regression test verifies the configurable delay, an app-rule
block, waiting for movement, click cancellation, and restoration of a native editor
across processes. Actual touchscreen event timing still needs hardware verification.

## Validation

Required build dependencies: .NET SDK 10 (see `global.json`), a Rust toolchain with
Windows build tools, and the repository's HLab.Core / HLab.Avalonia submodules.

From the repository root:

```powershell
git submodule update --init --recursive
dotnet test LittleBigMouse.Core/LittleBigMouse.DisplayLayout.Tests/LittleBigMouse.DisplayLayout.Tests.csproj
dotnet build LittleBigMouse.Ui/LittleBigMouse.Ui.Avalonia/LittleBigMouse.Ui.Avalonia.csproj
cargo test --manifest-path rust/Cargo.toml -p lbm-hook -p lbm-layout -p lbm-store
cargo fmt --manifest-path rust/Cargo.toml --all -- --check
```

Automated coverage includes touch/pen signatures, repeated taps, touch-drag release,
missing cursor positions, state reset, opt-in XML parsing, startup replay, and
configuration persistence. Shared snapshot expectations include the new default-off
field in C# and Rust output.

Hardware acceptance checks (still required):

1. With the option off, confirm ordinary mouse and touch behavior.
2. Enable it; park the mouse on the main display, tap a button on the touchscreen,
   then move the mouse. The button must activate and the mouse must resume at its
   original location.
3. Repeat several taps before moving, then test a touch drag and release.
4. Repeat with mixed DPI, negative desktop coordinates, and both crossing algorithms.
5. Confirm normal mouse clicks, dragging, scrolling, and border crossings after
   restoration. Confirm the documented click-before-move behavior.
6. Test disabling, Stop/Start, layout changes, excluded apps, desktop switching,
   and the rescue shortcut while a restoration is pending.
7. Restart LittleBigMouse and confirm the saved option is retained.

The user confirmed mouse-position independence works on their touchscreen setup. The Rust hook, layout, and store regression suites pass, as do all 365 C# display-layout tests. The remaining hardware acceptance cases above have not all been verified.
