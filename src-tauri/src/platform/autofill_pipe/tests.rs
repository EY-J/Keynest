use std::{
    io::{Read, Write},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use serde_json::{json, Value};
use tempfile::TempDir;

use super::{
    client::{connect, forward_at, PipeError},
    io::Event,
    security::Identity,
    server::{create_pipe, PipeServer},
    RECEIPT,
};
use crate::{
    autofill::framing::{read_frame, write_frame},
    security::{
        AuthService, ClipboardError, ClipboardPort, ClipboardService, KdfParams, LockCoordinator,
        LockError, LockEventSink, OsEntropy, ProfileStore, SecurityOperationGate,
    },
    vault::{VaultRecordInput, VaultService},
    AutofillService,
};

const PAGE: &str = "https://fixture.example.test/login";
const PASSWORD: &str = "disposable IPC fixture password";
static NEXT_PIPE: AtomicUsize = AtomicUsize::new(0);

struct NoClipboard;
impl ClipboardPort for NoClipboard {
    fn write_text(&self, _: &str) -> Result<(), ClipboardError> {
        panic!("unexpected clipboard write")
    }
    fn read_text(&self) -> Result<String, ClipboardError> {
        panic!("unexpected clipboard read")
    }
    fn clear(&self) -> Result<(), ClipboardError> {
        panic!("unexpected clipboard clear")
    }
}
struct Events;
impl LockEventSink for Events {
    fn emit_locked(&self) -> Result<(), LockError> {
        Ok(())
    }
}

struct Fixture {
    _temp: TempDir,
    auth: AuthService,
    vault: VaultService,
    gate: SecurityOperationGate,
    locks: LockCoordinator,
    service: AutofillService,
    name: String,
    session: u32,
    server: Option<PipeServer>,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let auth = AuthService::load(
            ProfileStore::new(temp.path().into()),
            KdfParams::testing(),
            Arc::new(OsEntropy),
        );
        let vault = VaultService::new(temp.path().into(), Arc::new(OsEntropy));
        let gate = SecurityOperationGate::new();
        let locks = LockCoordinator::new(
            auth.clone(),
            ClipboardService::new(Arc::new(NoClipboard), Duration::from_secs(30)),
            Arc::new(Events),
            gate.clone(),
        );
        let service = AutofillService::new(auth.clone(), vault.clone(), gate.clone());
        let identity = Identity::current().unwrap();
        let name = format!(
            "{}-test-{}-{}",
            identity.pipe_name(),
            std::process::id(),
            NEXT_PIPE.fetch_add(1, Ordering::Relaxed)
        );
        Self {
            _temp: temp,
            auth,
            vault,
            gate,
            locks,
            service,
            name,
            session: identity.session,
            server: None,
        }
    }
    fn start(&mut self) {
        self.server = Some(PipeServer::start_fixture(self.service.clone(), &self.name).unwrap());
    }
    fn add(&self, password: &str) -> String {
        drop(
            self.auth
                .create_master_password("a secure master password")
                .unwrap(),
        );
        self.auth
            .require_vault_key(|key| {
                self.vault.create(
                    key,
                    VaultRecordInput {
                        name: "IPC fixture".into(),
                        username: "fixture-user".into(),
                        password: password.into(),
                        website: Some(PAGE.into()),
                        allowed_login_hosts: vec![],
                        tags: vec![],
                    },
                )
            })
            .unwrap()
            .unwrap()
            .id
    }
    fn call(&self, request: Value) -> Value {
        let response = forward_at(
            &serde_json::to_vec(&request).unwrap(),
            &self.name,
            self.session,
        )
        .unwrap();
        serde_json::from_slice(&response).unwrap()
    }
    fn stop(&self) {
        self.server.as_ref().unwrap().shutdown();
        assert!(self.server.as_ref().unwrap().wait_stopped());
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(server) = self.server.as_ref() {
            server.shutdown();
            assert!(server.wait_stopped());
        }
    }
}

fn status() -> Value {
    json!({"version":1,"requestId":"pipe-status","type":"status"})
}
fn query() -> Value {
    json!({"version":1,"requestId":"pipe-query","type":"queryMatches","pageUrl":PAGE})
}
fn fill(id: &str) -> Value {
    json!({"version":1,"requestId":"pipe-fill","type":"requestFill","pageUrl":PAGE,"credentialId":id})
}

