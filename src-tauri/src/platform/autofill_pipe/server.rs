use std::{
    io::{self, Read},
    mem::size_of,
    sync::Arc,
    thread,
    time::Instant,
};

use windows_sys::Win32::{
    Foundation::{ERROR_BROKEN_PIPE, ERROR_NO_DATA, ERROR_PIPE_NOT_CONNECTED},
    Security::SECURITY_ATTRIBUTES,
    Storage::FileSystem::{
        FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OVERLAPPED, PIPE_ACCESS_DUPLEX,
    },
    System::Pipes::{
        CreateNamedPipeW, DisconnectNamedPipe, GetNamedPipeClientSessionId, PIPE_READMODE_BYTE,
        PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_WAIT,
    },
};

use super::{
    io::{owned, Event, Pipe},
    security::{wide, Identity},
    EXCHANGE_TIMEOUT, RECEIPT, RECEIPT_TIMEOUT, REQUEST_TIMEOUT, WRITE_TIMEOUT,
};
use crate::{
    autofill::framing::{read_frame, write_frame},
    AutofillService,
};

pub(crate) struct PipeServer {
    stop: Arc<Event>,
    finished: Arc<Event>,
}

impl PipeServer {
    pub(crate) fn start(service: AutofillService) -> io::Result<Self> {
        let identity = Identity::current()?;
        Self::start_at(service, &identity.pipe_name(), &identity)
    }

    pub(super) fn start_at(
        service: AutofillService,
        name: &str,
        identity: &Identity,
    ) -> io::Result<Self> {
        let pipe = create_pipe(name, identity)?;
        let stop = Arc::new(Event::new()?);
        let finished = Arc::new(Event::new()?);
        let worker_stop = stop.clone();
        let worker_finished = finished.clone();
        let session = identity.session;
        thread::Builder::new()
            .name("keynest-autofill-pipe".into())
            .spawn(move || {
                let _completion = Completion(worker_finished);
                serve(pipe, service, session, &worker_stop);
            })?;
        Ok(Self { stop, finished })
    }

    /// Nonblocking on the Tauri event-loop thread. The worker cancels/drains I/O
    /// and closes its pipe; existing protected operations retain their ordering.
    pub(crate) fn shutdown(&self) {
        if !self.finished.is_set() {
            self.stop.set();
        }
    }

    #[cfg(test)]
    pub(super) fn start_fixture(service: AutofillService, name: &str) -> io::Result<Self> {
        Self::start_at(service, name, &Identity::current()?)
    }

    #[cfg(test)]
    pub(super) fn wait_stopped(&self) -> bool {
        self.finished.wait(3000)
    }
}

impl Drop for PipeServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}
struct Completion(Arc<Event>);
impl Drop for Completion {
    fn drop(&mut self) {
        self.0.set();
    }
}

pub(super) fn create_pipe(name: &str, identity: &Identity) -> io::Result<Pipe> {
    let descriptor = identity.descriptor()?;
    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: 0,
    };
    let name = wide(name);
    // SAFETY: descriptor/name outlive the creation call. Own a single overlapped
    // instance for the server lifetime, reconnecting without dropping the name.
    let raw = unsafe {
        CreateNamedPipeW(
            name.as_ptr(),
            PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED | FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            4096,
            4096,
            0,
            &attributes,
        )
    };
    owned(raw).map(Pipe)
}

fn serve(pipe: Pipe, service: AutofillService, session: u32, stop: &Event) {
    while !stop.is_set() {
        if let Err(error) = pipe.connect(stop) {
            if !stop.is_set()
                && matches!(
                    error.raw_os_error().map(|code| code as u32),
                    Some(ERROR_BROKEN_PIPE | ERROR_NO_DATA | ERROR_PIPE_NOT_CONNECTED)
                )
            {
                // A client can connect and disappear before accept completes.
                unsafe {
                    DisconnectNamedPipe(pipe.raw());
                }
                continue;
            }
            break;
        }
        let _ = exchange(&pipe, &service, session, stop);
        // Pending operations have already completed or been cancelled/drained.
        // Disconnect clears residual OS buffers; the server handle stays owned.
        unsafe {
            DisconnectNamedPipe(pipe.raw());
        }
    }
}

fn exchange(pipe: &Pipe, service: &AutofillService, session: u32, stop: &Event) -> io::Result<()> {
    let mut client_session = 0;
    if unsafe { GetNamedPipeClientSessionId(pipe.raw(), &mut client_session) } == 0
        || client_session != session
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Autofill IPC session mismatch",
        ));
    }
    let request = read_frame(&mut pipe.stream(stop, Instant::now() + REQUEST_TIMEOUT))?;
    let exchange_deadline = Instant::now() + EXCHANGE_TIMEOUT;
    service.dispatch(&request, |response| {
        // Includes frame header and all partial writes in one short deadline.
        // The stop event also prevents queued operations sending during shutdown.
        write_frame(
            &mut pipe.stream(stop, exchange_deadline.min(Instant::now() + WRITE_TIMEOUT)),
            response,
        )
    })?;
    drop(request);
    // A receipt lets DisconnectNamedPipe avoid discarding an unread successful
    // response. This wait is OUTSIDE the security gate and holds no plaintext.
    let mut receipt = [0];
    pipe.stream(stop, Instant::now() + RECEIPT_TIMEOUT)
        .read_exact(&mut receipt)?;
    if receipt != [RECEIPT] {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid Autofill IPC receipt",
        ));
    }
    Ok(())
}
