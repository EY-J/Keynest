use std::{
    io,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc, Arc,
    },
    thread,
    time::{Duration, Instant},
};

use serde_json::{json, Value};
use tempfile::{tempdir, TempDir};
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::{
    ipc::PublicIpcError,
    security::{
        AuthService, AuthStatus, AutoLockService, ClipboardError, ClipboardPort, ClipboardService,
        KdfParams, LockCoordinator, LockError, LockEventSink, OsEntropy, ProfileStore,
        SecurityOperationGate,
    },
    vault::{VaultError, VaultRecordInput, VaultService},
};

use super::{
    protocol::{
        encode_response, ErrorCode, FillPayload, MatchSummary, Request, ResponseData,
        MAX_MESSAGE_BYTES,
    },
    url_match::HttpsHost,
    AutofillService,
};

const MASTER: &str = "a secure master password";
const PASSWORD: &str = "fixture-only-password<&>\"\\\n";
const PAGE: &str = "https://www.example.test/login";

/// Explicit disposable native-browser fixture, never part of production APIs.
/// Uses production KDF/storage and refuses to replace any existing directory.
#[cfg(windows)]
#[test]
#[ignore = "creates isolated encrypted data for the native desktop acceptance build"]
fn seed_native_browser_acceptance_vault() {
    let root = std::path::PathBuf::from(std::env::var_os("APPDATA").unwrap())
        .join("com.eyy.keynest.autofill-acceptance");
    assert!(!root.exists(), "Acceptance data directory already exists");
    std::fs::create_dir(&root).unwrap();
    let auth = AuthService::load(
        ProfileStore::new(root.clone()),
        KdfParams::production(),
        Arc::new(OsEntropy),
    );
    drop(
        auth.create_master_password("copper-planet-72-MINT!autofill")
            .unwrap(),
    );
    let vault = VaultService::new(root, Arc::new(OsEntropy));
    for suffix in ["A", "B"] {
        auth.require_vault_key(|key| {
            vault.create(
                key,
                VaultRecordInput {
                    name: format!("Autofill fixture {suffix}"),
                    username: format!("fixture-{suffix}@example.test"),
                    password: format!("KeyNest-Autofill-Fixture-Password-Only-{suffix}"),
                    website: Some("https://login.keynest-autofill.test/".into()),
                    allowed_login_hosts: vec![],
                    tags: vec![],
                },
            )
        })
        .unwrap()
        .unwrap();
    }
    println!("Disposable encrypted acceptance vault prepared; no secrets printed.");
}

