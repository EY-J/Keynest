//! Native API smoke tests, not screen-capture/rendering acceptance tests.
use windows_sys::{
    core::w,
    Win32::{Foundation::HWND, UI::WindowsAndMessaging::*},
};

struct TestWindow(HWND);
impl Drop for TestWindow {
    fn drop(&mut self) {
        // SAFETY: the test creates and destroys this hidden window on the same thread.
        unsafe {
            DestroyWindow(self.0);
        }
    }
}

#[test]
fn configured_windows_enable_capture_protection_before_webview_content() {
    let config: serde_json::Value =
        serde_json::from_str(include_str!("../../tauri.conf.json")).unwrap();
    let windows = config["app"]["windows"].as_array().unwrap();
    assert!(!windows.is_empty());
    for window in windows {
        assert_eq!(window["contentProtected"], true);
    }
    // No frontend capability should be able to switch protection off.
    let capabilities = include_str!("../../capabilities/default.json");
    assert!(!capabilities.contains("allow-set-content-protected"));
}

#[test]
fn effective_capture_config_allows_debug_but_preserves_release_protection() {
    let mut config: tauri::Config =
        serde_json::from_str(include_str!("../../tauri.conf.json")).unwrap();
    assert!(!config.app.windows.is_empty());
    crate::configure_capture_protection(&mut config);
    for window in config.app.windows {
        assert_eq!(window.content_protected, !cfg!(debug_assertions));
    }
}

#[test]
fn windows_accepts_and_reports_capture_affinity_without_destroying_the_window() {
    // A hidden, fixture-only top-level window. Never captures pixels, shows user data,
    // changes the real KeyNest window or modifies the user's clipboard/session.
    unsafe {
        let window = TestWindow(CreateWindowExW(
            0,
            w!("STATIC"),
            w!("KeyNest capture test"),
            WS_OVERLAPPED,
            0,
            0,
            64,
            64,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null(),
        ));
        assert!(!window.0.is_null(), "{}", std::io::Error::last_os_error());
        assert_ne!(
            SetWindowDisplayAffinity(window.0, WDA_EXCLUDEFROMCAPTURE),
            0,
            "{}",
            std::io::Error::last_os_error()
        );
        let mut affinity = WDA_NONE;
        assert_ne!(
            GetWindowDisplayAffinity(window.0, &mut affinity),
            0,
            "{}",
            std::io::Error::last_os_error()
        );
        assert!(matches!(affinity, WDA_EXCLUDEFROMCAPTURE | WDA_MONITOR));
        assert_ne!(IsWindow(window.0), 0);
        // Probe reversibility on the fixture only; release application windows stay protected.
        assert_ne!(SetWindowDisplayAffinity(window.0, WDA_NONE), 0);
        assert_ne!(GetWindowDisplayAffinity(window.0, &mut affinity), 0);
        assert_eq!(affinity, WDA_NONE);
    }
}
