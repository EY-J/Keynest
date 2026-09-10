mod native_host;
mod protocol;
mod response;
mod service;
mod url_match;

pub use native_host::run_native_host;

#[cfg(any(windows, test))]
pub(crate) mod framing;

#[cfg(windows)]
pub(crate) fn valid_request(bytes: &[u8]) -> bool {
    protocol::Request::parse(bytes).is_ok()
}

pub use service::AutofillService;
pub(crate) use service::{
    HostApprovalCandidate, HostApprovalCompleted, HostApprovalError, PendingHostApprovalView,
    TauriHostApprovalEventSink,
};

#[cfg(test)]
mod tests;
