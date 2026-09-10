# KeyNest Autofill V1

Status: **Checkpoint G and Autofill V1 complete (2026-09-08).**
See the [final acceptance and 19-invariant review](autofill-v1-completion.md).
The authoritative requirements remain [the specification](../../KEYNEST_AUTOFILL_V1_SPEC.md).

## Architecture and repository integration

The implemented path is Chrome/Edge MV3 extension -> Native Messaging ->
`com.eyy.keynest.autofill` native host -> Windows named pipe -> running desktop ->
existing unlocked vault service. KeyNest remains offline. There is no HTTP,
TCP, WebSocket, cloud, or remote API transport for Autofill.

`src-tauri/src/autofill/` implements the core. Desktop setup in `lib.rs` creates
one AutofillService using clones of the same AuthService, VaultService, and
SecurityOperationGate already managed by Tauri. Cloning AuthService and the gate
shares their original synchronization/state; no second vault, authentication
state, or key cache is created. VaultService has no decrypted-record cache.

There are no new Tauri commands, WebView capabilities, CSP changes, crates,
database migrations, or changes to cryptography, Master Password, Recovery Key,
clipboard, or existing lock behavior. The Rust library exposes the service's
bounded JSON dispatch interface; its constructor is crate-private and its fields
are private. Callers cannot obtain key material or a database handle through it.

## Request and response contract

Checkpoint B handles JSON bodies. Checkpoint C adds bounded Windows pipe framing
and I/O deadlines. Checkpoint D handles browser stdin/stdout framing.

Requests are single JSON objects with `version: 1`, `requestId`, and `type`.
Request IDs contain 1-128 ASCII letters/digits/hyphens/underscores (UUIDs fit).
Page URLs are strings of at most 8192 UTF-8 bytes. Credential IDs use the existing
32-character lowercase hexadecimal representation. Input and output JSON are
limited to 65536 bytes each, including all escaping/envelope overhead.

| Request type | Additional fields | Successful response |
| --- | --- | --- |
| `status` | None | `type: "status"`, `data: { state }` |
| `queryMatches` | `pageUrl` | `type: "matches"`, `data: { matches: [{ credentialId, name, username }] }` |
| `requestFill` | `pageUrl`, `credentialId` | `type: "fillPayload"`, `data: { username, password }` |

Successful envelopes include `version: 1`, the original `requestId`, and
`ok: true`. The core reports status `unlocked`, `locked` (also setup required),
or `error` (damaged profile). The host/transport will supply `app_not_running`
and `unsupported` when it can actually establish those conditions.

Error envelopes have `version: 1`, `requestId`, `ok: false`, `type: "error"`,
and `error: { code, message }`. The implementation reuses PublicIpcError's fixed
safe fields and the existing auth/vault-to-public-error conversions. It maps
those safe outcomes to the Autofill codes without changing the desktop taxonomy:

- `VAULT_LOCKED`: a well-formed query/fill cannot access an unlocked key.
- `UNSUPPORTED_URL`: the current page URL is ineligible or invalid.
- `NO_MATCHES`: no stored website matches the current host.
- `CREDENTIAL_NOT_FOUND`: the selected valid ID no longer exists.
- `DOMAIN_MISMATCH`: the selected record's current stored website is absent,
  invalid, unsupported, or a different host.
- `INVALID_REQUEST`: bad protocol/schema/field bounds.
- `INTERNAL_ERROR`: storage, damaged record, other internal failure, or response
  exceeds the JSON size bound. No partial or silently truncated matches are sent.

Malformed UTF-8/JSON, duplicate fields, unknown fields/types/versions, missing
fields, null/non-string optional fields, invalid identifiers, and fields
belonging to another operation are rejected. A validated request ID is preserved
on operation/version/field-bound errors. Malformed envelopes or invalid IDs
produce `requestId: null`; no untrusted ID is recovered by reparsing malformed
JSON. Errors never include incoming URL, username, password, SQL, crypto details,
local paths, or underlying exception text. The ID is correlation only, never an
authorization token or replay exemption.

## Exact-host matching

Both page and saved URLs are parsed by the existing `tauri::Url` re-export of
the URL parser. Parsing performs no DNS lookup. Only absolute HTTPS URLs with
an explicit `://` authority are accepted. Backslashes, literal whitespace/control
characters, nonempty user-info, empty hosts/labels, and repaired missing-authority forms
are rejected. Scheme-less stored values are never prefixed or rewritten.