#[test]
fn native_round_trip_reuses_vault_and_revalidates_lock_between_connections() {
    let mut f = Fixture::new();
    let id = f.add(PASSWORD);
    f.start();
    assert_eq!(f.call(status())["data"]["state"], "unlocked");
    let matches = f.call(query());
    assert_eq!(matches["data"]["matches"][0]["credentialId"], id);
    assert!(!matches.to_string().contains("password"));
    assert_eq!(f.call(fill(&id))["data"]["password"], PASSWORD);
    let approval = json!({"version":1,"requestId":"pipe-approval","type":"requestHostApproval","pageUrl":"https://accounts.example.test/login"});
    let approval_response = f.call(approval);
    assert_eq!(approval_response["type"], "hostApprovalRequested");
    assert_eq!(approval_response["data"]["state"], "requested");
    assert!(!approval_response.to_string().contains("password"));
    let pending = f.service.pending_host_approval().unwrap().unwrap();
    assert_eq!(pending.requested_host, "accounts.example.test");
    f.service
        .cancel_host_approval(&pending.approval_id)
        .unwrap();
    let mut mismatch = fill(&id);
    mismatch["pageUrl"] = "https://fixture.example.test.evil.test".into();
    assert_eq!(f.call(mismatch)["error"]["code"], "DOMAIN_MISMATCH");
    f.locks.lock_and_emit().unwrap();
    assert_eq!(f.call(status())["data"]["state"], "locked");
    for request in [query(), fill(&id)] {
        assert_eq!(f.call(request)["error"]["code"], "VAULT_LOCKED");
    }
}

#[test]
fn native_fragmented_request_reads_and_invalid_frames_leave_listener_usable() {
    let mut f = Fixture::new();
    f.start();
    let stop = Event::new().unwrap();
    {
        let pipe = connect(&f.name).unwrap();
        let mut stream = pipe.stream(&stop, Instant::now() + Duration::from_secs(2));
        let bytes = serde_json::to_vec(&status()).unwrap();
        for byte in (bytes.len() as u32)
            .to_le_bytes()
            .iter()
            .chain(bytes.iter())
        {
            stream.write_all(&[*byte]).unwrap();
        }
        assert_eq!(
            serde_json::from_slice::<Value>(&read_frame(&mut stream).unwrap()).unwrap()["data"]
                ["state"],
            "locked"
        );
        stream.write_all(&[RECEIPT]).unwrap();
    }
    for length in [0_u32, 65537, u32::MAX] {
        let pipe = connect(&f.name).unwrap();
        let mut stream = pipe.stream(&stop, Instant::now() + Duration::from_secs(2));
        stream.write_all(&length.to_le_bytes()).unwrap();
        assert!(read_frame(&mut stream).is_err());
    }
    assert_eq!(f.call(status())["ok"], true);
}

#[test]
fn native_server_rejects_invalid_json_and_unknown_operations_independently_of_client() {
    let mut f = Fixture::new();
    f.start();
    let stop = Event::new().unwrap();
    for request in [
        b"\xff".as_slice(),
        b"{",
        br#"{"version":1,"requestId":"bad","type":"unlock"}"#,
    ] {
        let pipe = connect(&f.name).unwrap();
        let mut stream = pipe.stream(&stop, Instant::now() + Duration::from_secs(2));
        write_frame(&mut stream, request).unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&read_frame(&mut stream).unwrap()).unwrap()["error"]
                ["code"],
            "INVALID_REQUEST"
        );
        stream.write_all(&[RECEIPT]).unwrap();
    }
}

#[test]
fn native_idle_client_cannot_hold_lock_and_shutdown_cancels_pending_read() {
    let mut f = Fixture::new();
    f.add(PASSWORD);
    f.start();
    let pipe = connect(&f.name).unwrap();
    let stop = Event::new().unwrap();
    let mut stream = pipe.stream(&stop, Instant::now() + Duration::from_secs(3));
    stream.write_all(&[50, 0]).unwrap(); // incomplete length, no service call
    let start = Instant::now();
    f.locks.lock_and_emit().unwrap();
    assert!(start.elapsed() < Duration::from_secs(1));
    f.stop();
    assert!(read_frame(&mut stream).is_err());
}

#[test]
fn native_read_deadline_cancels_partial_request_and_listener_recovers() {
    let mut f = Fixture::new();
    f.start();
    let pipe = connect(&f.name).unwrap();
    let stop = Event::new().unwrap();
    let mut stream = pipe.stream(&stop, Instant::now() + Duration::from_secs(4));
    stream.write_all(&[20, 0, 0, 0, b'{']).unwrap();
    let start = Instant::now();
    assert!(read_frame(&mut stream).is_err());
    assert!(start.elapsed() < Duration::from_secs(3));
    assert_eq!(f.call(status())["ok"], true);
}

