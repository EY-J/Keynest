# Autofill V1 final acceptance

2026-09-08: **Checkpoint G PASS. Autofill V1 complete. No V2 work started.**

This record supersedes the incomplete 2026-09-07 acceptance records. Final work
was a non-destructive source/security review, regression runs and documentation
updates; no production behavior, dependencies, permissions or CSP changed.

## Browser acceptance and provenance

The user explicitly reported human manual acceptance complete and passed for
both browsers. These are user-performed results, not a claim that the agent
performed those manual clicks during the final regression run.

| Manual check | Chrome | Edge |
| --- | --- | --- |
| Explicit Fill | PASS | PASS |
| No auto-submit | PASS | PASS |
| Locked-vault refusal | PASS | PASS |
| Lookalike-domain rejection | PASS | PASS |
| Post-fill storage review | PASS | PASS |
| Popup and worker network review | PASS | PASS |
| Console/log review | PASS | PASS |

Edge screenshots and reported checks additionally show matching summaries,
populated fields with submitCount zero and no password attribute, refusal on a
lookalike hostname, and empty popup Web Storage, IndexedDB and Cache Storage
with chrome.storage unavailable. Worker/popup network and Console observations
were reported empty while exercising Fill. Chrome's final manual pass is the
user's explicit report; its manual browser version was not separately supplied.

Earlier Chrome 152.0.7977.82 automation passed ten native checks and one
absent-desktop check through the actual extension, Rust host and desktop pipe.
Those checks include stale Fill after locking, actual document-ID injection,
ambiguous/missing forms and safe app-unavailable behavior. They supplement,
rather than replace, the manual acceptance.

## Final regression results

All commands below were rerun successfully on 2026-09-08:

| Check | Result |
| --- | --- |
| npm run build (npm.cmd on Windows) | PASS |
| node --experimental-vm-modules --test --test-reporter=dot tests/*.test.mjs | 88 passed |
| cargo fmt --check | PASS |
| cargo clippy --locked --offline --all-targets --all-features | PASS |
| cargo test --locked --offline --quiet | 253 library + 2 executable integration tests passed; 2 intentional fixtures ignored |
| tests/autofillHostRegistration.tests.ps1 with process-scoped ExecutionPolicy Bypass | PASS, mocked registration/ownership/rollback checks |
| node --check for each browser-extension/src/*.js | PASS |
| scripts/Test-AutofillDom.ps1 -Browser Chrome | 22 passed |
| scripts/Test-AutofillDom.ps1 -Browser Edge | 22 passed |

Rust commands ran in src-tauri. DOM runners used disposable browser profiles,
real DOM/layout/events and local synthetic fixtures, outside the tool sandbox;
they do not independently constitute extension/native acceptance. The ignored
Rust cases are interactive pipe and disposable-vault seeding fixtures, not failed
unit tests. Earlier checkpoint records retain their separate fixture execution.
Existing frontend chunk-size, Rust user-path canonicalization and Node
experimental-module warnings were not suppressed. No regression failure remains.

## Final security invariants

All 19 specification invariants were checked against the implementation and
available regression/manual evidence. Paths below are relative to the repository.

| # | Invariant | Evidence and result |
| --- | --- | --- |
| 1 | Desktop gains no remote network access | PASS: Autofill desktop transport is platform/autofill_pipe only; no network client/listener added. Existing Tauri development tooling is unchanged. |
| 2 | Extension has no cloud/API path | PASS: extension source/policy tests and connect-src 'none'; manual popup/worker network review passed. |
| 3 | No all_urls permission | PASS: manifest and policy tests; no host_permissions. |
| 4 | User-triggered activeTab access | PASS: popup action/Retry and explicit Fill; worker validates popup sender and current tab, window, URL and document. |
| 5 | Browser/native uses Native Messaging | PASS: worker native connection, native_host framing and real browser acceptance. |
| 6 | Exact extension allowed_origins | PASS: registration helper generates exact loaded extension IDs; rejects malformed IDs; ownership/rollback tests passed. |
| 7 | Host never decrypts vault files | PASS: native_host relays bounded typed messages; no AuthService/VaultService or vault-file access in host path. |
| 8 | Host talks only to local running app | PASS: fixed current-user/session Windows pipe; no app launch, unlock or remote fallback. |
| 9 | No localhost HTTP/WebSocket server | PASS: Autofill uses stdio and named pipes; browser fixtures use inherited DevTools pipes, not a debugging server. Existing Vite development server is not an Autofill transport. |
| 10 | Locked vault returns no summaries/secrets | PASS: shared AuthService require_vault_key and SecurityOperationGate through delivery; lock race tests and both manual refusals. |
| 11 | requestFill revalidates host | PASS: fresh selected VaultService.get and website match inside the protected operation; changed/deleted record tests. |
| 12 | Exact host fails closed | PASS: URL parser requires HTTPS, rejects malformed/userinfo URLs, canonicalizes hostname, then exact equality; URL tests and manual lookalike refusal. |
| 13 | queryMatches returns no password | PASS: dedicated summary schema, operation-specific native response reconstruction and protocol tests. |
| 14 | Password only on explicit requestFill | PASS: offered credential ID consumed by Fill, fresh preflight/current-page checks, no automatic query-to-fill; worker tests and human clicks. |
| 15 | No extension password persistence | PASS: no storage permission/calls; transient handling; both manual post-fill storage reviews. |
| 16 | No password logging | PASS: no production Autofill payload logging; redacted Rust Debug, fixed panic/public errors, protocol-only host stdout, executable tests and both manual Console/log reviews. |
| 17 | No auto-submit | PASS: no submit/click path in injection, policy/DOM tests and both manual checks. |
| 18 | Ambiguous forms fail closed | PASS: unique eligible password field and deterministic same-form preceding username selection; ties/ambiguity refused; unit, DOM and prior Chrome native checks. |
| 19 | Existing lock/recovery/crypto behavior unchanged | PASS: shared existing auth/vault/lock services and gate; no parallel unlock state or Autofill activity rearming; existing regression suite passed. Crypto-file change is legacy test formatting only. |

This is review within the documented V1 threat model, not a guarantee against
same-user malware or an audit of all operating-system diagnostics. Page scripts
can read credentials intentionally filled into their page. JavaScript strings
cannot be reliably zeroized; storage emptiness does not establish memory erasure.
A lock prevents subsequent protected retrieval/delivery; it cannot revoke a
secret already delivered to the browser before locking.

## Required implementation report

1. **Files:** added src-tauri/src/autofill/*, src-tauri/src/platform/autofill_pipe/*,
   src-tauri/src/bin/keynest_native_host.rs, src-tauri/tests/native_host_stdio.rs,
   browser-extension/manifest.json and src/*, registration/acceptance scripts,
   Autofill JS/PowerShell tests and fixtures, and security/release documentation.
   Integration changes are src-tauri/Cargo.toml, src-tauri/src/lib.rs and
   src-tauri/src/platform/mod.rs. The specification now records completion.
   Other pre-existing workspace edits are not attributed to Autofill.
2. **Architecture:** MV3 extension → Native Messaging host → Windows named pipe
   → running desktop → existing unlocked VaultService, mediated by AutofillService.
   No duplicate vault/auth service. No new crate or npm package; existing
   serde_json raw_value and windows-sys features support the implementation.
3. **Permissions:** exactly activeTab, scripting, nativeMessaging; Chromium 106+
   for document-ID targeting. No persistent site access.
4. **Extension CSP:** default-src 'none'; script-src 'self'; style-src 'self';
   img-src 'self'; object-src 'none'; connect-src 'none'; base-uri 'none';
   form-action 'none'; frame-src 'none'. Desktop CSP remains unchanged.
5. **Host name:** com.eyy.keynest.autofill.
6. **Registry:** HKCU\Software\Google\Chrome\NativeMessagingHosts\com.eyy.keynest.autofill
   and HKCU\Software\Microsoft\Edge\NativeMessagingHosts\com.eyy.keynest.autofill.
   Separate browser manifests contain exact actual extension IDs; no wildcard.
7. **IPC:** local named pipe keyed by current SID/session, current-user-only DACL,
   remote clients rejected, first-instance ownership and session checks on both
   ends. Client uses identification-only security quality of service. Bounded
   cancellable I/O; response write bounded to 250 ms while holding the lock gate,
   receipt wait outside it. Same-user malware is outside this authorization boundary.
8. **Protocol:** version 1 status/queryMatches/requestFill JSON with correlated
   request IDs and strict schemas; 4-byte length prefix, 1..65536-byte bodies.
   Bounded URLs/IDs, fixed public errors, operation-specific responses. Native
   stdout is exclusively protocol frames; no raw backend diagnostics forwarded.
9. **Matching:** parse HTTPS URL, reject malformed authorities/userinfo, normalize
   host case and one terminal dot, compare exact canonical hostname. Ports do
   not broaden hostname matching; no suffix, subdomain or fuzzy matching.
10. **Lock:** the existing auth key requirement and shared security operation gate
    cover lookup, serialization and delivery. LockCoordinator uses that same gate.
    Autofill introduces no timer, unlock state or idle-timer refresh.
11. **Summaries:** reuse VaultService.list and encrypted website metadata, return
    only matching ID/name/username. Existing list internals may decrypt records;
    no bulk passwords cross the Autofill summary boundary.
12. **Full record:** only selected VaultService.get at requestFill, with fresh
    website validation; no full-record cache or key sent to the extension.
13. **Fields:** main frame only; bounded visible, enabled, editable input search;
    exactly one eligible password and an unambiguous preceding same-form username
    or email. Hidden/new-password/ambiguous cases fail closed.
14. **Controlled inputs:** native input value setter and bubbling input/change
    events support framework-controlled inputs. DOM tests verify this mechanism;
    arbitrary deployed React/Vue applications were not individually certified.
15. **Submission:** never invokes submit/requestSubmit or login-button clicks;
    the user submits the form. Automated and manual checks passed.
16. **Secret lifetime:** Rust zeroizing payloads/buffers and redacted Debug;
    bounded typed host relay; worker releases references on completion/cancellation,
    injection drops references, password never enters popup markup/storage/logs.
    Document-ID targeting, stale-page checks and short injection deadline fail closed.
17. **Tests added:** matching/protocol/service/lock races, Windows pipe security
    and framing/cancellation, native executable stdio, registration rollback,
    extension policy/worker/fill tests, native DOM fixtures and isolated native
    browser/manual acceptance tooling. Historical checkpoint notes provide detail.
18. **Results:** final build, formatting, lint and all regressions passed as listed above.
19. **Chrome manual:** PASS, explicitly reported by the user.
20. **Edge manual:** PASS, explicitly reported by the user, supported by the
    recorded screenshots and boolean observations. Automation limitation below.
21. **Intentional limits:** Windows Chrome/Edge, manual Fill, exact HTTPS hostname,
    main frame only. No iframe/shadow-form support, automatic fill/submission,
    webpage capture/save, passkeys, OTP/TOTP, generated-password insertion,
    cloud sync, browser vault or fuzzy/subdomain matching. Installer registration,
    signing, store publishing and managed deployment are outside this V1 acceptance.
22. **Remaining verification:** no required V1 acceptance item remains open.
    Internal browser crash cause, whole-machine packet/memory/crash-dump auditing,
    arbitrary framework sites and every OS sleep/session variant were not manually
    verified. Existing lock architecture/tests support the reviewed lock invariant;
    these limits are not claims of additional manual testing.

## Edge automation limitation

Extensions.triggerAction caused msedge.exe to exit with 0xc0000005, including
with a static MV3 extension containing no scripts, permissions, KeyNest code,
host registration or vault integration. Node reported the browser disconnection;
Node did not suffer that access violation. The internal cause remains unknown.
The investigation also retains an intermittent Chrome browser exit and subsequent
successful rerun; the evidence does not establish an Edge-exclusive defect.

Actual Edge toolbar/native Fill subsequently passed manual acceptance without
changing production checks. Therefore the reproduced crash is an automation
limitation, not an outstanding V1 native acceptance failure. The human-assisted
runner separately stopped at its desktop foreground-focus guard; this was not
the browser crash and is not recorded as a fully passing script run. Guided
manual interaction completed the acceptance instead. No focus/security guard
was bypassed in production. See the [original investigation](autofill-g-edge-investigation.md)
for diagnostics and reproduction. Its unretained browser stderr is not claimed
audited or harmless; final source, framing tests and user Console/log review
provide the scoped application logging evidence.