Compare parsed hostnames after ASCII lowercase and removal of one trailing dot.
An extra trailing dot remains invalid. The parser's IDNA and IP normalization
apply; Unicode and its corresponding ASCII IDNA hostname identify the same host.
Paths, query strings, fragments, and ports are ignored under the specification's
hostname-only rule. This is not an origin/port restriction: the same HTTPS host
on ports 443 and 8443 matches. There is no wildcard, fuzzy, substring, public
suffix, registrable-domain, or implicit subdomain matching.

`facebook.com` does not match `www.facebook.com`, `fake-facebook.com`, or
`facebook.com.evil.example`. `mail.google.com` does not match
`accounts.google.com`. Missing/invalid/HTTP stored websites remain readable and
editable by existing vault features but are ineligible for Autofill.

## Vault reads and lock ordering

Every well-formed query/fill acquires the existing shared operation gate and
uses AuthService::require_vault_key before URL validation or vault access. Locked,
setup-required, and damaged-profile states cannot read summaries or fill secrets.

Matching calls VaultService::list, then projects only the three specified summary
fields. Website metadata shares an encrypted payload with the password in the
existing storage format, so list internally decrypts each record, extracts its
summary, and drops zeroizing password intermediates. This checkpoint does not
claim to avoid that internal decryption or introduce a metadata index/cache.

Fill calls VaultService::get for only the selected ID and independently validates
its current stored website against the current supplied page URL. A prior query
is not an authorization capability. Changing/deleting a record or locking after
query cannot reuse an earlier authorization. Changes to a selected password are
read fresh. An unrelated damaged record does not make a selected-record Fill
read all records, although list correctly fails closed on damaged records.

The synchronous dispatch callback receives borrowed serialized bytes while the
same shared gate still protects the operation. The payload and serialized buffer
are dropped before releasing the gate, including delivery errors/unwinding.
Requests waiting behind a completed lock recheck auth and fail locked. A lock
waits for an already active protected operation, as it does elsewhere in KeyNest.

The transport callback performs only a bounded write with a short deadline. It
does not wait for incoming requests/browser acknowledgements, call other
gate-taking services, or retain response bytes. See the Windows transport section
for C's actual cancellation, connection bounds, access restrictions, and tests.

All existing lock paths converge on the same state/gate: manual lock, inactivity,
lock shortcut, Windows session lock/disconnect/logoff, configured sleep/resume,
and protected reset. No new timer or parallel lock state is introduced. Status,
query, and Fill never call record_activity or arm auto-lock. The existing pause
during one-time Recovery Key display is unchanged; Autofill follows the backend
auth state and does not add a separate definition of unlocked.

## Secret lifetime and threat boundary

Fill moves username/password from the selected zeroizing record into a zeroizing
FillPayload without cloning the password at that boundary. The full record wipes
on drop, including rejected domain selections. Match DTOs also wipe on drop;
existing VaultRecordSummary handling remains unchanged. Fill Debug is redacted;
requests/responses otherwise provide no Debug implementation or logging.

JSON serialization writes directly into a Zeroizing buffer preallocated to the
64 KiB bound, preventing secret-bearing reallocations in this serializer. Partial
serialization failures wipe their buffer and return a small fixed error. No
Autofill plaintext cache, file, clipboard operation, or persistent storage exists.
These measures do not guarantee erasure of all library/OS copies, registers,
stack data, process dumps, or future browser/JavaScript copies.

Later browser code must request Fill only on explicit user action, recheck the
active tab/destination across navigation, target the main frame, fail on ambiguous
forms, and never submit. The destination page can access a password after filling;
already delivered secrets cannot be revoked by locking. JavaScript strings cannot
be reliably zeroized and must not be stored or logged. This feature does not
protect against a compromised browser, arbitrary code running as the same Windows
user, process-memory access, or administrator/kernel control.

## Windows named-pipe transport (Checkpoint C)

`src-tauri/src/platform/autofill_pipe/` owns identity/ACL creation, overlapped I/O,
the server, and the client forwarding function for the future native host.
`src-tauri/src/autofill/framing.rs` provides bounded byte-stream frames.
The only dependency change enables these features on existing windows-sys 0.59:
Win32_Security, Win32_Security_Authorization, Win32_Storage_FileSystem,
Win32_System_IO, Win32_System_Pipes, and Win32_System_Threading.
Cargo.lock has no dependency/version changes.

The fixed local path is `\\.\pipe\keynest-autofill-v1-<current-user-SID>-<session-id>`.
The SID/session are Windows identity metadata, not secrets. No request chooses a
production pipe path. The pipe uses PIPE_REJECT_REMOTE_CLIENTS, a protected DACL
with exactly one current-user allow ACE, and non-inheritable handles. The ACL
mask is `0x0012019b`: data read/write, EA/attribute access, READ_CONTROL and
SYNCHRONIZE, excluding FILE_CREATE_PIPE_INSTANCE, WRITE_DAC, WRITE_OWNER and
generic rights. Both peers also check the other endpoint's actual Windows
session through named-pipe APIs. The client requests only read/write data and
synchronize, with SECURITY_IDENTIFICATION to avoid granting impersonation use.