#[test]
fn native_nonreading_client_cannot_keep_response_write_holding_the_lock_gate() {
    let mut f = Fixture::new();
    let id = f.add(&"\"".repeat(4096));
    f.start();
    let pipe = connect(&f.name).unwrap();
    let stop = Event::new().unwrap();
    let mut stream = pipe.stream(&stop, Instant::now() + Duration::from_secs(3));
    write_frame(&mut stream, &serde_json::to_vec(&fill(&id)).unwrap()).unwrap();
    let mut prefix = [0; 4];
    stream.read_exact(&mut prefix).unwrap();
    assert!(u32::from_le_bytes(prefix) > 4096); // body exceeds configured pipe quota
    let start = Instant::now();
    f.locks.lock_and_emit().unwrap();
    assert!(start.elapsed() < Duration::from_secs(1));
    assert_eq!(f.call(query())["error"]["code"], "VAULT_LOCKED");
}

#[test]
fn native_receipt_wait_is_outside_lock_gate_and_expires() {
    let mut f = Fixture::new();
    let id = f.add(PASSWORD);
    f.start();
    let pipe = connect(&f.name).unwrap();
    let stop = Event::new().unwrap();
    let mut stream = pipe.stream(&stop, Instant::now() + Duration::from_secs(2));
    write_frame(&mut stream, &serde_json::to_vec(&fill(&id)).unwrap()).unwrap();
    drop(read_frame(&mut stream).unwrap()); // deliberately no receipt
    let start = Instant::now();
    f.locks.lock_and_emit().unwrap();
    assert!(start.elapsed() < Duration::from_millis(200));
    assert_eq!(f.call(status())["data"]["state"], "locked");
}

#[test]
fn native_shutdown_prevents_a_queued_response_from_being_delivered() {
    let mut f = Fixture::new();
    let id = f.add(PASSWORD);
    f.start();
    let guard = f.gate.lock();
    let pipe = connect(&f.name).unwrap();
    let stop = Event::new().unwrap();
    let mut stream = pipe.stream(&stop, Instant::now() + Duration::from_secs(2));
    write_frame(&mut stream, &serde_json::to_vec(&fill(&id)).unwrap()).unwrap();
    f.server.as_ref().unwrap().shutdown();
    drop(guard);
    assert!(f.server.as_ref().unwrap().wait_stopped());
    assert!(read_frame(&mut stream).is_err());
}

#[test]
fn native_shutdown_cancels_pending_accept_and_releases_name() {
    let mut f = Fixture::new();
    f.start();
    f.stop();
    f.start();
    assert_eq!(f.call(status())["ok"], true);
}

#[test]
fn native_name_is_user_session_scoped_and_first_instance_prevents_takeover() {
    let identity = Identity::current().unwrap();
    assert!(identity
        .pipe_name()
        .starts_with(r"\\.\pipe\keynest-autofill-v1-S-"));
    assert!(identity
        .pipe_name()
        .ends_with(&format!("-{}", identity.session)));
    let mut f = Fixture::new();
    f.start();
    assert!(PipeServer::start_fixture(f.service.clone(), &f.name).is_err());
    assert_eq!(f.call(status())["ok"], true);
}

#[test]
fn native_client_rejects_wrong_server_session_and_invalid_requests() {
    let mut f = Fixture::new();
    f.start();
    assert_eq!(
        forward_at(
            &serde_json::to_vec(&status()).unwrap(),
            &f.name,
            f.session + 1
        )
        .unwrap_err(),
        PipeError::IpcUnavailable
    );
    assert_eq!(
        forward_at(b"{}", &f.name, f.session).unwrap_err(),
        PipeError::InvalidRequest
    );
    f.stop();
    assert_eq!(
        forward_at(&serde_json::to_vec(&status()).unwrap(), &f.name, f.session).unwrap_err(),
        PipeError::AppNotRunning
    );
}