struct UnusedClipboard;
impl ClipboardPort for UnusedClipboard {
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

struct GateCheckingApprovalEvents {
    gate: SecurityOperationGate,
    response_delivered: Arc<AtomicBool>,
    notifications: Arc<AtomicUsize>,
}

impl super::service::HostApprovalEventSink for GateCheckingApprovalEvents {
    fn requested(&self) -> Result<(), ()> {
        if !self.response_delivered.load(Ordering::SeqCst) {
            return Err(());
        }
        let gate = self.gate.clone();
        let (sent, received) = mpsc::channel();
        thread::spawn(move || {
            let _guard = gate.lock();
            let _ = sent.send(());
        });
        received
            .recv_timeout(Duration::from_secs(1))
            .map_err(|_| ())?;
        self.notifications.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

struct Fixture {
    temp: TempDir,
    auth: AuthService,
    vault: VaultService,
    gate: SecurityOperationGate,
    locks: LockCoordinator,
    service: AutofillService,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempdir().unwrap();
        let auth = AuthService::load(
            ProfileStore::new(temp.path().into()),
            KdfParams::testing(),
            Arc::new(OsEntropy),
        );
        let vault = VaultService::new(temp.path().into(), Arc::new(OsEntropy));
        let gate = SecurityOperationGate::new();
        let locks = LockCoordinator::new(
            auth.clone(),
            ClipboardService::new(Arc::new(UnusedClipboard), Duration::from_secs(30)),
            Arc::new(Events),
            gate.clone(),
        );
        let service = AutofillService::new(auth.clone(), vault.clone(), gate.clone());
        Self {
            temp,
            auth,
            vault,
            gate,
            locks,
            service,
        }
    }

    fn unlocked() -> Self {
        let f = Self::new();
        drop(f.auth.create_master_password(MASTER).unwrap());
        f
    }

    fn add(&self, website: Option<&str>) -> String {
        self.add_with_hosts(website, &[])
    }

    fn add_with_hosts(&self, website: Option<&str>, allowed_login_hosts: &[&str]) -> String {
        let _guard = self.gate.lock();
        let mut record = input(website);
        record.allowed_login_hosts = allowed_login_hosts
            .iter()
            .map(|host| (*host).into())
            .collect();
        self.auth
            .require_vault_key(|key| self.vault.create(key, record))
            .unwrap()
            .unwrap()
            .id
    }

    fn call(&self, request: Value) -> Value {
        call(&self.service, request)
    }

    fn fill(&self, id: &str, page: &str) -> Value {
        self.call(json!({"version":1,"requestId":"fill-1","type":"requestFill","pageUrl":page,"credentialId":id}))
    }
}

fn input(website: Option<&str>) -> VaultRecordInput {
    VaultRecordInput {
        name: "Fixture account".into(),
        username: "fixture@example.test".into(),
        password: PASSWORD.into(),
        website: website.map(str::to_owned),
        allowed_login_hosts: vec![],
        tags: vec!["private-tag".into()],
    }
}

fn query(page: &str) -> Value {
    json!({"version":1,"requestId":"query_1","type":"queryMatches","pageUrl":page})
}
fn status() -> Value {
    json!({"version":1,"requestId":"status-1","type":"status"})
}
fn request_approval(page: &str, request_id: &str) -> Value {
    json!({"version":1,"requestId":request_id,"type":"requestHostApproval","pageUrl":page})
}
fn call(service: &AutofillService, request: Value) -> Value {
    let mut response = None;
    service
        .dispatch(&serde_json::to_vec(&request).unwrap(), |bytes| {
            assert!(bytes.len() <= MAX_MESSAGE_BYTES);
            response = Some(serde_json::from_slice(bytes).unwrap());
            Ok(())
        })
        .unwrap();
    response.unwrap()
}
fn error(value: &Value, code: &str) {
    assert_eq!(value["version"], 1);
    assert_eq!(value["ok"], false);
    assert_eq!(value["type"], "error");
    assert_eq!(value["error"]["code"], code);
    assert!(value.get("data").is_none());
    assert!(!value.to_string().contains(PASSWORD));
}

#[test]
fn exact_https_hosts_normalize_without_path_query_fragment_or_port_matching() {
    for (saved, page) in [
        ("https://facebook.com/", "https://facebook.com/"),
        ("https://facebook.com/", "https://www.facebook.com/login"),
        ("https://www.facebook.com/", "https://facebook.com/login"),
        ("https://www.facebook.com/", "https://www.facebook.com/"),
        (
            "https://www.facebook.com/",
            "https://www.facebook.com/login",
        ),
        (
            "https://facebook.com/",
            "https://facebook.com/login?next=other.test#fragment",
        ),
        ("HTTPS://WWW.FACEBOOK.COM/", "https://FACEBOOK.COM/"),
        ("https://www.facebook.com./", "https://facebook.com/"),
        ("https://facebook.com/", "https://www.facebook.com./"),
        (
            "https://login.facebook.com/",
            "https://www.login.facebook.com/",
        ),
        ("https://example.test:8443/", "https://example.test:443/"),
        ("https://bücher.example/", "https://xn--bcher-kva.example/"),
        ("https://[::1]/", "https://[::1]/login"),
    ] {
        assert!(
            HttpsHost::parse(page)
                .unwrap_or_else(|_| panic!("fixture URL rejected"))
                .matches_saved(Some(saved), &[]),
            "{saved} / {page}"
        );
    }
}

#[test]
fn www_equivalence_is_applied_by_query_and_independent_fill_revalidation() {
    for (saved, page) in [
        ("https://facebook.com/", "https://www.facebook.com/login"),
        ("https://www.facebook.com/", "https://facebook.com/login"),
    ] {
        let f = Fixture::unlocked();
        let id = f.add(Some(saved));
        assert_eq!(f.fill(&id, page)["data"]["password"], PASSWORD);
        let matches = f.call(query(page));
        assert_eq!(matches["data"]["matches"][0]["credentialId"], id);
    }
}

#[test]
fn suffix_lookalike_and_unlisted_subdomains_never_match() {
    for page in [
        "https://login.facebook.com/",
        "https://www.login.facebook.com/",
        "https://facebook.com.evil.example/",
        "https://fake-facebook.com/",
        "https://www.www.facebook.com/",
        "https://fаcebook.com/",
        "https://evil.example/?facebook.com",
        "https://evil.example/facebook.com",
    ] {
        assert!(
            !HttpsHost::parse(page)
                .unwrap_or_else(|_| panic!("fixture URL rejected"))
                .matches_saved(Some("https://facebook.com/"), &[]),
            "{page}"
        );
    }
    assert!(!HttpsHost::parse("https://accounts.google.com/")
        .unwrap_or_else(|_| panic!())
        .matches_saved(Some("https://gmail.com/"), &[]));
}

#[test]
fn invalid_or_unsupported_urls_fail_closed_on_both_sides() {
    let host = HttpsHost::parse(PAGE).unwrap_or_else(|_| panic!());
    assert!(!host.matches_saved(None, &[]));
    for bad in [
        "",
        "www.example.test",
        "/login",
        "http://www.example.test/",
        "ftp://www.example.test/",
        "file:///tmp/test",
        "javascript:alert(1)",
        "data:text/html,test",
        "https://",
        "https:example.test",
        "https:///www.example.test",
        " https://www.example.test",
        "https://www.example.test/\n",
        "https://www.example.test\\@evil.example/",
        "https://user:password@www.example.test/",
        "https://www.example.test@evil.example/",
        "https://www.example.test../",
        "https://www..example.test/",
        "https://[invalid]/",
        "https://www.example.test:99999/",
    ] {
        assert!(HttpsHost::parse(bad).is_err(), "{bad}");
        assert!(!host.matches_saved(Some(bad), &[]), "{bad}");
    }
}

#[test]
fn explicit_alternate_hosts_match_exactly_for_query_and_fill() {
    let f = Fixture::unlocked();
    let id = f.add_with_hosts(Some("https://gmail.com/"), &["ACCOUNTS.GOOGLE.COM."]);
    let page = "https://accounts.google.com/signin";
    let matches = f.call(query(page));
    assert_eq!(matches["data"]["matches"][0]["credentialId"], id);
    assert_eq!(f.fill(&id, page)["data"]["password"], PASSWORD);
    for rejected in [
        "https://mail.google.com/",
        "https://accounts.google.com.evil.test/",
        "https://login.accounts.google.com/",
    ] {
        error(&f.call(query(rejected)), "NO_MATCHES");
        error(&f.fill(&id, rejected), "DOMAIN_MISMATCH");
    }
}

#[test]
fn host_approval_request_is_url_only_canonical_and_preserves_request_id() {
    let f = Fixture::new();
    let response = f.call(request_approval(
        "https://WWW.ACCOUNTS.GOOGLE.COM./signin?continue=x",
        "approval-request_1",
    ));
    assert_eq!(
        response,
        json!({"version":1,"requestId":"approval-request_1","ok":true,"type":"hostApprovalRequested","data":{"state":"requested"}})
    );
    assert!(!response
        .to_string()
        .to_ascii_lowercase()
        .contains("password"));
    assert_eq!(
        f.service
            .pending_host_approval()
            .unwrap()
            .unwrap()
            .requested_host,
        "accounts.google.com"
    );
}

#[test]
fn host_approval_notifies_only_after_response_delivery_and_gate_release() {
    let f = Fixture::new();
    let response_delivered = Arc::new(AtomicBool::new(false));
    let notifications = Arc::new(AtomicUsize::new(0));
    let service = AutofillService::new(f.auth.clone(), f.vault.clone(), f.gate.clone())
        .with_host_approval_events(Arc::new(GateCheckingApprovalEvents {
            gate: f.gate.clone(),
            response_delivered: response_delivered.clone(),
            notifications: notifications.clone(),
        }));
    let request = serde_json::to_vec(&request_approval(
        "https://login.live.com/signin",
        "notify-after-delivery",
    ))
    .unwrap();
    let mut response = Vec::new();

    service
        .dispatch(&request, |bytes| {
            response.extend_from_slice(bytes);
            response_delivered.store(true, Ordering::SeqCst);
            Ok(())
        })
        .unwrap();

    assert_eq!(notifications.load(Ordering::SeqCst), 1);
    assert_eq!(
        serde_json::from_slice::<Value>(&response).unwrap(),
        json!({"version":1,"requestId":"notify-after-delivery","ok":true,"type":"hostApprovalRequested","data":{"state":"requested"}})
    );
    assert_eq!(
        service
            .pending_host_approval()
            .unwrap()
            .unwrap()
            .requested_host,
        "login.live.com"
    );
}

#[test]
fn malformed_approval_urls_fail_safely_without_creating_pending_state() {
    for page in [
        "accounts.google.com",
        "http://accounts.google.com/",
        "*.google.com",
        "accounts.google.com/login",
        "https://google/",
        "https://*.google.com/",
        "https://accounts.google.com\\evil",
        "https://user:secret@accounts.google.com/",
    ] {
        let f = Fixture::new();
        error(
            &f.call(request_approval(page, "bad-approval")),
            "UNSUPPORTED_URL",
        );
        assert!(f.service.pending_host_approval().unwrap().is_none());
    }
}

#[test]
fn replacing_a_request_invalidates_the_previous_desktop_confirmation_token() {
    let f = Fixture::unlocked();
    let id = f.add(Some("https://gmail.com/"));
    f.call(request_approval("https://first.example.test/", "reused-id"));
    let first = f.service.pending_host_approval().unwrap().unwrap();
    f.call(request_approval(
        "https://second.example.test/",
        "reused-id",
    ));
    let second = f.service.pending_host_approval().unwrap().unwrap();
    assert_ne!(first.approval_id, second.approval_id);
    assert!(matches!(
        f.service.approve_login_host(&first.approval_id, &id),
        Err(super::service::HostApprovalError::Unavailable)
    ));
    let record = f
        .auth
        .require_vault_key(|key| f.vault.get(key, &id))
        .unwrap()
        .unwrap();
    assert!(record.allowed_login_hosts.is_empty());
    assert_eq!(
        f.service
            .pending_host_approval()
            .unwrap()
            .unwrap()
            .requested_host,
        "second.example.test"
    );
}

#[test]
fn locked_vault_cannot_enumerate_approval_candidates_and_cancel_mutates_nothing() {
    let f = Fixture::unlocked();
    let id = f.add(Some("https://gmail.com/"));
    f.locks.lock_and_emit().unwrap();
    assert_eq!(
        f.call(request_approval("https://accounts.google.com/", "locked"))["ok"],
        true
    );
    let approval_id = f
        .service
        .pending_host_approval()
        .unwrap()
        .unwrap()
        .approval_id;
    assert!(matches!(
        f.service.host_approval_candidates(&approval_id),
        Err(super::service::HostApprovalError::Locked)
    ));
    f.auth.unlock(MASTER).unwrap();
    f.service.cancel_host_approval(&approval_id).unwrap();
    let record = f
        .auth
        .require_vault_key(|key| f.vault.get(key, &id))
        .unwrap()
        .unwrap();
    assert_eq!(record.website.as_deref(), Some("https://gmail.com/"));
    assert!(record.allowed_login_hosts.is_empty());
    assert!(f.service.pending_host_approval().unwrap().is_none());
}

#[test]
fn desktop_candidates_are_password_free_and_approval_appends_exact_host() {
    let f = Fixture::unlocked();
    let id = f.add_with_hosts(Some("https://gmail.com/"), &["login.example.test"]);
    error(&f.call(query("https://accounts.google.com/")), "NO_MATCHES");
    error(
        &f.fill(&id, "https://accounts.google.com/"),
        "DOMAIN_MISMATCH",
    );
    f.call(request_approval(
        "https://www.accounts.google.com/signin",
        "approve-google",
    ));
    let approval_id = f
        .service
        .pending_host_approval()
        .unwrap()
        .unwrap()
        .approval_id;
    let candidates = f.service.host_approval_candidates(&approval_id).unwrap();
    let serialized = serde_json::to_value(&candidates).unwrap();
    assert_eq!(serialized.as_array().unwrap().len(), 1);
    assert_eq!(serialized[0]["credentialId"], id);
    assert_eq!(serialized[0]["website"], "https://gmail.com/");
    assert!(!serialized.to_string().contains(PASSWORD));
    assert!(serialized[0].get("password").is_none());

    let completed = f.service.approve_login_host(&approval_id, &id).unwrap();
    assert_eq!(completed.requested_host, "accounts.google.com");
    assert!(completed.changed);
    assert!(f.service.pending_host_approval().unwrap().is_none());
    let record = f
        .auth
        .require_vault_key(|key| f.vault.get(key, &id))
        .unwrap()
        .unwrap();
    assert_eq!(record.website.as_deref(), Some("https://gmail.com/"));
    assert_eq!(
        record.allowed_login_hosts,
        ["login.example.test", "accounts.google.com"]
    );
    assert_eq!(
        f.call(query("https://accounts.google.com/"))["data"]["matches"][0]["credentialId"],
        id
    );
    assert_eq!(
        f.fill(&id, "https://accounts.google.com/")["data"]["password"],
        PASSWORD
    );
    error(
        &f.fill(&id, "https://accounts.google.com.evil.example/"),
        "DOMAIN_MISMATCH",
    );
}

#[test]
fn synthetic_microsoft_redirect_requires_then_gains_only_exact_host_approval() {
    let f = Fixture::unlocked();
    let id = f.add(Some("https://www.microsoft.com/"));
    let login_page = "https://login.live.com/oauth20_authorize.srf";

    error(&f.call(query(login_page)), "NO_MATCHES");
    error(&f.fill(&id, login_page), "DOMAIN_MISMATCH");

    f.call(request_approval(login_page, "approve-live-host"));
    let pending = f.service.pending_host_approval().unwrap().unwrap();
    assert_eq!(pending.requested_host, "login.live.com");
    let completed = f
        .service
        .approve_login_host(&pending.approval_id, &id)
        .unwrap();
    assert_eq!(completed.requested_host, "login.live.com");
    assert!(completed.changed);

    let record = f
        .auth
        .require_vault_key(|key| f.vault.get(key, &id))
        .unwrap()
        .unwrap();
    assert_eq!(
        record.website.as_deref(),
        Some("https://www.microsoft.com/")
    );
    assert_eq!(record.allowed_login_hosts, ["login.live.com"]);
    assert_eq!(
        f.call(query(login_page))["data"]["matches"][0]["credentialId"],
        id
    );
    assert_eq!(f.fill(&id, login_page)["ok"], true);
    error(
        &f.call(query("https://login.live.com.evil.test/")),
        "NO_MATCHES",
    );
}

#[test]
fn duplicate_approval_is_idempotent_and_does_not_rewrite_the_record() {
    let f = Fixture::unlocked();
    let id = f.add_with_hosts(
        Some("https://gmail.com/"),
        &["accounts.google.com", "login.example.test"],
    );
    let before = f
        .auth
        .require_vault_key(|key| f.vault.get(key, &id))
        .unwrap()
        .unwrap();
    f.call(request_approval(
        "https://www.accounts.google.com/",
        "duplicate",
    ));
    let approval_id = f
        .service
        .pending_host_approval()
        .unwrap()
        .unwrap()
        .approval_id;
    let result = f.service.approve_login_host(&approval_id, &id).unwrap();
    assert!(!result.changed);
    let after = f
        .auth
        .require_vault_key(|key| f.vault.get(key, &id))
        .unwrap()
        .unwrap();
    assert_eq!(after.website, before.website);
    assert_eq!(after.allowed_login_hosts, before.allowed_login_hosts);
    assert_eq!(after.updated_at_ms, before.updated_at_ms);
}

#[test]
fn failed_approval_update_is_atomic_and_keeps_pending_request_retryable() {
    let f = Fixture::unlocked();
    let hosts: Vec<String> = (0..20)
        .map(|index| format!("login-{index}.example.test"))
        .collect();
    let host_refs: Vec<&str> = hosts.iter().map(String::as_str).collect();
    let id = f.add_with_hosts(Some("https://gmail.com/"), &host_refs);
    f.call(request_approval(
        "https://accounts.google.com/",
        "atomic-failure",
    ));
    let approval_id = f
        .service
        .pending_host_approval()
        .unwrap()
        .unwrap()
        .approval_id;
    assert!(matches!(
        f.service.approve_login_host(&approval_id, &id),
        Err(super::service::HostApprovalError::Internal)
    ));
    let record = f
        .auth
        .require_vault_key(|key| f.vault.get(key, &id))
        .unwrap()
        .unwrap();
    assert_eq!(record.allowed_login_hosts, hosts);
    assert_eq!(
        f.service
            .pending_host_approval()
            .unwrap()
            .unwrap()
            .approval_id,
        approval_id
    );
}

#[test]
fn request_fill_independently_revalidates_changed_alternate_hosts() {
    let f = Fixture::unlocked();
    let id = f.add_with_hosts(Some("https://gmail.com/"), &["accounts.google.com"]);
    let page = "https://accounts.google.com/signin";
    assert_eq!(f.call(query(page))["ok"], true);
    let mut changed = input(Some("https://gmail.com/"));
    changed.allowed_login_hosts = vec!["login.example.test".into()];
    f.auth
        .require_vault_key(|key| f.vault.update(key, &id, changed))
        .unwrap()
        .unwrap();
    error(&f.fill(&id, page), "DOMAIN_MISMATCH");
}

#[test]
fn status_tracks_existing_auth_states_without_creating_vault_storage() {
    let f = Fixture::new();
    assert_eq!(f.call(status())["data"]["state"], "locked");
    assert!(!f.temp.path().join("vault.enc").exists());
    drop(f.auth.create_master_password(MASTER).unwrap());
    assert_eq!(
        f.call(status()),
        json!({"version":1,"requestId":"status-1","ok":true,"type":"status","data":{"state":"unlocked"}})
    );
    f.locks.lock_and_emit().unwrap();
    assert_eq!(f.call(status())["data"]["state"], "locked");
}

#[test]
fn damaged_profile_status_is_safe_and_secret_requests_are_denied() {
    let f = Fixture::new();
    std::fs::write(f.temp.path().join("profile.json"), b"damaged fixture").unwrap();
    let auth = AuthService::load(
        ProfileStore::new(f.temp.path().into()),
        KdfParams::testing(),
        Arc::new(OsEntropy),
    );
    let service = AutofillService::new(auth, f.vault.clone(), f.gate.clone());
    assert_eq!(call(&service, status())["data"]["state"], "error");
    error(&call(&service, query(PAGE)), "VAULT_LOCKED");
}

#[test]
fn locked_requests_fail_before_url_validation_or_vault_access() {
    let f = Fixture::new();
    for page in [PAGE, "invalid"] {
        error(&f.call(query(page)), "VAULT_LOCKED");
        error(&f.fill(&"a".repeat(32), page), "VAULT_LOCKED");
    }
    assert!(!f.temp.path().join("vault.enc").exists());
}

#[test]
fn match_response_exposes_only_matching_summary_fields() {
    let f = Fixture::unlocked();
    let first = f.add(Some(PAGE));
    let second = f.add(Some("https://WWW.EXAMPLE.TEST./other"));
    let third = f.add(Some("https://example.test"));
    for website in [
        None,
        Some("www.example.test"),
        Some("http://www.example.test"),
        Some("https://www.example.test.evil.test"),
    ] {
        f.add(website);
    }
    let response = f.call(query(PAGE));
    assert_eq!(response["ok"], true);
    assert_eq!(response["type"], "matches");
    assert_eq!(response["requestId"], "query_1");
    let matches = response["data"]["matches"].as_array().unwrap();
    assert_eq!(matches.len(), 3);
    for item in matches {
        assert!([first.as_str(), second.as_str(), third.as_str()]
            .contains(&item["credentialId"].as_str().unwrap()));
        assert_eq!(item.as_object().unwrap().len(), 3);
        assert_eq!(item["name"], "Fixture account");
        assert_eq!(item["username"], "fixture@example.test");
    }
    for forbidden in [
        "password",
        "vaultKey",
        "masterPassword",
        "recoveryKey",
        "website",
        "tags",
        "createdAtMs",
        "private-tag",
    ] {
        assert!(!response.to_string().contains(forbidden));
    }
}

#[test]
fn no_matches_and_unsupported_page_errors_are_distinct() {
    let f = Fixture::unlocked();
    error(&f.call(query(PAGE)), "NO_MATCHES");
    error(&f.call(query("http://www.example.test")), "UNSUPPORTED_URL");
    error(&f.fill(&"a".repeat(32), ""), "UNSUPPORTED_URL");
}

#[test]
fn fill_returns_only_selected_username_and_exact_password() {
    let f = Fixture::unlocked();
    let id = f.add(Some(PAGE));
    // A fresh Fill independently authorizes; queries do not create capabilities.
    assert_eq!(
        f.fill(&id, PAGE),
        json!({"version":1,"requestId":"fill-1","ok":true,"type":"fillPayload","data":{"username":"fixture@example.test","password":PASSWORD}})
    );
    error(
        &f.fill(&id, "https://www.example.test.evil.test"),
        "DOMAIN_MISMATCH",
    );
    error(&f.fill(&"f".repeat(32), PAGE), "CREDENTIAL_NOT_FOUND");
}

#[test]
fn missing_or_invalid_stored_websites_cannot_release_a_password() {
    let f = Fixture::unlocked();
    for website in [
        None,
        Some(""),
        Some("www.example.test"),
        Some("http://www.example.test"),
        Some("https://www.example.test../"),
    ] {
        let id = f.add(website);
        error(&f.fill(&id, PAGE), "DOMAIN_MISMATCH");
    }
}

#[test]
fn fill_reloads_changed_website_password_and_deleted_selection() {
    let f = Fixture::unlocked();
    let id = f.add(Some(PAGE));
    assert_eq!(f.call(query(PAGE))["ok"], true);
    f.auth
        .require_vault_key(|key| {
            f.vault
                .update(key, &id, input(Some("https://different.test")))
        })
        .unwrap()
        .unwrap();
    error(&f.fill(&id, PAGE), "DOMAIN_MISMATCH");
    let mut replacement = input(Some(PAGE));
    replacement.password.zeroize();
    replacement.password = "updated-fixture-secret".into();
    f.auth
        .require_vault_key(|key| f.vault.update(key, &id, replacement))
        .unwrap()
        .unwrap();
    assert_eq!(
        f.fill(&id, PAGE)["data"]["password"],
        "updated-fixture-secret"
    );
    f.auth
        .require_vault_key(|key| f.vault.delete(key, &id))
        .unwrap()
        .unwrap();
    error(&f.fill(&id, PAGE), "CREDENTIAL_NOT_FOUND");
}

#[test]
fn lock_after_query_refuses_stale_selection_and_subsequent_queries() {
    let f = Fixture::unlocked();
    let id = f.add(Some(PAGE));
    assert_eq!(f.call(query(PAGE))["ok"], true);
    f.locks.lock_and_emit().unwrap();
    error(&f.fill(&id, PAGE), "VAULT_LOCKED");
    error(&f.call(query(PAGE)), "VAULT_LOCKED");
    f.auth.unlock(MASTER).unwrap();
    assert_eq!(f.fill(&id, PAGE)["ok"], true);
}

#[test]
fn request_waiting_behind_lock_rechecks_authentication() {
    let f = Fixture::unlocked();
    let id = f.add(Some(PAGE));
    let guard = f.gate.lock();
    let service = f.service.clone();
    let (started_tx, started_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        started_tx.send(()).unwrap();
        done_tx.send(call(&service, json!({"version":1,"requestId":"queued","type":"requestFill","pageUrl":PAGE,"credentialId":id}))).unwrap();
    });
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(done_rx.recv_timeout(Duration::from_millis(40)).is_err());
    f.locks.lock_and_emit_with_operation_guard(&guard).unwrap();
    drop(guard);
    error(
        &done_rx.recv_timeout(Duration::from_secs(2)).unwrap(),
        "VAULT_LOCKED",
    );
    worker.join().unwrap();
}

#[test]
fn response_delivery_is_in_the_same_protected_operation_as_authorization() {
    let f = Fixture::unlocked();
    let id = f.add(Some(PAGE));
    let (started_tx, started_rx) = mpsc::channel();
    let (locked_tx, locked_rx) = mpsc::channel();
    let locks = f.locks.clone();
    let mut worker = None;
    f.service.dispatch(&serde_json::to_vec(&json!({"version":1,"requestId":"race","type":"requestFill","pageUrl":PAGE,"credentialId":id})).unwrap(), |bytes| {
        assert_eq!(serde_json::from_slice::<Value>(bytes).unwrap()["ok"], true);
        worker = Some(thread::spawn(move || {
            started_tx.send(()).unwrap();
            locks.lock_and_emit().unwrap();
            locked_tx.send(()).unwrap();
        }));
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(locked_rx.recv_timeout(Duration::from_millis(40)).is_err());
        Ok(())
    }).unwrap();
    locked_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    worker.unwrap().join().unwrap();
    error(&f.call(query(PAGE)), "VAULT_LOCKED");
}

#[test]
fn delivery_failure_releases_gate_without_retained_authorization() {
    let f = Fixture::unlocked();
    let id = f.add(Some(PAGE));
    let request = json!({"version":1,"requestId":"failure","type":"requestFill","pageUrl":PAGE,"credentialId":id});
    assert!(f
        .service
        .dispatch(&serde_json::to_vec(&request).unwrap(), |_| Err(
            io::Error::other("fixture write failure")
        ))
        .is_err());
    f.locks.lock_and_emit().unwrap();
    error(&f.fill(&id, PAGE), "VAULT_LOCKED");
}

#[test]
fn queries_status_and_fills_do_not_postpone_the_existing_auto_lock_deadline() {
    let f = Fixture::unlocked();
    let id = f.add(Some(PAGE));
    let auto_lock =
        AutoLockService::new_for_test(Arc::new(f.locks.clone()), Duration::from_secs(60));
    let start = Instant::now() - Duration::from_secs(120);
    auto_lock.arm_at_for_test(start);
    for _ in 0..3 {
        f.call(status());
        f.call(query(PAGE));
        f.fill(&id, PAGE);
    }
    assert!(auto_lock.expire_at_for_test(start + Duration::from_secs(60)));
    assert_eq!(f.auth.status(), AuthStatus::Locked);
    error(&f.fill(&id, PAGE), "VAULT_LOCKED");
}

#[test]
fn fill_does_not_read_other_records_and_corrupt_data_is_a_safe_error() {
    let f = Fixture::unlocked();
    let selected = f.add(Some(PAGE));
    let damaged = f.add(Some("https://other.test"));
    let db = rusqlite::Connection::open(f.temp.path().join("vault.enc")).unwrap();
    db.execute(
        "UPDATE vault_records SET ciphertext = X'00' WHERE id = ?1",
        [&damaged],
    )
    .unwrap();
    assert_eq!(f.fill(&selected, PAGE)["ok"], true);
    error(&f.fill(&damaged, "https://other.test"), "INTERNAL_ERROR");
    error(&f.call(query(PAGE)), "INTERNAL_ERROR");
}

#[test]
fn request_parser_rejects_malformed_utf8_json_duplicates_and_oversized_input() {
    for bytes in [
        b"".as_slice(),
        b"[]",
        br#"[1,"array-id","status"]"#,
        br#"[1,"array-id","queryMatches","https://www.example.test"]"#,
        b"null",
        b"{}",
        b"{",
        b"\xff",
        br#"{"version":1,"requestId":"a","type":"status","type":"status"}"#,
        br#"{"version":1,"requestId":"a","type":"status"} {}"#,
        br#"{"version":1,"requestId":"a","type":"status","unknown":true}"#,
        br#"{"version":1,"requestId":"a","type":"status","pageUrl":null}"#,
    ] {
        assert!(Request::parse(bytes).is_err());
    }
    let f = Fixture::new();
    f.service
        .dispatch(&vec![b' '; MAX_MESSAGE_BYTES + 1], |bytes| {
            let response: Value = serde_json::from_slice(bytes).unwrap();
            error(&response, "INVALID_REQUEST");
            assert!(response["requestId"].is_null());
            Ok(())
        })
        .unwrap();
}

#[test]
fn invalid_protocol_version_type_fields_and_identifiers_are_rejected() {
    let f = Fixture::new();
    for request in [
        json!({"version":2,"requestId":"known-id","type":"status"}),
        json!({"version":2,"requestId":"known-id","type":"requestHostApproval","pageUrl":PAGE}),
        json!({"version":1,"requestId":"known-id","type":"unlock"}),
        json!({"version":1,"requestId":"known-id","type":"status","pageUrl":PAGE}),
        json!({"version":1,"requestId":"known-id","type":"queryMatches"}),
        json!({"version":1,"requestId":"known-id","type":"requestHostApproval"}),
        json!({"version":1,"requestId":"known-id","type":"requestHostApproval","pageUrl":PAGE,"credentialId":"a".repeat(32)}),
        json!({"version":1,"requestId":"known-id","type":"queryMatches","pageUrl":PAGE,"credentialId":"a".repeat(32)}),
        json!({"version":1,"requestId":"known-id","type":"queryMatches","pageUrl":"x".repeat(8193)}),
        json!({"version":1,"requestId":"known-id","type":"requestFill","pageUrl":PAGE,"credentialId":"../profile.json"}),
        json!({"version":1,"requestId":"known-id","type":"requestFill","pageUrl":PAGE,"credentialId":"A".repeat(32)}),
    ] {
        let response = f.call(request);
        error(&response, "INVALID_REQUEST");
        assert_eq!(response["requestId"], "known-id");
    }
    for id in ["".into(), "a".repeat(129), "bad\nid".into(), "💥".into()] {
        let response = f.call(json!({"version":1,"requestId":id,"type":"status"}));
        error(&response, "INVALID_REQUEST");
        assert!(response["requestId"].is_null());
    }
}

#[test]
fn secret_payload_and_match_metadata_wipe_and_debug_is_redacted() {
    fn wipes<T: Zeroize + ZeroizeOnDrop>() {}
    wipes::<FillPayload>();
    wipes::<MatchSummary>();
    let mut payload = FillPayload {
        username: "sentinel-user".into(),
        password: PASSWORD.into(),
    };
    assert_eq!(format!("{payload:?}"), "FillPayload([REDACTED])");
    payload.zeroize();
    assert!(payload.username.is_empty() && payload.password.is_empty());
}

#[test]
fn serialization_rejects_oversize_without_delivering_partial_json() {
    let data = ResponseData::FillPayload(FillPayload {
        username: "fixture".into(),
        password: "x".repeat(MAX_MESSAGE_BYTES),
    });
    assert!(matches!(
        encode_response("bounded", &data),
        Err(ErrorCode::InternalError)
    ));
    let f = Fixture::unlocked();
    for _ in 0..100 {
        let mut record = input(Some(PAGE));
        record.name = "n".repeat(200);
        record.username = "u".repeat(500);
        f.auth
            .require_vault_key(|key| f.vault.create(key, record))
            .unwrap()
            .unwrap();
    }
    let response = f.call(query(PAGE));
    error(&response, "INTERNAL_ERROR");
    assert_eq!(response["requestId"], "query_1");
}

#[test]
fn vault_public_errors_map_to_fixed_autofill_errors_without_internal_details() {
    for (internal, expected) in [
        (VaultError::NotFound, "CREDENTIAL_NOT_FOUND"),
        (VaultError::DataDamaged, "INTERNAL_ERROR"),
        (VaultError::StorageUnavailable, "INTERNAL_ERROR"),
        (VaultError::EntropyUnavailable, "INTERNAL_ERROR"),
    ] {
        let public = ErrorCode::from(PublicIpcError::from(internal)).public();
        assert_eq!(public.code, expected);
        assert!(public.retry_after_ms.is_none());
        for forbidden in [
            "SQL",
            "crypto",
            "nonce",
            "ciphertext",
            "profile.json",
            "vault.enc",
        ] {
            assert!(!public.message.contains(forbidden));
        }
    }
}
