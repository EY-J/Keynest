//! Native notifications on a hidden window, separate from the WebView event loop.
//! Lock/clipboard I/O must not block the UI thread that clipboard plugins may use.
use std::{io, ptr::null_mut, sync::mpsc, thread};

use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    System::{
        LibraryLoader::GetModuleHandleW,
        RemoteDesktop::{
            WTSRegisterSessionNotification, WTSUnRegisterSessionNotification,
            NOTIFY_FOR_THIS_SESSION,
        },
    },
    UI::WindowsAndMessaging::*,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EnvironmentLock {
    Session,
    Sleep,
}

fn lock_trigger(message: u32, event: usize) -> Option<EnvironmentLock> {
    match (message, event as u32) {
        // Console/remote disconnect, logoff, and workstation lock. Unlock never unlocks KeyNest.
        (
            WM_WTSSESSION_CHANGE,
            WTS_CONSOLE_DISCONNECT | WTS_REMOTE_DISCONNECT | WTS_SESSION_LOGOFF | WTS_SESSION_LOCK,
        ) => Some(EnvironmentLock::Session),
        (WM_POWERBROADCAST, PBT_APMSUSPEND | PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMESUSPEND) => {
            Some(EnvironmentLock::Sleep)
        }
        _ => None,
    }
}

struct Context(Box<dyn Fn(EnvironmentLock) + Send>);

pub(crate) struct SessionMonitor {
    // HWND is only used to post shutdown, never dereferenced across threads.
    window: usize,
}

impl SessionMonitor {
    pub(crate) fn start(on_lock: impl Fn(EnvironmentLock) + Send + 'static) -> io::Result<Self> {
        let (ready, result) = mpsc::sync_channel(1);
        thread::Builder::new()
            .name("keynest-session-lock".into())
            .spawn(move || {
                run(Context(Box::new(on_lock)), ready);
            })?;
        result
            .recv()
            .map_err(|_| io::Error::other("session monitor failed to start"))?
            .map(|window| Self { window })
    }
}

impl Drop for SessionMonitor {
    fn drop(&mut self) {
        // No join on the UI thread: an in-flight clipboard operation may need that thread.
        unsafe {
            PostMessageW(self.window as HWND, WM_CLOSE, 0, 0);
        }
    }
}

fn run(context: Context, ready: mpsc::SyncSender<io::Result<usize>>) {
    let class_name: Vec<u16> = format!("KeyNestSessionMonitor{:?}\0", thread::current().id())
        .encode_utf16()
        .collect();
    let context = Box::new(context);
    // SAFETY: this thread owns the window and stable context until after DestroyWindow.
    // The hidden top-level window receives power broadcasts (message-only windows do not).
    unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        let class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: class_name.as_ptr(),
            ..std::mem::zeroed()
        };
        if RegisterClassW(&class) == 0 {
            let _ = ready.send(Err(io::Error::last_os_error()));
            return;
        }
        let window = CreateWindowExW(
            0,
            class_name.as_ptr(),
            class_name.as_ptr(),
            0,
            0,
            0,
            0,
            0,
            null_mut(),
            null_mut(),
            instance,
            (&*context as *const Context).cast(),
        );
        if window.is_null() {
            let _ = ready.send(Err(io::Error::last_os_error()));
            UnregisterClassW(class_name.as_ptr(), instance);
            return;
        }
        if WTSRegisterSessionNotification(window, NOTIFY_FOR_THIS_SESSION) == 0 {
            let _ = ready.send(Err(io::Error::last_os_error()));
            DestroyWindow(window);
            UnregisterClassW(class_name.as_ptr(), instance);
            return;
        }
        if ready.send(Ok(window as usize)).is_ok() {
            let mut message = std::mem::zeroed();
            while GetMessageW(&mut message, null_mut(), 0, 0) > 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        WTSUnRegisterSessionNotification(window);
        DestroyWindow(window);
        UnregisterClassW(class_name.as_ptr(), instance);
    }
}

unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_NCCREATE {
        let create = &*(lparam as *const CREATESTRUCTW);
        SetWindowLongPtrW(window, GWLP_USERDATA, create.lpCreateParams as isize);
    }
    if message == WM_CLOSE {
        PostQuitMessage(0);
        return 0;
    }
    if let Some(trigger) = lock_trigger(message, wparam) {
        let context = GetWindowLongPtrW(window, GWLP_USERDATA) as *const Context;
        if !context.is_null() {
            // Never unwind across the native callback boundary.
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ((*context).0)(trigger)))
                .is_err()
            {
                std::process::abort();
            }
        }
        return 1;
    }
    DefWindowProcW(window, message, wparam, lparam)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::{Arc, Mutex},
        time::Duration,
    };

    #[test]
    fn only_security_relevant_native_messages_trigger_lock() {
        for event in [
            WTS_SESSION_LOCK,
            WTS_CONSOLE_DISCONNECT,
            WTS_REMOTE_DISCONNECT,
            WTS_SESSION_LOGOFF,
        ] {
            assert_eq!(
                lock_trigger(WM_WTSSESSION_CHANGE, event as usize),
                Some(EnvironmentLock::Session)
            );
        }
        for event in [WTS_SESSION_UNLOCK, WTS_CONSOLE_CONNECT, WTS_REMOTE_CONNECT] {
            assert_eq!(lock_trigger(WM_WTSSESSION_CHANGE, event as usize), None);
        }
        for message in [WM_KILLFOCUS, WM_SIZE] {
            assert_eq!(lock_trigger(message, 0), None);
        }
        for event in [PBT_APMSUSPEND, PBT_APMRESUMEAUTOMATIC, PBT_APMRESUMESUSPEND] {
            assert_eq!(
                lock_trigger(WM_POWERBROADCAST, event as usize),
                Some(EnvironmentLock::Sleep)
            );
        }
    }

    #[test]
    fn native_window_delivers_lock_notifications_and_shuts_down() {
        let (sent, received) = mpsc::channel();
        let dropped = Arc::new(Mutex::new(Some(sent)));
        let callback = dropped.clone();
        let monitor = SessionMonitor::start(move |event| {
            callback
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .send(event)
                .unwrap();
        })
        .unwrap();
        unsafe {
            PostMessageW(
                monitor.window as HWND,
                WM_WTSSESSION_CHANGE,
                WTS_SESSION_LOCK as usize,
                0,
            );
        }
        assert_eq!(
            received.recv_timeout(Duration::from_secs(2)).unwrap(),
            EnvironmentLock::Session
        );
        drop(monitor);
        // The callback's only remaining owner is the window thread until native cleanup completes.
        drop(dropped);
        assert!(matches!(
            received.recv_timeout(Duration::from_secs(2)),
            Err(mpsc::RecvTimeoutError::Disconnected)
        ));
    }
}
