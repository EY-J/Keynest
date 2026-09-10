use std::io::{self, Read, Write};
use zeroize::Zeroizing;

use super::{
    protocol::{
        encode_error, encode_response, ErrorCode, Operation, Request, ResponseData, Status,
        MAX_MESSAGE_BYTES,
    },
    response,
};

/// Native Messaging executable entry point. The browser enforces exact manifest
/// allowed_origins; the supplied origin argument is format-checked, not treated
/// as authentication against a process already running as the same user.
pub fn run_native_host() {
    crate::diagnostics::install_panic_hook();
    if !valid_arguments(std::env::args_os().skip(1)) {
        return;
    }
    let _ = serve(&mut io::stdin().lock(), &mut io::stdout().lock(), forward);
}

fn valid_arguments(mut args: impl Iterator<Item = std::ffi::OsString>) -> bool {
    let Some(origin) = args.next().and_then(|arg| arg.into_string().ok()) else {
        return false;
    };
    let Some(id) = origin
        .strip_prefix("chrome-extension://")
        .map(|id| id.strip_suffix('/').unwrap_or(id))
    else {
        return false;
    };
    if id.len() != 32 || !id.bytes().all(|b| (b'a'..=b'p').contains(&b)) {
        return false;
    }
    match args.next() {
        None => true,
        Some(parent) => {
            parent
                .to_str()
                .and_then(|parent| parent.strip_prefix("--parent-window="))
                .is_some_and(|handle| {
                    !handle.is_empty() && handle.bytes().all(|b| b.is_ascii_digit())
                })
                && args.next().is_none()
        }
    }
}

fn forward(bytes: &[u8]) -> Result<Zeroizing<Vec<u8>>, ErrorCode> {
    #[cfg(windows)]
    {
        crate::forward_autofill_request(bytes).map_err(|error| match error {
            crate::PipeError::AppNotRunning => ErrorCode::AppNotRunning,
            crate::PipeError::IpcUnavailable => ErrorCode::IpcUnavailable,
            crate::PipeError::InvalidRequest => ErrorCode::InvalidRequest,
        })
    }
    #[cfg(not(windows))]
    {
        let _ = bytes;
        Err(ErrorCode::IpcUnavailable)
    }
}

fn serve(
    reader: &mut impl Read,
    writer: &mut impl Write,
    mut forward: impl FnMut(&[u8]) -> Result<Zeroizing<Vec<u8>>, ErrorCode>,
) -> io::Result<()> {
    loop {
        let bytes = match read_message(reader) {
            Ok(Some(bytes)) => bytes,
            Ok(None) => return Ok(()),
            Err(_) => {
                send(writer, &error_bytes(None, ErrorCode::InvalidRequest)?)?;
                return Ok(()); // do not try to resynchronize malformed framing
            }
        };
        let request = match Request::parse(&bytes) {
            Ok(request) => request,
            Err(invalid) => {
                send(
                    writer,
                    &error_bytes(invalid.request_id.as_deref(), ErrorCode::InvalidRequest)?,
                )?;
                continue;
            }
        };
        let result = if cfg!(windows) || cfg!(test) {
            match forward(&bytes) {
                Ok(response) => response::validate(&response, &request),
                Err(ErrorCode::AppNotRunning) if matches!(request.operation, Operation::Status) => {
                    Ok(ResponseData::Status {
                        state: Status::AppNotRunning,
                    })
                }
                Err(error) => Err(error),
            }
        } else if matches!(request.operation, Operation::Status) {
            Ok(ResponseData::Status {
                state: Status::Unsupported,
            })
        } else {
            Err(ErrorCode::IpcUnavailable)
        };
        // No cache and no generic JSON tree. Each reply is reconstructed using
        // bounded zeroizing serialization, with fixed public error text only.
        let encoded = match result {
            Ok(data) => encode_response(&request.request_id, &data),
            Err(error) => encode_error(Some(&request.request_id), error),
        };
        let encoded = encoded
            .or_else(|_| encode_error(Some(&request.request_id), ErrorCode::InternalError))
            .map_err(|_| io::Error::other("Autofill response unavailable"))?;
        send(writer, &encoded)?;
    }
}

fn error_bytes(id: Option<&str>, error: ErrorCode) -> io::Result<Zeroizing<Vec<u8>>> {
    encode_error(id, error).map_err(|_| io::Error::other("Autofill response unavailable"))
}

