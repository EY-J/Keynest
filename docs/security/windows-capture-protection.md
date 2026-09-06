# Phase 10: Windows capture protection

## Implemented scope

The existing main Tauri window retains `contentProtected: true` in its configuration. Before window creation, Rust overrides it to false only when `cfg!(debug_assertions)` is true. Thus `npm run tauri dev` allows screenshots for UI development; normal `npm run tauri build`/release binaries preserve protection automatically. This follows Rust's build profile, not the frontend build or a user setting (an explicitly debug-built bundle also allows screenshots).

In release builds, all current sensitive views (master-password entry, recovery display and credential reveal/editor) share the protected window. Protection stays enabled when dialogs close or navigation changes; Settings/Home are protected too. There is no per-view toggle, extra window, capture hook, or frontend permission to disable it.

This is the safest feasible scope in the current single-WebView architecture: Windows display affinity applies to a process-owned top-level HWND, not a DOM element. Route-based asynchronous enable/disable calls would introduce exposure gaps and overlapping-dialog cleanup races. Whole-window protection also means legitimate KeyNest screenshots/screen sharing may be blank or exclude the window.

The installed Tauri/Tao implementation maps content protection to `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)`. Microsoft documents support from Windows 10 version 2004, with older systems using `WDA_MONITOR` behavior. DWM composition is required. The runtime currently ignores the native setter's return value; the configuration alone is not proof of protection on every system.

## Validation performed

- Config regression tests verify the protected default, the effective configuration for the active Rust build profile, and absence of a frontend protection-toggle permission.
- A fixture-only native Windows test creates a hidden top-level window, sets display affinity, reads it back successfully, checks the window still exists, resets affinity on the fixture and destroys it. No screenshots, real profile access, user clipboard changes or workstation lock are performed by these tests.
- This verifies API availability/readback on the current Windows host only. It does **not** verify rendered WebView content, Snipping Tool, screen recording, other Windows versions or GPU drivers.

## Justified acceptance-test deferral

Phase 10 remains unchecked until its actual desktop tests are completed. This environment provides code/native smoke tests but not a verified interactive path to inspect the native KeyNest WebView and common Windows capture tools. Browser-only rendering tests cannot exercise native HWND display affinity. It would be misleading to substitute the hidden-window test for the required screenshot and normal-rendering checks.

Using only disposable fixture credentials in a separate test profile, validate:

1. Launch a release-built Windows app (not `tauri dev` or a debug binary). Check setup/unlock, recovery display, credential Reveal/Edit, focus, resize, modal close and navigation render and work normally. Separately confirm screenshots work in a debug build using only disposable fixture credentials.
2. Try Snipping Tool (`Win+Shift+S`) and a supported Windows screen-recording/display-sharing path while those fixture views are visible. Confirm KeyNest is omitted or blank, recording the OS build, capture tool/version and observed result. Do not capture real keys/passwords.
3. Close sensitive dialogs and navigate to Settings/Home: capture protection should remain enabled by design, with no rendering regression or crash. No restoration is required for this whole-window scope.
4. Repeat on supported Windows releases, including the oldest supported one. Older Windows may blank the window instead of omitting it. If a supported setup fails or breaks rendering, document it and revisit the native scope/fallback before marking complete.

This is defense in depth, not a guarantee against all capture methods, cameras, privileged malware, administrators or active process-memory access. No protection claims are made for untested non-Windows platforms.

References: [Microsoft display-affinity API](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowdisplayaffinity), [Tauri window configuration](https://v2.tauri.app/reference/config/#contentprotected). Local implementation inspected: `tao-0.35.3/src/platform_impl/windows/window.rs` (`set_content_protection`, `init_window`).