This follows Microsoft's [named-pipe access guidance](https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-security-and-access-rights):
generic write includes permission to create pipe instances. The server uses
[FILE_FLAG_FIRST_PIPE_INSTANCE and PIPE_REJECT_REMOTE_CLIENTS](https://learn.microsoft.com/en-us/windows/win32/api/namedpipeapi/nf-namedpipeapi-createnamedpipew).
It keeps one handle for its entire lifetime, disconnecting/reconnecting that
instance rather than leaving a gap where another server can claim the name.
There is one worker, one pipe instance, and no application request queue or
per-client thread creation.

Windows desktop setup starts the server after existing security services and
session monitoring. ExitRequested/Exit signal shutdown without blocking the UI
thread. The worker cancels/drains pending I/O and closes the pipe. If it is already
waiting for an existing protected operation, it cannot cancel that Rust mutex
wait; the callback checks shutdown and refuses output when it eventually runs.
This preserves existing operation ordering without permitting a post-shutdown
secret response. An already delivered response cannot be revoked.

Pipe startup failure leaves Autofill unavailable while the vault and existing
lock services remain usable; no relaxed ACL or alternate transport is tried.
If multiple KeyNest processes share the same user/session, the first successful
listener owns Autofill; later instances do not replace it. The current client
maps a missing endpoint to AppNotRunning and busy/access/transport failures to
IpcUnavailable. Missing endpoint does not prove there is no desktop process
(its pipe setup might have failed); browser error UX should say unavailable.

Each connection handles one request and one response. Frames have a four-byte
little-endian length followed by 1-65536 UTF-8 JSON bytes, matching Windows native
endianness. The core independently validates incoming JSON/protocol. The client
validates requests before connecting and returns a bounded Zeroizing response
buffer; the Native Messaging host validates its response schema. No plaintext
payload is sent to logging or disk.

| Phase | Bound |
| --- | --- |
| Busy-client connection retry | 750 ms |
| Complete incoming frame, including fragmented reads | 2 seconds |
| Client request/response/receipt I/O | 10 seconds total after connecting |
| Server response write, including header and all fragments | 250 ms, also capped by a 10-second dispatch deadline |
| Receipt read after response | 250 ms, outside the security gate |
| Idle accept | Waits on both connection and shutdown events |

The dispatch deadline refuses stale output; it does not preempt existing crypto,
database work, or the shared operation mutex. Existing vault busy-timeout/locking
semantics are unchanged. Frame buffers wipe on drop. Response serialization and
the response write remain inside the shared gate, with no read/receipt wait there.

After consuming the entire response the client sends one byte `0x06`, an IPC-only
receipt (never forwarded to the browser). The server then disconnects. This avoids
discarding unread buffered response bytes at DisconnectNamedPipe without using
an unbounded FlushFileBuffers wait. Receipt timeout or disconnect simply ends the
connection; no secret is resent or cached. Request/response buffers are dropped
before the receipt wait. See Microsoft's [disconnect semantics](https://learn.microsoft.com/en-us/windows/win32/api/namedpipeapi/nf-namedpipeapi-disconnectnamedpipe).

I/O uses FILE_FLAG_OVERLAPPED and a completion event. Timeout/shutdown calls
CancelIoEx and drains completion before releasing buffers or OVERLAPPED storage,
as required by [Windows cancellation semantics](https://learn.microsoft.com/en-us/windows/win32/api/ioapiset/nf-ioapiset-cancelioex).
Shutdown is a terminal ConnectionAborted error, not Interrupted: Rust exact-read
and write-all helpers retry Interrupted. The initial native tests caught that
retry-loop bug; the fix and a specific regression test are included. Clients
disconnecting before accept are recycled without terminating the listener.

These restrictions do not authenticate a trusted executable against arbitrary
same-user malware, a compromised browser, or an administrator. Remote rejection
is configured in the actual CreateNamedPipe call; no second-machine remote
connection or separate-user interactive logon was exercised in this environment.
Native tests inspect the installed kernel DACL and exercise both session checks.

## Two-process local IPC smoke procedure

Use only the disposable test fixture. This procedure needs neither the existing
personal vault nor a real desktop restart. From the repository root, in one terminal:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline manual_fixture_server -- --ignored --nocapture
```

The test prints a unique `SMOKE_PIPE` name and accepts lock/unlock/quit on stdin.
These fixture controls are test-only, never production protocol operations. In a
second terminal, use the printed name:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/Test-AutofillPipe.ps1 -PipeName '<SMOKE_PIPE>' -State Unlocked
```

The process-scoped policy option was necessary under this machine's script policy;
it changes no persistent execution policy. The inspected smoke script only accepts
test pipe names, performs local pipe requests, and prints fixed pass/fail summaries.
It verifies status, matching with no password field, the selected fake Fill,
domain mismatch, and no matches. Type `lock` in the fixture terminal, then run:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/Test-AutofillPipe.ps1 -PipeName '<SMOKE_PIPE>' -State Locked
```

This verifies locked status and query/fill refusal. Type `quit` in the fixture
terminal; the server stops and the temporary encrypted fixture vault is removed.

Executed in C: both smoke phases passed using a Rust fixture server and a separate
PowerShell/.NET client. The script's first run exposed unintended async-task return
values in the PowerShell pipeline; these are now discarded and subsequent unlocked
and locked phases passed. No real credential payloads were used or logged. This is
tool-driven interactive local IPC verification, not a human Chrome/Edge acceptance
test or a test of the Tauri GUI startup/shutdown path in a running desktop.

## Native Messaging host (Checkpoint D)

`src-tauri/src/bin/keynest_native_host.rs` is a separate console-subsystem
executable named `keynest-native-host.exe`. It calls only the host entry point,
never desktop setup, key derivation, vault storage, or a second VaultService.
The existing package/library is reused, with `default-run = "keynest"` preserving
the normal desktop run target. The only D dependency change enables `raw_value`
on existing serde_json; no crate/version or Cargo.lock change is required.

`autofill/native_host.rs` accepts the Chromium extension-origin argument and
optional decimal `--parent-window` argument. This checks argument format; the
browser's exact manifest `allowed_origins` provides extension authorization.
Same-user arbitrary code can imitate arguments, which remains outside the threat
boundary. Invalid startup arguments cause a quiet exit with no protocol output.

Rust's binary stdin/stdout carry four native-endian length bytes, then 1-65536
UTF-8 JSON bytes. On Windows this is little-endian, as required by Chromium.
Fragmented reads and multiple requests on a long-lived Port are supported.
Clean EOF exits quietly; truncated or invalid framing returns one fixed
`INVALID_REQUEST` response then exits without resynchronizing. Invalid JSON,
UTF-8, schema, operation and version are rejected before any pipe access. A
well-framed invalid request does not prevent the next framed request. The
executable emits no stdout diagnostics; it reuses the existing sanitized panic
hook on stderr without arbitrary payloads/paths.

The host calls C's fixed user/session pipe client. It never accepts a pipe path,
launches/unlocks the desktop, resets activity, or retains a response cache.
A missing endpoint returns status `app_not_running`, or `APP_NOT_RUNNING` for
query/fill. Other connection, timeout or response-validation failures return
`IPC_UNAVAILABLE`. Non-Windows status returns `unsupported` and query/fill refuse
IPC; Windows is the only supported/tested native platform.

`autofill/response.rs` validates version, request correlation, explicit JSON
objects, known fields, response shape, field bounds, and operation correspondence.
Status and summary requests cannot forward a fill payload; summaries cannot
contain a password. Borrowed RawValue fields avoid a generic secret-bearing JSON
tree, and only validated DTOs are reserialized. Backend error messages are
discarded and replaced with fixed public text; unknown codes fail closed.
Full response/frame buffers and accepted credential DTOs wipe on drop. As with
the existing serde/zeroize patterns, parser scratch/error paths and OS copies
are not guaranteed erased. There is no persistence or secret logging.

The host retains only one response while writing its browser frame, then drops
it. Native stdio can block while its browser peer stops reading; this does not
hold the desktop security gate or prevent desktop lock. Data already handed to
the local pipe/host cannot be revoked by a subsequent lock. Desktop authorization
is checked on every subsequent request. Browser-side request cancellation and
stale-response handling are described in the E/F sections below.

The reference manifest has no guessed IDs. Development registration requires
actual exact IDs and an existing local executable, writes a UTF-8 browser-specific
manifest, and changes only the corresponding HKCU host registration. Chrome and
Edge registrations are independent. Scripts refuse conflicting registrations,
restore the previous manifest on registry-write failure, support WhatIf, and
unregister without recursive file/registry deletion. They do not weaken browser
policy, execution policy, extension CSP or vault access. See
[development setup and removal](../release/browser-autofill-dev-setup.md) for
commands, registry views, allowlists and the remaining browser acceptance work.

## MV3 extension shell (Checkpoint E)

This section records the E baseline; F extends the Fill interface below.

`browser-extension/` is plain JavaScript/HTML/CSS with no dependency or generated
bundle. Its manifest requests exactly activeTab, scripting and nativeMessaging.
There are no host permissions, persistent content scripts, external messaging
handlers or web-accessible resources. The CSP is:

```text
default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self';
object-src 'none'; connect-src 'none'; base-uri 'none'; form-action 'none';
frame-src 'none';
```

The service-worker entry registers a passive internal connection listener. Only
this extension's exact bundled popup URL is accepted, with no sender tab or
subframe. Unlike a general native-protocol relay, the E interface accepts only a
versioned/correlated refresh command with no URL or credential ID. The worker
reads the active tab using temporary activeTab access and performs status followed
by queryMatches only if unlocked. It does not match saved domains in JavaScript;
the existing Rust service remains authoritative. Unsupported/missing/loading
URLs are rejected before native connection. URL eligibility also rejects user
info, whitespace/control/backslash repairs and missing HTTPS authority.

An internal runtime Port (instead of one-shot sendMessage) gives the worker an
explicit popup-disconnect lifecycle. Only one popup session is retained. A native
Port is scoped to each refresh and closed after success/failure. Each native
request has a 12-second timeout, refresh has a 30-second limit including tab API
waits, and the popup has a 32-second worker-response limit. No timeout schedules
another request. Navigation/reload, tab switch/removal and window focus loss
invalidate current metadata, cancel pending work and suppress stale responses.
Fresh tab/window/URL checks also run after native replies. Closing/hiding the
popup removes session listeners/timers and drops metadata. C's pipe deadlines
and D's per-request backend authorization remain unchanged.

The worker validates native version, request ID, exact operation/shape, known
errors, summary fields and UTF-8 message bounds. It rejects fill payloads and
extra password fields. Native error strings and Chromium lastError text never
reach the UI/logs. The popup validates worker views, uses textContent for metadata
and clears old rows before retry/error/change. There is no storage API, network
primitive or logging in extension code. No extension request invokes activity,
unlock, key access, vault files, or password retrieval. Fill controls are disabled
and there is no injection code in E; scripting is reserved for F.

Displayed summaries are snapshots, not cached authorization. Without polling or
a lock push channel they can remain visible until retry/navigation/popup close;
all subsequent native queries recheck locked state. F must independently check
the current page and lock state before obtaining a password and must never use a
prior summary as authority. No secure JavaScript string erasure is claimed.

The implementation follows Chrome's [activeTab access model](https://developer.chrome.com/docs/extensions/develop/concepts/activeTab),
[Port lifetime model](https://developer.chrome.com/docs/extensions/develop/concepts/messaging),
[extension CSP rules](https://developer.chrome.com/docs/extensions/reference/manifest/content-security-policy),
and [Native Messaging API](https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging).
Native messaging is an extension API rather than a web connection; actual
connectNative operation under this CSP is reserved for native Chrome/Edge
acceptance and is not established by mocks. See the
[extension README](../../browser-extension/README.md) for load/setup and shell
smoke steps. No real browser/registry changes were made in E.

## Explicit Fill (Checkpoint F)

F enables a trusted popup Fill click to send only a validated selected ID. The
worker accepts IDs offered by its latest successful query for the current tab,
consumes that set for one attempt and never queues/overlaps Fill. Retry obtains
fresh choices. The popup receives only summary metadata or fixed result states;
no password flows through its runtime Port, UI state, DOM or errors.

The worker rechecks its tab/window/full URL snapshot and runs a secret-free
main-frame inspection through `fill-client.js`. The self-contained `fill.js`
routine runs in the isolated world and returns only ok/error. The browser supplies
the inspected document ID. A missing/non-main-frame result fails closed before
password retrieval. Only then does the worker send requestFill to the existing
native host, which invokes the unchanged Rust authorization/selected-record/host
checks. No Rust API, vault service, activity semantics or cryptography changes.

After the native reply, a fresh active-tab check precedes the final injection,
which uses `target.documentIds: [inspectedDocumentId]`. There is no fallback to
a frame ID if that document no longer exists. This closes the same-tab document
replacement gap without adding webNavigation permissions. Manifest minimum
Chromium version is 106, when [document IDs became available in the scripting API](https://developer.chrome.com/docs/extensions/reference/api/scripting).
Permissions and CSP are unchanged. Both injections use isolated-world functions,
immediate scheduling, and a one-second expiry checked inside the function. Full
URL equality, top-level frame and visible-document checks apply inside the
destination as well as the worker. An already dispatched browser operation cannot
be atomically recalled on popup close/lock; expired, hidden or replaced documents
refuse, and canceled replies never update UI. Already delivered values cannot be
revoked, as in the existing desktop-to-host boundary.

Form detection inspects only input metadata/layout, never scrapes existing values
during preflight. It caps inspection at 2000 inputs and requires exactly one
visible editable password field plus a preceding editable username/email with
the same form owner. Nearby formless pairs are supported. Hidden/inert ancestors,
no rendered box, display/visibility/opacity/content-visibility exclusion,
disabled fieldsets, readonly fields and new-password-only forms are refused.
Multiple visible password fields abort with AMBIGUOUS_LOGIN_FORM. Username scores
are 300 for autocomplete=username, 200 for an email input/hint, otherwise
100 minus preceding input distance. Equal highest scores abort. Formless pairs
must be within two input positions; ordinary text candidates within four.
No broad account/form guessing, iframe traversal or shadow-root traversal occurs.

The routine uses the native HTMLInputElement.prototype.value setter, then bubbling
input/change events. Username events may rerender fields or navigate; destination,
chosen field eligibility, form ownership and password ambiguity are checked again
before setting the password. Post-event value checks detect a site clearing or
rejecting values; only a fixed result leaves the page. The implementation never
sets credential attributes/HTML, creates script tags, clicks buttons, generates
keyboard events or calls either submission method. It does not control code
already owned by the destination site. The native setter/event pattern was tested
with real browser input prototypes including an overridden instance setter;
compatibility with specific React/Vue applications still requires G acceptance.

Native Fill replies are accepted only for the correlated requestFill and exact
username/password shape. Existing 500/4096 character bounds guarantee the JSON
fits the 64-KiB envelope even with maximal escaping, avoiding another secret JSON
string solely for size checking. Raw native payload references are cleared after
validation; the selected transient payload remains only in the Fill operation
and injection arguments. Completion, failure and abort clear those references;
the injected routine clears its own payload references in finally. This is not
secure erasure of JavaScript strings, browser serialization buffers or destination
page memory. No storage, log, global password state, background retry or activity
update is introduced. Conservative unsupported cases include password-only or
readonly-username-only layouts, signup/change-password forms and ambiguous pairs.

## Validation and remaining scope

`src-tauri/src/autofill/tests.rs` uses synthetic credentials and temporary vaults.
It covers strict URLs/attacks, protocol errors and bounds, safe wire shapes,
selected-record reads, changed/deleted selections, damaged data, lock ordering
through delivery, lock-after-query, and unchanged inactivity deadlines. Explicit
zeroization/type tests do not read freed memory. Existing auth, recovery, crypto,
WebView policy, and release tests remain relevant regression coverage.

Commands from the repository root:

```powershell
npm.cmd run build
node --experimental-vm-modules --test tests/*.test.mjs
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline
cargo clippy --manifest-path src-tauri/Cargo.toml --locked --offline --all-targets --all-features
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

Checkpoint B validation on Windows: 228 Rust tests passed (22 new Autofill
tests plus 206 existing tests); 59 JavaScript tests passed; frontend build and
Clippy across all targets/features passed. Targeted rustfmt checks passed for
the changed Rust files. The first populated-array rejection regression failed
before the explicit object check was added; it passes in the final full suite.
The frontend build still reports its existing large-chunk warning, and Rust
tooling reports its existing user-path canonicalization warning.

Checkpoint A already identified an unrelated rustfmt failure in the legacy-record
test in `src-tauri/src/vault/crypto.rs`. Do not interpret passing Autofill tests
as a clean whole-repository formatting result. C adds two framing tests, sixteen
native pipe tests, and one explicitly invoked interactive fixture test. The full
Rust suite passed 246 tests with the interactive fixture ignored by default; that
fixture passed separately during the two-process smoke test. Frontend build,
59 JavaScript tests, and Clippy across all targets/features also passed. The
existing whole-repository rustfmt failure and tooling/build warnings remain.
At the end of C, extension policy/DOM tests and browser acceptance remained for E-G.
No installer, registry entry, extension, desktop GUI
acceptance, or browser acceptance was exercised in C.

Checkpoint D validation on Windows: 253 library tests plus two built-executable
integration tests passed (255 total); the interactive C fixture remains ignored
in the default suite and was exercised separately in C. Seven new host unit
tests cover fragmented/multiple messages, malformed inputs, operation/schema
validation, safe errors and argument handling. The two executable tests verify
real binary redirected stdin/stdout, request-ID preservation, malformed bounds,
clean EOF and absence of diagnostics. Mocked PowerShell registration tests passed
for exact IDs, path handling, UTF-8 manifests, separate browsers, rollback,
ownership refusal and idempotent removal. Clippy across all targets/features,
the frontend build and all 59 JavaScript tests passed. Targeted Rust formatting
passed; whole-repository formatting still fails only at the previously identified
legacy crypto test. Existing tooling/build warnings remain. No actual browser
registration, Chrome/Edge Native Messaging Port, extension UI or native browser
acceptance was tested in D. The real executable tests intentionally do not
connect to the existing personal desktop/vault.

Checkpoint E validation on Windows: all 78 JavaScript tests passed, including
19 new extension tests for manifest/CSP/network/storage policy, native validation,
popup sender restrictions, status/matches/locked states, tab changes, same-URL
reload, timeouts, superseding/closed popup cleanup and plain-text rendering.
Every extension JavaScript file passed `node --check`. The full Rust suite again
passed 255 tests (253 library and two executable tests), with the interactive
fixture ignored by default. Frontend build, Clippy across all targets/features
and `git diff --check` passed. Whole-repository rustfmt still reports only the
pre-existing legacy crypto test; E changes no Rust code. Existing large-chunk,
user-path canonicalization and Node experimental-module warnings remain. Browser
API and DOM tests use mocks; no Chrome/Edge unpacked loading, visual review,
actual registration or native Port/CSP acceptance was performed in E. These are
documented acceptance steps, not claimed test results. F implements Fill and DOM
detection tests; G requires native Chrome and Edge acceptance before V1 is complete.

Checkpoint F validation on Windows: all 88 JavaScript tests passed, including ten
new Fill boundary/lifecycle tests and the updated extension policy checks. The
full Rust suite passed 255 tests with the interactive fixture ignored by default;
Clippy across all targets/features, frontend build, extension syntax checks and
git diff checks passed. Whole-repository rustfmt still fails at the pre-existing
`vault/crypto.rs:132` layout; F changes no Rust source. Existing build/tooling
warnings remain. An initial source-policy test matched the word executeScript in
a comment; the check now detects calls, and the full suite passes.

`tests/fixtures/autofill-dom.html`/`.js` and `scripts/Test-AutofillDom.ps1` run 22
real DOM/layout/prototype/event checks in each installed headless browser. Both
Chrome and Edge passed all 22, including required field eligibility, ambiguity,
native setters, bubbling events, no automatic submit/markup, mutation/navigation
guards and payload-reference cleanup. The fixture substitutes a synthetic HTTPS
window location while using the browser's real DOM. Function construction for
that test adapter is confined to the local test fixture, never shipped extension
code. Initial restricted launches failed with Chromium GPU subprocess access
errors; the inspected disposable-profile runner passed outside the tool sandbox
with browser sandboxing still enabled. No HTTP/debug server, real credentials,
personal browser profile, extension installation or host registration was used.
These are headless DOM tests, not native extension/host acceptance. Actual browser
document-ID routing, CSP/native Port operation, real framework sites, visual UX
and the complete Chrome/Edge sections 34/35 procedure remain for G.

Intentional V1 limits: Windows; Chrome/Edge; manual Fill; exact HTTPS host; main
frame only. No iframe filling, page-load autofill, auto-submit, capture/save from
websites, passkeys, OTP/TOTP, generated-password insertion, cloud sync, browser-side
vault, or fuzzy/subdomain matching. Browser-store publishing, stable signing keys,
automatic app launch, and V2 features remain outside this work.

## Checkpoint G acceptance record — incomplete (2026-09-07)

**Historical record:** the incomplete status and outstanding work below describe
2026-09-07 only. They are superseded by the [2026-09-08 completion](autofill-v1-completion.md),
including both user-reported manual browser passes and final regressions.


Follow-up: [Edge crash isolation and current manual handoff](autofill-g-edge-investigation.md).
The newer record distinguishes the actual browser exit from the surviving Node
harness and documents the minimal extension reproduction, final Chrome rerun,
remaining uncertainty and intentionally retained fake fixture. G is still incomplete.

The full frontend build, all 88 JavaScript tests, 253 Rust library tests plus two
native-host executable tests, all-target/all-feature Clippy, whole-repository
`cargo fmt --check`, mocked registration tests and diff whitespace checks passed.
The default Rust suite now ignores two explicit fixtures: the C interactive pipe
fixture and G's disposable acceptance-vault seeder. The latter passed separately
using the existing AuthService/VaultService, production KDF and encrypted storage.
The previously reported legacy crypto-test formatting failure was corrected by
rustfmt only; no crypto behavior changed. Existing large frontend chunk, Rust
user-path canonicalization and Node experimental-module warnings remain.

G adds test tooling in `scripts/autofill-browser-driver.mjs`,
`scripts/Test-AutofillBrowser.mjs`, `scripts/probe-autofill-browser.mjs` and
`scripts/Control-AutofillDesktop.ps1`, plus the isolated Tauri configuration in
`tests/fixtures/tauri.autofill-acceptance.json` and ignored vault seeder in
`src-tauri/src/autofill/tests.rs`. No production dependency or permission was
added in G. The separate desktop build uses application identifier
`com.eyy.keynest.autofill-acceptance`, a separate target directory and synthetic
credentials. It still uses the production per-user/session named pipe; the
personal desktop had to be closed by the user before starting the fixture.
The tool never stopped or queried the personal desktop or vault.

Chrome **152.0.7977.82** passed ten native checks plus a separate absent-desktop
check. The unchanged unpacked extension's actual ID was
`ohjdijkmicaoalcbfgoajkgfnipbefai`; the runner registered that exact ID through
the existing HKCU script, then removed the registration and temporary manifest.
The browser launched the real Rust host, which contacted the running isolated
desktop through the production named pipe. Desktop unlock and lock used its
normal UI, including a guarded native click to open its navigation. Fill used
a trusted browser input event, temporary activeTab permission and actual browser
document-ID routing, with the restrictive extension CSP unchanged.

Passed native Chrome checks:

- Running locked desktop refusal; two exact-host summaries when unlocked.
- No page-load or popup-open autofill; selected fake credentials filled only
  after the trusted Fill click.
- Correct input/change events; no auto-submit, password attributes or password
  in popup markup.
- `chrome.storage` unavailable without permission; popup localStorage,
  sessionStorage, IndexedDB database list and Cache Storage empty after Fill.
- Lock after query refused the stale Fill and left the new form blank.
- Lookalike suffix and different-host pages offered no credentials.
- Ambiguous and missing login forms failed closed without modifying fields.
- Attached extension targets emitted no HTTP/WebSocket requests or fixture
  secrets in observed Runtime/Log events.
- With the desktop stopped, the real host returned the safe unavailable state
  and left fields blank.

The runner uses DevTools **inherited pipes**, with disposable browser profiles;
there is no HTTP/WebSocket debugging server. Synthetic HTTPS fixture documents
are supplied by DevTools request interception without network access to a test
site. This tests actual browser URL/document/extension behavior, not TLS or a
real site's React/Vue application. Browser sandboxing stayed enabled. The
extension received no debugger permission. `--enable-unsafe-extension-debugging`
belongs only to the disposable test browser so its external test driver can load
the unchanged unpacked extension and invoke its action.

These were **headless, tool-driven native checks**, not human manual acceptance.
Runtime network/log monitoring begins when the test attaches to popup/worker
targets; it does not cover every startup event or OS/browser/native diagnostic
source. Browser stderr was discarded, not audited. Source-policy and native
framing tests supplement these observations; they are not a whole-machine
network trace or proof of memory erasure. DevTools `Extensions.getStorageItems`
could not inspect this permission-free extension (browser-context/extension-not-
found errors); the final checks use unavailable `chrome.storage` and real Web
Storage/IndexedDB/Cache Storage inspection instead. Initial test-driver failures
in desktop accessibility selectors and popup/navigation timing were corrected
before the successful ten-check run. No production workaround was introduced.

Edge **152.0.4191.66** loaded the unchanged extension and reported the same actual
ID, but `Extensions.triggerAction` crashed Edge with exit code **3221225477**
(`0xC0000005`) in both headless and visible disposable profiles, including an
about:blank probe without any host registration or vault request. A toolbar UI
fallback failed its foreground-process guard and was not used to bypass that
guard. **Edge native Fill/lock/domain/storage acceptance did not pass or complete.**
F's 22 real DOM checks in each browser remain valid separate evidence, not Edge
Native Messaging acceptance.

After testing, both temporary HKCU registrations were verified absent, the
isolated desktop was closed, and its disposable encrypted application data was
removed. Generated acceptance build artifacts remain under the ignored target
directory. No browser store publishing or installer deployment occurred.

Remaining before G/V1 can be called complete: the full Edge native procedure;
human Chrome/Edge sections 34/35 acceptance including visual UX; and complete
manual network/storage/log review. Real framework-site behavior, OS sleep/session
events and browser navigation races beyond the existing automated fixtures are
not claimed as manually verified. No security architecture conflict was found;
the observed Edge failure blocks acceptance evidence, not authorization for a
production workaround.