fn read_message(reader: &mut impl Read) -> io::Result<Option<Zeroizing<Vec<u8>>>> {
    let mut prefix = [0; 4];
    loop {
        match reader.read(&mut prefix[..1]) {
            Ok(0) => return Ok(None),
            Ok(_) => break,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    reader.read_exact(&mut prefix[1..])?;
    let length = u32::from_ne_bytes(prefix) as usize;
    if length == 0 || length > MAX_MESSAGE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid native message length",
        ));
    }
    let mut bytes = Zeroizing::new(vec![0; length]);
    reader.read_exact(&mut bytes)?;
    Ok(Some(bytes))
}

fn send(writer: &mut impl Write, bytes: &[u8]) -> io::Result<()> {
    if bytes.is_empty() || bytes.len() > MAX_MESSAGE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Invalid native message length",
        ));
    }
    writer.write_all(&(bytes.len() as u32).to_ne_bytes())?;
    writer.write_all(bytes)?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    fn request(kind: &str) -> Value {
        let mut request = json!({"version":1,"requestId":"host-1","type":kind});
        if kind != "status" {
            request["pageUrl"] = "https://fixture.test".into();
        }
        if kind == "requestFill" {
            request["credentialId"] = "a".repeat(32).into();
        }
        request
    }
    fn framed(bytes: &[u8]) -> Vec<u8> {
        let mut wire = Vec::new();
        send(&mut wire, bytes).unwrap();
        wire
    }
    fn run(request: Value, response: Value) -> Value {
        let input = framed(&serde_json::to_vec(&request).unwrap());
        let mut output = Vec::new();
        serve(&mut input.as_slice(), &mut output, |_| {
            Ok(Zeroizing::new(serde_json::to_vec(&response).unwrap()))
        })
        .unwrap();
        let mut reader = output.as_slice();
        let body = read_message(&mut reader).unwrap().unwrap();
        assert!(
            reader.is_empty(),
            "stdout contains bytes outside the response frame"
        );
        serde_json::from_slice(&body).unwrap()
    }

    #[test]
    fn framing_handles_fragmented_reads_multiple_messages_and_clean_eof() {
        struct Fragments<'a>(&'a [u8]);
        impl Read for Fragments<'_> {
            fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
                let n = bytes.len().min(1);
                self.0.read(&mut bytes[..n])
            }
        }
        let mut wire = framed(&serde_json::to_vec(&request("status")).unwrap());
        wire.extend_from_slice(&wire.clone());
        let mut output = Vec::new();
        let mut count = 0;
        serve(&mut Fragments(&wire), &mut output, |_| {
            count += 1;
            Err(ErrorCode::AppNotRunning)
        })
        .unwrap();
        assert_eq!(count, 2);
        let mut reader = output.as_slice();
        for _ in 0..2 {
            assert_eq!(
                serde_json::from_slice::<Value>(&read_message(&mut reader).unwrap().unwrap())
                    .unwrap()["data"]["state"],
                "app_not_running"
            );
        }
        assert!(read_message(&mut reader).unwrap().is_none());
        let mut empty = Vec::new();
        serve(&mut [].as_slice(), &mut empty, |_| panic!()).unwrap();
        assert!(empty.is_empty());
    }

    #[test]
    fn malformed_lengths_truncation_utf8_json_versions_and_types_never_forward() {
        for input in [
            vec![1],
            vec![1, 0, 0],
            framed(b"\xff"),
            framed(b"{"),
            framed(br#"{"version":2,"requestId":"host-1","type":"status"}"#),
            framed(br#"{"version":1,"requestId":"host-1","type":"unlock"}"#),
            0_u32.to_ne_bytes().to_vec(),
            65537_u32.to_ne_bytes().to_vec(),
            u32::MAX.to_ne_bytes().to_vec(),
            vec![2, 0, 0, 0, b'{'],
        ] {
            let mut output = Vec::new();
            serve(&mut input.as_slice(), &mut output, |_| {
                panic!("invalid message forwarded")
            })
            .unwrap();
            let body = read_message(&mut output.as_slice()).unwrap().unwrap();
            assert_eq!(
                serde_json::from_slice::<Value>(&body).unwrap()["error"]["code"],
                "INVALID_REQUEST"
            );
        }
    }

    #[test]
    fn only_matching_response_types_can_cross_the_host_boundary() {
        let fill = json!({"version":1,"requestId":"host-1","ok":true,"type":"fillPayload","data":{"username":"fixture-user","password":"fixture-secret\n雪"}});
        assert_eq!(run(request("requestFill"), fill.clone()), fill);
        for kind in ["status", "queryMatches"] {
            assert_eq!(
                run(request(kind), fill.clone())["error"]["code"],
                "IPC_UNAVAILABLE"
            );
        }
        let matches = json!({"version":1,"requestId":"host-1","ok":true,"type":"matches","data":{"matches":[{"credentialId":"a".repeat(32),"name":"fixture","username":"user"}]}});
        assert_eq!(run(request("queryMatches"), matches.clone()), matches);
        let approval = json!({"version":1,"requestId":"host-1","ok":true,"type":"hostApprovalRequested","data":{"state":"requested"}});
        assert_eq!(
            run(request("requestHostApproval"), approval.clone()),
            approval
        );
        assert_eq!(
            run(
                request("status"),
                json!({"version":1,"requestId":"host-1","ok":true,"type":"hostApprovalRequested","data":{"state":"requested"}})
            )["error"]["code"],
            "IPC_UNAVAILABLE"
        );
    }

    #[test]
    fn malformed_or_secret_smuggling_responses_are_not_forwarded() {
        let valid = json!({"version":1,"requestId":"host-1","ok":true,"type":"status","data":{"state":"locked"}});
        for bad in [
            json!([]),
            json!({"version":1}),
            {
                let mut v = valid.clone();
                v["requestId"] = "wrong-id".into();
                v
            },
            {
                let mut v = valid.clone();
                v["version"] = 2.into();
                v
            },
            {
                let mut v = valid.clone();
                v["data"]["password"] = "secret".into();
                v
            },
            {
                let mut v = valid.clone();
                v["data"] = json!(["locked"]);
                v
            },
        ] {
            assert_eq!(
                run(request("status"), bad)["error"]["code"],
                "IPC_UNAVAILABLE"
            );
        }
        let summary = json!({"version":1,"requestId":"host-1","ok":true,"type":"matches","data":{"matches":[{"credentialId":"a".repeat(32),"name":"fixture","username":"user","password":"secret"}]}});
        assert_eq!(
            run(request("queryMatches"), summary)["error"]["code"],
            "IPC_UNAVAILABLE"
        );
        let approval_with_secret = json!({"version":1,"requestId":"host-1","ok":true,"type":"hostApprovalRequested","data":{"state":"requested","password":"secret"}});
        assert_eq!(
            run(request("requestHostApproval"), approval_with_secret)["error"]["code"],
            "IPC_UNAVAILABLE"
        );
    }

    #[test]
    fn backend_error_text_is_replaced_with_fixed_public_text() {
        let result = run(
            request("queryMatches"),
            json!({"version":1,"requestId":"host-1","ok":false,"type":"error","error":{"code":"VAULT_LOCKED","message":"sentinel C:/private/vault.enc password"}}),
        );
        assert_eq!(result["error"]["code"], "VAULT_LOCKED");
        assert!(!result.to_string().contains("sentinel"));
        assert_eq!(result["requestId"], "host-1");
    }

    #[test]
    fn stdout_has_exact_byte_lengths_and_no_text_translation_or_diagnostics() {
        let bytes = b"\n\r\n\x1a\0fixture";
        let wire = framed(bytes);
        assert_eq!(&wire[..4], &(bytes.len() as u32).to_ne_bytes());
        assert_eq!(&wire[4..], bytes);
        let mut output = Vec::new();
        assert!(send(&mut output, &vec![0; 65537]).is_err());
        assert!(output.is_empty());
    }

    #[test]
    fn arguments_accept_only_extension_origin_and_optional_parent_window() {
        let origin = format!("chrome-extension://{}/", "a".repeat(32));
        for args in [
            vec![origin.clone()],
            vec![origin.clone(), "--parent-window=0".into()],
        ] {
            assert!(valid_arguments(args.into_iter().map(Into::into)));
        }
        for args in [
            vec![],
            vec!["https://evil.test".into()],
            vec![origin.clone(), "--vault-path=private".into()],
            vec![origin, "--parent-window=x".into()],
        ] {
            assert!(!valid_arguments(args.into_iter().map(Into::into)));
        }
    }
}
