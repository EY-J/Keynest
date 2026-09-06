# WebView policy (Phase 11)

Phase 11: production CSP is based on inspected Vite output (module scripts and CSS under `/assets/`, local font files/PNG, canvas/WebGL). Scripts/fonts/styles are self-only; image data URLs are permitted; no eval, frames, workers, objects, remote code, base changes or form navigation. `style-src-attr 'unsafe-inline'` preserves React strength-meter and canvas sizing attributes, not script execution. Tauri's automatic script hashes/nonces remain enabled. Connect is restricted to `ipc:` and Windows `http://ipc.localhost`; no general network fetch permission. The separate development CSP allows Vite inline/HMR behavior at localhost:1420 only; production does not.

Validation: bundled-output/CSP regression tests, frontend build and Rust embedded-asset compilation. The in-app browser connection failed before bootstrap (`sandboxPolicy` metadata missing). Actual production launch, auth/settings/clipboard/data-folder flows and CSP console/remote-script-block checks remain manual and must be completed before Phase 11 is checked off. Do not infer browser enforcement from a string assertion.

Reference: [Tauri CSP handling](https://v2.tauri.app/security/csp/).

## Remaining native acceptance checklist

Use a disposable test profile, not real secrets. Launch the embedded-assets binary built with `cargo build --features tauri/custom-protocol` (not a Vite browser tab). Confirm setup/unlock, settings changes, explicit credential copy and the data-folder button work. Check that bundled fonts/images/PixelBlast/strength controls render without CSP errors. In WebView developer tools, verify a deliberately inserted remote test script is rejected by CSP (use a harmless fixture URL and no sensitive payload); retain only the violation result, never passwords/keys. Repeat using the distributed release build before release. The development server's more permissive policy does not validate the production policy.
