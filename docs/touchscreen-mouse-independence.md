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
- Keyboard focus is unchanged. This is sequential touch/mouse use, not multiple
  simultaneous independent pointers.

Detection uses Windows' documented touch signature in promoted mouse events:
[System Events and Mouse Messages](https://learn.microsoft.com/en-us/windows/win32/tablet/system-events-and-mouse-messages).
Drivers, remote sessions, or applications that do not produce these tagged events
need hardware testing and may require a different input backend.

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
