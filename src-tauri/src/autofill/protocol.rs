use std::io::{self, Write};

use serde::{Deserialize, Deserializer, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use crate::ipc::PublicIpcError;

pub(super) const VERSION: u8 = 1;
pub(super) const MAX_MESSAGE_BYTES: usize = 64 * 1024;
const MAX_PAGE_URL_BYTES: usize = 8192;

// No Debug: page URLs and identifiers must not reach diagnostics.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireRequest {
    version: u32,
    request_id: String,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default, deserialize_with = "present_string")]
    page_url: Option<String>,
    #[serde(default, deserialize_with = "present_string")]
    credential_id: Option<String>,
}

fn present_string<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<String>, D::Error> {
    String::deserialize(deserializer).map(Some)
}

pub(super) struct Request {
    pub request_id: String,
    pub operation: Operation,
}

pub(super) enum Operation {
    Status,
    QueryMatches {
        page_url: String,
    },
    RequestFill {
        page_url: String,
        credential_id: String,
    },
    RequestHostApproval {
        page_url: String,
    },
}

pub(super) struct InvalidRequest {
    pub request_id: Option<String>,
}

impl Request {
    pub(super) fn parse(bytes: &[u8]) -> Result<Self, InvalidRequest> {
        let uncorrelated = || InvalidRequest { request_id: None };
        if bytes.is_empty() || bytes.len() > MAX_MESSAGE_BYTES {
            return Err(uncorrelated());
        }
        // Serde structs can also deserialize positional arrays. The V1 wire
        // contract accepts objects only, even if such an array has valid fields.
        if bytes.iter().find(|byte| !byte.is_ascii_whitespace()) != Some(&b'{') {
            return Err(uncorrelated());
        }
        // Direct typed deserialization also rejects duplicate fields and trailing JSON.
        let wire: WireRequest = serde_json::from_slice(bytes).map_err(|_| uncorrelated())?;
        if wire.request_id.is_empty()
            || wire.request_id.len() > 128
            || !wire
                .request_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(uncorrelated());
        }
        let invalid = || InvalidRequest {
            request_id: Some(wire.request_id.clone()),
        };
        if wire.version != u32::from(VERSION)
            || wire
                .page_url
                .as_ref()
                .is_some_and(|url| url.len() > MAX_PAGE_URL_BYTES)
        {
            return Err(invalid());
        }
        let operation = match (
            wire.kind.as_str(),
            wire.page_url.as_ref(),
            wire.credential_id.as_ref(),
        ) {
            ("status", None, None) => Operation::Status,
            ("queryMatches", Some(page_url), None) => Operation::QueryMatches {
                page_url: page_url.clone(),
            },
            ("requestHostApproval", Some(page_url), None) => Operation::RequestHostApproval {
                page_url: page_url.clone(),
            },
            ("requestFill", Some(page_url), Some(id))
                if id.len() == 32
                    && id
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) =>
            {
                Operation::RequestFill {
                    page_url: page_url.clone(),
                    credential_id: id.clone(),
                }
            }
            _ => return Err(invalid()),
        };
        Ok(Self {
            request_id: wire.request_id,
            operation,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ErrorCode {
    VaultLocked,
    UnsupportedUrl,
    NoMatches,
    CredentialNotFound,
    DomainMismatch,
    InvalidRequest,
    InternalError,
    AppNotRunning,
    IpcUnavailable,
}

impl ErrorCode {
    pub(super) fn public(self) -> PublicIpcError {
        let (code, message) = match self {
            Self::AppNotRunning => ("APP_NOT_RUNNING", "KeyNest desktop app is unavailable."),
            Self::IpcUnavailable => (
                "IPC_UNAVAILABLE",
                "KeyNest local Autofill connection is unavailable.",
            ),
            Self::VaultLocked => ("VAULT_LOCKED", "KeyNest is locked."),
            Self::UnsupportedUrl => (
                "UNSUPPORTED_URL",
                "This website is not supported for Autofill.",
            ),
            Self::NoMatches => ("NO_MATCHES", "No saved login matches this website."),
            Self::CredentialNotFound => ("CREDENTIAL_NOT_FOUND", "The credential was not found."),
            Self::DomainMismatch => (
                "DOMAIN_MISMATCH",
                "This saved login does not match the current website.",
            ),
            Self::InvalidRequest => (
                "INVALID_REQUEST",
                "KeyNest could not understand the Autofill request.",
            ),
            Self::InternalError => (
                "INTERNAL_ERROR",
                "KeyNest could not complete the Autofill request.",
            ),
        };
        PublicIpcError {
            code,
            message,
            retry_after_ms: None,
        }
    }
}

// Preserve the desktop's existing error taxonomy. Only safe public outcomes are
// adapted here; underlying database/crypto/I/O errors are never serialized.
impl From<PublicIpcError> for ErrorCode {
    fn from(error: PublicIpcError) -> Self {
        match error.code {
            "unauthorized" | "not-initialized" => Self::VaultLocked,
            "vault-record-not-found" => Self::CredentialNotFound,
            _ => Self::InternalError,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Status {
    Unlocked,
    Locked,
    Error,
    AppNotRunning,
    Unsupported,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ApprovalRequestStatus {
    Requested,
}

#[derive(Serialize, Zeroize, ZeroizeOnDrop)]
#[serde(rename_all = "camelCase")]
pub(super) struct MatchSummary {
    pub credential_id: String,
    pub name: String,
    pub username: String,
}

#[derive(Serialize, Zeroize, ZeroizeOnDrop)]
pub(super) struct FillPayload {
    pub username: String,
    pub password: String,
}

impl std::fmt::Debug for FillPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FillPayload([REDACTED])")
    }
}

#[derive(Serialize)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub(super) enum ResponseData {
    Status { state: Status },
    Matches { matches: Vec<MatchSummary> },
    FillPayload(FillPayload),
    HostApprovalRequested { state: ApprovalRequestStatus },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Success<'a> {
    version: u8,
    request_id: &'a str,
    ok: bool,
    #[serde(flatten)]
    data: &'a ResponseData,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Failure<'a> {
    version: u8,
    request_id: Option<&'a str>,
    ok: bool,
    #[serde(rename = "type")]
    kind: &'static str,
    error: PublicIpcError,
}

pub(super) fn encode_response(
    request_id: &str,
    data: &ResponseData,
) -> Result<Zeroizing<Vec<u8>>, ErrorCode> {
    encode(&Success {
        version: VERSION,
        request_id,
        ok: true,
        data,
    })
}

pub(super) fn encode_error(
    request_id: Option<&str>,
    code: ErrorCode,
) -> Result<Zeroizing<Vec<u8>>, ErrorCode> {
    encode(&Failure {
        version: VERSION,
        request_id,
        ok: false,
        kind: "error",
        error: code.public(),
    })
}

fn encode(value: &impl Serialize) -> Result<Zeroizing<Vec<u8>>, ErrorCode> {
    // Preallocate the whole bound: secret-bearing serialization cannot leave old
    // allocations behind through Vec growth. Partial failures also wipe on drop.
    let mut writer = BoundedJson(Zeroizing::new(Vec::with_capacity(MAX_MESSAGE_BYTES)));
    serde_json::to_writer(&mut writer, value).map_err(|_| ErrorCode::InternalError)?;
    Ok(writer.0)
}

struct BoundedJson(Zeroizing<Vec<u8>>);

impl Write for BoundedJson {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_MESSAGE_BYTES - self.0.len() {
            return Err(io::Error::other(
                "Autofill response exceeds the message limit",
            ));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
