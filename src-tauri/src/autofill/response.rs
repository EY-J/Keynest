//! Validate and reconstruct pipe responses, never forwarding arbitrary JSON or
//! backend-provided error text to the browser. No generic secret-bearing Value.
use serde::Deserialize;
use serde_json::value::RawValue;
use zeroize::{Zeroize, ZeroizeOnDrop};

use super::protocol::{
    ApprovalRequestStatus, ErrorCode, FillPayload, MatchSummary, Operation, Request, ResponseData,
    Status, MAX_MESSAGE_BYTES,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Envelope<'a> {
    version: u8,
    request_id: String,
    ok: bool,
    #[serde(rename = "type")]
    kind: String,
    #[serde(borrow)]
    data: Option<&'a RawValue>,
    #[serde(borrow)]
    error: Option<&'a RawValue>,
}

#[derive(Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
struct Fill {
    username: String,
    password: String,
}
#[derive(Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Summary {
    credential_id: String,
    name: String,
    username: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Matches<'a> {
    #[serde(borrow)]
    matches: Vec<&'a RawValue>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    state: String,
}
#[derive(Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
struct Failure {
    code: String,
    message: String,
}

fn object<'a, T: Deserialize<'a>>(json: &'a str) -> Result<T, ErrorCode> {
    if !json.trim_start().starts_with('{') {
        return Err(ErrorCode::IpcUnavailable);
    }
    serde_json::from_str(json).map_err(|_| ErrorCode::IpcUnavailable)
}

pub(super) fn validate(bytes: &[u8], request: &Request) -> Result<ResponseData, ErrorCode> {
    if bytes.len() > MAX_MESSAGE_BYTES {
        return Err(ErrorCode::IpcUnavailable);
    }
    let json = std::str::from_utf8(bytes).map_err(|_| ErrorCode::IpcUnavailable)?;
    let response: Envelope<'_> = object(json)?;
    if response.version != 1 || response.request_id != request.request_id {
        return Err(ErrorCode::IpcUnavailable);
    }
    if !response.ok {
        if response.kind != "error" || response.data.is_some() {
            return Err(ErrorCode::IpcUnavailable);
        }
        let error: Failure = object(response.error.ok_or(ErrorCode::IpcUnavailable)?.get())?;
        return Err(match error.code.as_str() {
            "VAULT_LOCKED" => ErrorCode::VaultLocked,
            "UNSUPPORTED_URL" => ErrorCode::UnsupportedUrl,
            "NO_MATCHES" => ErrorCode::NoMatches,
            "CREDENTIAL_NOT_FOUND" => ErrorCode::CredentialNotFound,
            "DOMAIN_MISMATCH" => ErrorCode::DomainMismatch,
            "INVALID_REQUEST" => ErrorCode::InvalidRequest,
            "INTERNAL_ERROR" => ErrorCode::InternalError,
            _ => ErrorCode::IpcUnavailable,
        });
    }
    if response.error.is_some() {
        return Err(ErrorCode::IpcUnavailable);
    }
    let data = response.data.ok_or(ErrorCode::IpcUnavailable)?.get();
    match (&request.operation, response.kind.as_str()) {
        (Operation::Status, "status") => {
            let state: State = object(data)?;
            Ok(ResponseData::Status {
                state: match state.state.as_str() {
                    "unlocked" => Status::Unlocked,
                    "locked" => Status::Locked,
                    "error" => Status::Error,
                    _ => return Err(ErrorCode::IpcUnavailable),
                },
            })
        }
        (Operation::QueryMatches { .. }, "matches") => {
            let raw: Matches<'_> = object(data)?;
            let mut matches = Vec::with_capacity(raw.matches.len());
            for item in raw.matches {
                let mut item: Summary = object(item.get())?;
                if item.credential_id.len() != 32
                    || !item
                        .credential_id
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                    || item.name.is_empty()
                    || item.name.chars().count() > 200
                    || item.username.is_empty()
                    || item.username.chars().count() > 500
                {
                    return Err(ErrorCode::IpcUnavailable);
                }
                matches.push(MatchSummary {
                    credential_id: std::mem::take(&mut item.credential_id),
                    name: std::mem::take(&mut item.name),
                    username: std::mem::take(&mut item.username),
                });
            }
            Ok(ResponseData::Matches { matches })
        }
        (Operation::RequestFill { .. }, "fillPayload") => {
            let mut fill: Fill = object(data)?;
            if fill.username.is_empty()
                || fill.username.chars().count() > 500
                || fill.password.is_empty()
                || fill.password.chars().count() > 4096
            {
                return Err(ErrorCode::IpcUnavailable);
            }
            Ok(ResponseData::FillPayload(FillPayload {
                username: std::mem::take(&mut fill.username),
                password: std::mem::take(&mut fill.password),
            }))
        }
        (Operation::RequestHostApproval { .. }, "hostApprovalRequested") => {
            let state: State = object(data)?;
            if state.state != "requested" {
                return Err(ErrorCode::IpcUnavailable);
            }
            Ok(ResponseData::HostApprovalRequested {
                state: ApprovalRequestStatus::Requested,
            })
        }
        _ => Err(ErrorCode::IpcUnavailable),
    }
}
