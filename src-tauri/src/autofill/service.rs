use std::{
    io,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use serde::Serialize;
use tauri::{Emitter, Manager};
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::{
    ipc::PublicIpcError,
    security::{AuthError, AuthService, AuthStatus, SecurityOperationGate},
    vault::{VaultError, VaultRecordInput, VaultService},
};

use super::{
    protocol::{
        encode_error, encode_response, ApprovalRequestStatus, ErrorCode, FillPayload, MatchSummary,
        Operation, Request, ResponseData, Status,
    },
    url_match::HttpsHost,
};

pub(crate) const HOST_APPROVAL_EVENT: &str = "keynest://host-approval-requested";
const PENDING_APPROVAL_TTL: Duration = Duration::from_secs(5 * 60);

pub(crate) trait HostApprovalEventSink: Send + Sync {
    fn requested(&self) -> Result<(), ()>;
}

struct NoopHostApprovalEventSink;

impl HostApprovalEventSink for NoopHostApprovalEventSink {
    fn requested(&self) -> Result<(), ()> {
        Ok(())
    }
}

pub(crate) struct TauriHostApprovalEventSink {
    app: tauri::AppHandle,
}

impl TauriHostApprovalEventSink {
    pub(crate) fn new(app: tauri::AppHandle) -> Self {
        Self { app }
    }
}

impl HostApprovalEventSink for TauriHostApprovalEventSink {
    fn requested(&self) -> Result<(), ()> {
        if let Some(window) = self.app.get_webview_window("main") {
            let _ = window.unminimize();
            let _ = window.show();
            let _ = window.set_focus();
        }
        self.app.emit(HOST_APPROVAL_EVENT, ()).map_err(|_| ())
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PendingHostApprovalView {
    pub(crate) approval_id: String,
    pub(crate) requested_host: String,
}

#[derive(Serialize, Zeroize, ZeroizeOnDrop)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HostApprovalCandidate {
    pub(crate) credential_id: String,
    pub(crate) name: String,
    pub(crate) username: String,
    pub(crate) website: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HostApprovalCompleted {
    pub(crate) credential_name: String,
    pub(crate) requested_host: String,
    pub(crate) changed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HostApprovalError {
    Locked,
    Unavailable,
    CredentialNotFound,
    Internal,
}

struct PendingHostApproval {
    approval_id: String,
    requested_host: String,
    created_at: Instant,
}

/// Desktop-owned Autofill boundary. Contains shared services and the one
/// short-lived, memory-only host-approval request. It never caches vault keys,
/// passwords, matches, or browser authorization.
#[derive(Clone)]
pub struct AutofillService {
    auth: AuthService,
    vault: VaultService,
    operation_gate: SecurityOperationGate,
    pending_approval: Arc<Mutex<Option<PendingHostApproval>>>,
    approval_sequence: Arc<AtomicU64>,
    approval_events: Arc<dyn HostApprovalEventSink>,
}

impl AutofillService {
    pub(crate) fn new(
        auth: AuthService,
        vault: VaultService,
        operation_gate: SecurityOperationGate,
    ) -> Self {
        Self {
            auth,
            vault,
            operation_gate,
            pending_approval: Arc::new(Mutex::new(None)),
            approval_sequence: Arc::new(AtomicU64::new(0)),
            approval_events: Arc::new(NoopHostApprovalEventSink),
        }
    }

    pub(crate) fn with_host_approval_events(
        mut self,
        approval_events: Arc<dyn HostApprovalEventSink>,
    ) -> Self {
        self.approval_events = approval_events;
        self
    }

    pub fn dispatch(
        &self,
        request_json: &[u8],
        deliver: impl FnOnce(&[u8]) -> io::Result<()>,
    ) -> io::Result<()> {
        let request = match Request::parse(request_json) {
            Ok(request) => request,
            Err(invalid) => {
                let bytes = encode_error(invalid.request_id.as_deref(), ErrorCode::InvalidRequest)
                    .map_err(|_| io::Error::other("Autofill response unavailable"))?;
                return deliver(&bytes);
            }
        };
        let guard = self.operation_gate.lock();
        let result = match &request.operation {
            Operation::Status => Ok(self.status()),
            Operation::QueryMatches { page_url } => self.query_matches(page_url),
            Operation::RequestFill {
                page_url,
                credential_id,
            } => self.request_fill(page_url, credential_id),
            Operation::RequestHostApproval { page_url } => self.request_host_approval(page_url),
        };
        let notify_host_approval =
            result.is_ok() && matches!(request.operation, Operation::RequestHostApproval { .. });
        let encoded = match result {
            Ok(data) => encode_response(&request.request_id, &data),
            Err(code) => encode_error(Some(&request.request_id), code),
        };
        let bytes = encoded
            .or_else(|_| encode_error(Some(&request.request_id), ErrorCode::InternalError))
            .map_err(|_| io::Error::other("Autofill response unavailable"))?;
        let delivered = deliver(&bytes);
        // The frontend notification immediately reads the pending request
        // through a Tauri command that acquires this same gate. Notify only
        // after the native response is written and the protected operation is
        // released, preventing a re-entrant WebView IPC stall.
        drop(guard);
        if delivered.is_ok() && notify_host_approval {
            let _ = self.approval_events.requested();
        }
        delivered
    }

    fn status(&self) -> ResponseData {
        ResponseData::Status {
            state: match self.auth.status() {
                AuthStatus::Unlocked => Status::Unlocked,
                AuthStatus::Locked | AuthStatus::SetupRequired => Status::Locked,
                AuthStatus::DataError => Status::Error,
            },
        }
    }

    fn query_matches(&self, page_url: &str) -> Result<ResponseData, ErrorCode> {
        self.auth
            .require_vault_key(|key| {
                let host = HttpsHost::parse(page_url)?;
                let summaries = self.vault.list(key).map_err(PublicIpcError::from)?;
                let matches: Vec<_> = summaries
                    .into_iter()
                    .filter(|record| {
                        host.matches_saved(record.website.as_deref(), &record.allowed_login_hosts)
                    })
                    .map(|record| MatchSummary {
                        credential_id: record.id,
                        name: record.name,
                        username: record.username,
                    })
                    .collect();
                if matches.is_empty() {
                    return Err(ErrorCode::NoMatches);
                }
                Ok(ResponseData::Matches { matches })
            })
            .map_err(PublicIpcError::from)?
    }

    fn request_fill(&self, page_url: &str, credential_id: &str) -> Result<ResponseData, ErrorCode> {
        self.auth
            .require_vault_key(|key| {
                let host = HttpsHost::parse(page_url)?;
                let mut record = self
                    .vault
                    .get(key, credential_id)
                    .map_err(PublicIpcError::from)?;
                if !host.matches_saved(record.website.as_deref(), &record.allowed_login_hosts) {
                    return Err(ErrorCode::DomainMismatch);
                }
                Ok(ResponseData::FillPayload(FillPayload {
                    username: std::mem::take(&mut record.username),
                    password: std::mem::take(&mut record.password),
                }))
            })
            .map_err(PublicIpcError::from)?
    }

    fn request_host_approval(&self, page_url: &str) -> Result<ResponseData, ErrorCode> {
        let requested_host = HttpsHost::parse(page_url)?.canonical().to_owned();
        // Alternate login hosts are persisted as DNS-like exact names. Do not
        // admit wildcard or ambiguous single-label values into an approval UI.
        if requested_host.contains('*') || !requested_host.contains('.') {
            return Err(ErrorCode::UnsupportedUrl);
        }
        let sequence = self
            .approval_sequence
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .map_err(|_| ErrorCode::InternalError)?;
        let approval_id = format!("approval-{sequence:016x}");
        {
            let mut pending = self
                .pending_approval
                .lock()
                .map_err(|_| ErrorCode::InternalError)?;
            *pending = Some(PendingHostApproval {
                approval_id: approval_id.clone(),
                requested_host,
                created_at: Instant::now(),
            });
        }
        Ok(ResponseData::HostApprovalRequested {
            state: ApprovalRequestStatus::Requested,
        })
    }

    pub(crate) fn pending_host_approval(
        &self,
    ) -> Result<Option<PendingHostApprovalView>, HostApprovalError> {
        let _guard = self.operation_gate.lock();
        self.pending_view_unlocked()
    }

    pub(crate) fn host_approval_candidates(
        &self,
        approval_id: &str,
    ) -> Result<Vec<HostApprovalCandidate>, HostApprovalError> {
        let _guard = self.operation_gate.lock();
        self.require_pending_unlocked(approval_id)?;
        let mut candidates = self
            .auth
            .require_vault_key(|key| self.vault.list(key))
            .map_err(map_auth_error)?
            .map_err(map_vault_error)?;
        candidates.sort_by(|left, right| {
            right
                .website
                .is_some()
                .cmp(&left.website.is_some())
                .then_with(|| {
                    left.name
                        .to_ascii_lowercase()
                        .cmp(&right.name.to_ascii_lowercase())
                        .then_with(|| left.id.cmp(&right.id))
                })
        });
        Ok(candidates
            .into_iter()
            .map(|record| HostApprovalCandidate {
                credential_id: record.id,
                name: record.name,
                username: record.username,
                website: record.website,
            })
            .collect())
    }

    pub(crate) fn approve_login_host(
        &self,
        approval_id: &str,
        credential_id: &str,
    ) -> Result<HostApprovalCompleted, HostApprovalError> {
        let _guard = self.operation_gate.lock();
        let pending = self.require_pending_unlocked(approval_id)?;
        let requested_host = HttpsHost::parse(&format!("https://{}/", pending.requested_host))
            .map_err(|_| HostApprovalError::Internal)?;
        let result = self
            .auth
            .require_vault_key(|key| {
                let mut record = self.vault.get(key, credential_id)?;
                let credential_name = record.name.clone();
                let changed = !requested_host
                    .matches_saved(record.website.as_deref(), &record.allowed_login_hosts);
                if changed {
                    record
                        .allowed_login_hosts
                        .push(pending.requested_host.clone());
                    let input = VaultRecordInput {
                        name: std::mem::take(&mut record.name),
                        username: std::mem::take(&mut record.username),
                        password: std::mem::take(&mut record.password),
                        website: record.website.take(),
                        allowed_login_hosts: std::mem::take(&mut record.allowed_login_hosts),
                        tags: std::mem::take(&mut record.tags),
                    };
                    self.vault.update(key, credential_id, input)?;
                }
                Ok::<_, VaultError>((credential_name, changed))
            })
            .map_err(map_auth_error)?
            .map_err(map_vault_error)?;
        self.clear_pending_unlocked(approval_id)?;
        Ok(HostApprovalCompleted {
            credential_name: result.0,
            requested_host: pending.requested_host,
            changed: result.1,
        })
    }

    pub(crate) fn cancel_host_approval(&self, approval_id: &str) -> Result<(), HostApprovalError> {
        let _guard = self.operation_gate.lock();
        self.require_pending_unlocked(approval_id)?;
        self.clear_pending_unlocked(approval_id)
    }

    fn pending_view_unlocked(&self) -> Result<Option<PendingHostApprovalView>, HostApprovalError> {
        let mut pending = self
            .pending_approval
            .lock()
            .map_err(|_| HostApprovalError::Internal)?;
        if pending
            .as_ref()
            .is_some_and(|request| request.created_at.elapsed() > PENDING_APPROVAL_TTL)
        {
            *pending = None;
        }
        Ok(pending.as_ref().map(|request| PendingHostApprovalView {
            approval_id: request.approval_id.clone(),
            requested_host: request.requested_host.clone(),
        }))
    }

    fn require_pending_unlocked(
        &self,
        approval_id: &str,
    ) -> Result<PendingHostApprovalView, HostApprovalError> {
        self.pending_view_unlocked()?
            .filter(|pending| pending.approval_id == approval_id)
            .ok_or(HostApprovalError::Unavailable)
    }

    fn clear_pending_unlocked(&self, approval_id: &str) -> Result<(), HostApprovalError> {
        let mut pending = self
            .pending_approval
            .lock()
            .map_err(|_| HostApprovalError::Internal)?;
        if pending
            .as_ref()
            .is_some_and(|request| request.approval_id == approval_id)
        {
            *pending = None;
            return Ok(());
        }
        Err(HostApprovalError::Unavailable)
    }
}

fn map_auth_error(error: AuthError) -> HostApprovalError {
    match error {
        AuthError::Unauthorized | AuthError::NotInitialized => HostApprovalError::Locked,
        _ => HostApprovalError::Internal,
    }
}

fn map_vault_error(error: VaultError) -> HostApprovalError {
    match error {
        VaultError::NotFound => HostApprovalError::CredentialNotFound,
        _ => HostApprovalError::Internal,
    }
}