#[test]
fn native_pipe_dacl_contains_only_current_user_and_no_instance_creation_right() {
    use super::security::{LocalAllocation, CLIENT_ACL_RIGHTS};
    use windows_sys::Win32::Security::Authorization::ConvertSidToStringSidW;
    use windows_sys::Win32::Security::{
        GetAce, GetKernelObjectSecurity, GetSecurityDescriptorDacl, ACCESS_ALLOWED_ACE,
        DACL_SECURITY_INFORMATION,
    };
    let f = Fixture::new();
    let identity = Identity::current().unwrap();
    let pipe = create_pipe(&f.name, &identity).unwrap();
    // Inspect the actual kernel object's installed ACL, not just the SDDL source.
    unsafe {
        let mut length = 0;
        GetKernelObjectSecurity(
            pipe.raw(),
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            0,
            &mut length,
        );
        let mut bytes = vec![0_usize; (length as usize).div_ceil(std::mem::size_of::<usize>())];
        assert_ne!(
            GetKernelObjectSecurity(
                pipe.raw(),
                DACL_SECURITY_INFORMATION,
                bytes.as_mut_ptr().cast(),
                length,
                &mut length
            ),
            0
        );
        let mut present = 0;
        let mut defaulted = 0;
        let mut acl = std::ptr::null_mut();
        assert_ne!(
            GetSecurityDescriptorDacl(
                bytes.as_mut_ptr().cast(),
                &mut present,
                &mut acl,
                &mut defaulted
            ),
            0
        );
        assert_eq!(present, 1);
        assert!(!acl.is_null());
        assert_eq!((*acl).AceCount, 1);
        let mut ace = std::ptr::null_mut();
        assert_ne!(GetAce(acl, 0, &mut ace), 0);
        let ace = &*ace.cast::<ACCESS_ALLOWED_ACE>();
        assert_eq!(ace.Header.AceType, 0);
        assert_eq!(ace.Mask, CLIENT_ACL_RIGHTS);
        assert_eq!(ace.Mask & 4, 0);
        let mut text = std::ptr::null_mut();
        assert_ne!(
            ConvertSidToStringSidW(
                std::ptr::addr_of!(ace.SidStart).cast_mut().cast(),
                &mut text
            ),
            0
        );
        let _allocation = LocalAllocation(text.cast());
        let mut count = 0;
        while *text.add(count) != 0 {
            count += 1;
        }
        assert_eq!(
            String::from_utf16(std::slice::from_raw_parts(text, count)).unwrap(),
            identity.sid
        );
    }
}

#[test]
fn native_cancel_is_terminal_instead_of_read_exact_retryable_interruption() {
    let f = Fixture::new();
    let pipe = create_pipe(&f.name, &Identity::current().unwrap()).unwrap();
    let stop = Event::new().unwrap();
    stop.set();
    let mut stream = pipe.stream(&stop, Instant::now() + Duration::from_secs(1));
    assert_eq!(
        stream.read(&mut [0]).unwrap_err().kind(),
        std::io::ErrorKind::ConnectionAborted
    );
    assert_eq!(
        stream.write(b"fixture").unwrap_err().kind(),
        std::io::ErrorKind::ConnectionAborted
    );
    assert!(read_frame(&mut stream).is_err());
}

#[test]
fn native_clients_disconnecting_before_accept_do_not_stop_listener() {
    let mut f = Fixture::new();
    f.start();
    for _ in 0..20 {
        drop(connect(&f.name).unwrap());
    }
    assert_eq!(f.call(status())["ok"], true);
}

#[test]
fn native_busy_pipe_connect_has_a_deadline() {
    let mut f = Fixture::new();
    f.start();
    let _idle_client = connect(&f.name).unwrap();
    let start = Instant::now();
    assert!(matches!(connect(&f.name), Err(PipeError::IpcUnavailable)));
    assert!(start.elapsed() < Duration::from_millis(1500));
}

#[test]
fn native_server_refuses_a_client_from_an_unexpected_session() {
    let mut f = Fixture::new();
    let mut identity = Identity::current().unwrap();
    identity.session += 1;
    f.server = Some(PipeServer::start_at(f.service.clone(), &f.name, &identity).unwrap());
    assert_eq!(
        forward_at(&serde_json::to_vec(&status()).unwrap(), &f.name, f.session).unwrap_err(),
        PipeError::IpcUnavailable
    );
}

/// Start explicitly with --ignored --nocapture. Only synthetic vault data and a
/// unique test pipe; stdin controls are absent from production builds/protocol.
#[test]
#[ignore = "interactive two-process local IPC smoke fixture"]
fn manual_fixture_server() {
    let mut f = Fixture::new();
    f.add(PASSWORD);
    f.start();
    println!("SMOKE_PIPE={}", f.name.strip_prefix(r"\\.\pipe\").unwrap());
    println!("Fixture unlocked. Enter lock, unlock, or quit on stdin.");
    for line in std::io::stdin().lines() {
        match line.unwrap().trim() {
            "lock" => {
                f.locks.lock_and_emit().unwrap();
                println!("Fixture locked.");
            }
            "unlock" => {
                f.auth.unlock("a secure master password").unwrap();
                println!("Fixture unlocked.");
            }
            "quit" => break,
            _ => println!("Use lock, unlock, or quit."),
        }
    }
    f.stop();
}
