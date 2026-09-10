use std::{io::Write, ptr::null_mut, time::Instant};
use thiserror::Error;
use windows_sys::Win32::{
    Foundation::{ERROR_FILE_NOT_FOUND, ERROR_PIPE_BUSY},
    Storage::FileSystem::{
        CreateFileW, FILE_FLAG_OVERLAPPED, FILE_READ_DATA, FILE_WRITE_DATA, OPEN_EXISTING,
        SECURITY_IDENTIFICATION, SECURITY_SQOS_PRESENT, SYNCHRONIZE,
    },
    System::Pipes::{GetNamedPipeServerSessionId, WaitNamedPipeW},
};
use zeroize::Zeroizing;

use super::{
    io::{owned, Event, Pipe},
    security::{wide, Identity},
    CONNECT_TIMEOUT, EXCHANGE_TIMEOUT, RECEIPT,
};
use crate::autofill::{
    framing::{read_frame, write_frame},
    valid_request,
};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PipeError {
    #[error("KeyNest desktop app is not running.")]
    AppNotRunning,
    #[error("KeyNest local Autofill connection is unavailable.")]
    IpcUnavailable,
    #[error("KeyNest could not understand the Autofill request.")]
    InvalidRequest,
}

/// Thin client for the future Native Messaging host. Never opens vault files or
/// acquires key material. Returned JSON must remain transient and must not log.
pub fn forward_autofill_request(request: &[u8]) -> Result<Zeroizing<Vec<u8>>, PipeError> {
    if !valid_request(request) {
        return Err(PipeError::InvalidRequest);
    }
    let identity = Identity::current().map_err(|_| PipeError::IpcUnavailable)?;
    forward_at(request, &identity.pipe_name(), identity.session)
}

pub(super) fn forward_at(
    request: &[u8],
    name: &str,
    session: u32,
) -> Result<Zeroizing<Vec<u8>>, PipeError> {
    if !valid_request(request) {
        return Err(PipeError::InvalidRequest);
    }
    let pipe = connect(name)?;
    let mut server_session = 0;
    if unsafe { GetNamedPipeServerSessionId(pipe.raw(), &mut server_session) } == 0
        || server_session != session
    {
        return Err(PipeError::IpcUnavailable);
    }
    let stop = Event::new().map_err(|_| PipeError::IpcUnavailable)?;
    let mut stream = pipe.stream(&stop, Instant::now() + EXCHANGE_TIMEOUT);
    write_frame(&mut stream, request).map_err(|_| PipeError::IpcUnavailable)?;
    let response = read_frame(&mut stream).map_err(|_| PipeError::IpcUnavailable)?;
    std::str::from_utf8(&response).map_err(|_| PipeError::IpcUnavailable)?;
    stream
        .write_all(&[RECEIPT])
        .map_err(|_| PipeError::IpcUnavailable)?;
    Ok(response)
}

pub(super) fn connect(name: &str) -> Result<Pipe, PipeError> {
    let name = wide(name);
    let deadline = Instant::now() + CONNECT_TIMEOUT;
    loop {
        // SAFETY: fixed local-only name, non-inheritable overlapped handle. SQOS
        // permits identification only, preventing server-side impersonation use.
        let raw = unsafe {
            CreateFileW(
                name.as_ptr(),
                FILE_READ_DATA | FILE_WRITE_DATA | SYNCHRONIZE,
                0,
                null_mut(),
                OPEN_EXISTING,
                FILE_FLAG_OVERLAPPED | SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION,
                null_mut(),
            )
        };
        match owned(raw) {
            Ok(handle) => return Ok(Pipe(handle)),
            Err(error) if error.raw_os_error() == Some(ERROR_FILE_NOT_FOUND as i32) => {
                return Err(PipeError::AppNotRunning)
            }
            Err(error) if error.raw_os_error() == Some(ERROR_PIPE_BUSY as i32) => {
                if Instant::now() >= deadline {
                    return Err(PipeError::IpcUnavailable);
                }
                let timeout = deadline
                    .saturating_duration_since(Instant::now())
                    .as_millis()
                    .clamp(1, 50) as u32;
                unsafe {
                    WaitNamedPipeW(name.as_ptr(), timeout);
                }
            }
            Err(_) => return Err(PipeError::IpcUnavailable),
        }
    }
}
