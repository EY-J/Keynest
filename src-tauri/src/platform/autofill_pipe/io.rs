use std::{
    io::{self, Read, Write},
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    ptr::null_mut,
    time::Instant,
};

use windows_sys::Win32::{
    Foundation::{
        GetLastError, BOOL, ERROR_IO_PENDING, ERROR_PIPE_CONNECTED, HANDLE, INVALID_HANDLE_VALUE,
        WAIT_OBJECT_0, WAIT_TIMEOUT,
    },
    Storage::FileSystem::{ReadFile, WriteFile},
    System::{
        Pipes::ConnectNamedPipe,
        Threading::{
            CreateEventW, SetEvent, WaitForMultipleObjects, WaitForSingleObject, INFINITE,
        },
        IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED},
    },
};

pub(super) struct Event(OwnedHandle);
impl Event {
    pub fn new() -> io::Result<Self> {
        // SAFETY: creates an unnamed, non-inheritable manual-reset event.
        let handle = unsafe { CreateEventW(null_mut(), 1, 0, null_mut()) };
        owned(handle).map(Self)
    }
    pub fn set(&self) {
        // SAFETY: this event is owned and live. No security data crosses the API.
        unsafe {
            SetEvent(self.0.as_raw_handle());
        }
    }
    pub fn is_set(&self) -> bool {
        unsafe { WaitForSingleObject(self.0.as_raw_handle(), 0) == WAIT_OBJECT_0 }
    }
    #[cfg(test)]
    pub fn wait(&self, milliseconds: u32) -> bool {
        unsafe { WaitForSingleObject(self.0.as_raw_handle(), milliseconds) == WAIT_OBJECT_0 }
    }
}

pub(super) fn owned(handle: HANDLE) -> io::Result<OwnedHandle> {
    if handle.is_null() || handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: call sites transfer a newly created handle exactly once.
    Ok(unsafe { OwnedHandle::from_raw_handle(handle) })
}

pub(super) struct Pipe(pub OwnedHandle);
impl Pipe {
    pub fn raw(&self) -> HANDLE {
        self.0.as_raw_handle()
    }

    pub fn connect(&self, stop: &Event) -> io::Result<()> {
        self.operation(stop, None, true, |overlapped| {
            // SAFETY: pipe is overlapped; operation() owns/drains the OVERLAPPED.
            unsafe { ConnectNamedPipe(self.raw(), overlapped) }
        })
        .map(|_| ())
    }

    fn operation(
        &self,
        stop: &Event,
        deadline: Option<Instant>,
        connecting: bool,
        start: impl FnOnce(*mut OVERLAPPED) -> BOOL,
    ) -> io::Result<u32> {
        if stop.is_set() {
            return Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "Autofill IPC stopped",
            ));
        }
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Autofill IPC timed out",
            ));
        }
        let event = Event::new()?;
        // SAFETY: zero initialization is valid; stack address and event remain
        // stable until completion, including cancellation, before returning.
        let mut overlapped: OVERLAPPED = unsafe { std::mem::zeroed() };
        overlapped.hEvent = event.0.as_raw_handle();
        let started = start(&mut overlapped);
        let code = if started == 0 {
            unsafe { GetLastError() }
        } else {
            0
        };
        if connecting && code == ERROR_PIPE_CONNECTED {
            return Ok(0);
        }
        if code != 0 && code != ERROR_IO_PENDING {
            return Err(io::Error::from_raw_os_error(code as i32));
        }
        if code == ERROR_IO_PENDING {
            let timeout = deadline.map_or(INFINITE, |deadline| {
                deadline
                    .saturating_duration_since(Instant::now())
                    .as_millis()
                    .min(u128::from(INFINITE - 1)) as u32
            });
            let handles = [stop.0.as_raw_handle(), event.0.as_raw_handle()];
            let wait = unsafe { WaitForMultipleObjects(2, handles.as_ptr(), 0, timeout) };
            if wait != WAIT_OBJECT_0 + 1 {
                // Cancellation requests are not completion. Always drain before
                // buffers/OVERLAPPED can move or drop, even on ERROR_NOT_FOUND.
                unsafe {
                    CancelIoEx(self.raw(), &overlapped);
                    let mut transferred = 0;
                    GetOverlappedResult(self.raw(), &overlapped, &mut transferred, 1);
                }
                let kind = if wait == WAIT_TIMEOUT {
                    io::ErrorKind::TimedOut
                } else {
                    // Read::read_exact/Write::write_all retry Interrupted forever.
                    // Shutdown is terminal, not a retryable signal interruption.
                    io::ErrorKind::ConnectionAborted
                };
                return Err(io::Error::new(kind, "Autofill IPC cancelled"));
            }
        }
        let mut transferred = 0;
        if unsafe { GetOverlappedResult(self.raw(), &overlapped, &mut transferred, 0) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(transferred)
    }

    pub fn stream<'a>(&'a self, stop: &'a Event, deadline: Instant) -> PipeStream<'a> {
        PipeStream {
            pipe: self,
            stop,
            deadline,
        }
    }
}

pub(super) struct PipeStream<'a> {
    pipe: &'a Pipe,
    stop: &'a Event,
    deadline: Instant,
}
impl Read for PipeStream<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let length = bytes.len().min(u32::MAX as usize) as u32;
        self.pipe
            .operation(self.stop, Some(self.deadline), false, |overlapped| {
                // SAFETY: operation does not return until this borrowed buffer is no
                // longer accessed by the kernel, including timeout/stop cancellation.
                unsafe {
                    ReadFile(
                        self.pipe.raw(),
                        bytes.as_mut_ptr(),
                        length,
                        null_mut(),
                        overlapped,
                    )
                }
            })
            .map(|n| n as usize)
    }
}
impl Write for PipeStream<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let length = bytes.len().min(u32::MAX as usize) as u32;
        self.pipe
            .operation(self.stop, Some(self.deadline), false, |overlapped| unsafe {
                WriteFile(
                    self.pipe.raw(),
                    bytes.as_ptr(),
                    length,
                    null_mut(),
                    overlapped,
                )
            })
            .map(|n| n as usize)
    }
    // Never FlushFileBuffers: it waits for the client to consume all bytes and
    // would let a non-reading client hold the vault's lock gate indefinitely.
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
