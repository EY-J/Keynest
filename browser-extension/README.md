# KeyNest Autofill extension

Autofill V1 and Autofill V2 through Checkpoint K provide a Manifest V3 extension for Chrome and Edge on Windows.
Open its popup to see logins matching the active HTTPS hostname in the running,
unlocked KeyNest desktop vault, then explicitly click **Fill** for one account.
**Checkpoint G and V1 complete (2026-09-08).** Both browsers passed user-performed
manual acceptance; see the [final report](../docs/security/autofill-v1-completion.md).
Chromium 106+ is required for document-ID targeting.

## Load and connect

1. Build the native host and run a KeyNest desktop build containing Autofill IPC.
   Use disposable credentials for development testing.
2. Open `chrome://extensions` or `edge://extensions`, enable Developer mode and
   choose **Load unpacked**, selecting this `browser-extension` directory.
   The source is plain JavaScript/HTML/CSS; no build or package install is needed.
3. Copy the actual extension ID displayed by that browser. Follow the
   [native host setup](../docs/release/browser-autofill-dev-setup.md) to register
   that exact ID for Chrome or Edge. No ID/signing key is embedded in the manifest.
4. Open an HTTPS website, open the KeyNest popup, and unlock the vault in KeyNest
   when prompted. Use **Retry** after unlocking or changing the saved credential.
   Click **Fill** beside the intended account; review the website and submit it
   yourself. Another attempt requires Retry to obtain fresh choices.
5. Repeat registration with Edge's actual ID if testing both browsers. Reload
   the unpacked extension after changing its source; reopen the popup to retry.

## Behavior and boundaries

The local path is popup -> MV3 service worker -> Chromium Native Messaging ->
`com.eyy.keynest.autofill` -> current-user/session Windows pipe -> existing
desktop Autofill/Vault/auth services. The host does not launch or unlock KeyNest.
The desktop and extension have no KeyNest cloud/API or local HTTP/WebSocket path.

The only permissions are `activeTab`, `scripting`, and `nativeMessaging`.
`scripting` is used only after an explicit Fill action. There are no host
permissions, persistent content scripts, externally connectable endpoints, web
accessible resources, remote scripts/fonts, storage APIs, analytics or logs.
The extension CSP allows bundled scripts/styles/images, blocks objects and
connections, and forbids frames, base URLs and form actions. Desktop CSP is
unchanged. Native Messaging uses the browser extension API, not a web request.
Actual Native Messaging operation under this CSP still requires native browser
acceptance; mocked tests cannot establish it.

An internal Port ties each popup to its worker session, so closing/hiding it
cancels pending native work and drops the displayed metadata. Only the exact
bundled popup URL from this extension is accepted, without a sender tab or
subframe. The worker derives the active tab itself; no caller supplies a URL.
The popup sends only a selected credential ID for Fill, restricted to the latest
offered IDs. It handles refresh and Fill messages and maintains at most one popup
session. The native Port exists only during a refresh or selected Fill request.
There are no background queries, retries, polling, or auto-lock activity updates.

Popup opening and Retry check status, then query summaries only if unlocked.
Navigation, reload, tab closure/switch or window focus loss invalidates the
display and pending operation without automatically starting another query.
The worker also rechecks tab ID, window and URL after native replies. Protocol
version, correlation, schema, field lengths and message size are validated;
password-bearing status/summary or unsolicited reply types are rejected. Error text comes only
from fixed local strings. Names and usernames use text nodes, never HTML markup.

The popup supports locked, unavailable, unsupported page/device, generic error,
no matches, and one/multiple matches. These are snapshots from the last request;
there is no background lock subscription. Retry clears old summaries and checks
the current backend state. Fill independently revalidates the lock state,
selected record and current destination; displayed metadata never grants vault
authorization. Filling, success, unsupported/ambiguous form, changed-domain and
deleted-record results use fixed messages and contain no passwords.

## Fill behavior

A popup open or Retry checks for exact-host credential matches, then runs a
password-free form inspection in browser-permitted frames in the isolated world. Detection
returns `combined`, `username_only`, `password_only`, `unsupported`, `ambiguous`,
`challenge` or `passwordless`, without field values. Fillable account buttons are labelled
**Fill login**, **Fill username**, or **Fill password** for the detected stage.
Unsupported, ambiguous, verification-code-only, website-challenge-only and
passwordless pages never request a Fill payload.

If no candidate is initially present, that explicit inspection may observe DOM
changes for at most 2.5 seconds so a delayed SPA/modal form can appear. The
observer is local to that one inspection, queries the DOM again after each
relevant mutation, and disconnects on detection or timeout. Popup open, Retry
and Fill each start a separate fresh inspection; no input nodes are retained.

A trusted Fill click consumes the offered selection and disables pending actions.
The worker checks the active tab and freshly inspects the stage again before it
requests the existing native Rust Fill operation. The final injection targets
that inspection's browser frame and document ID, not whichever document later
occupies the same tab or frame, and must observe the same stage. The injected
function again checks the exact top-level URL, same origin, visible frame chain,
visible document and one-second execution deadline.
URL equality for this operation is stricter than the vault's hostname-only matching.

The existing `activeTab` grant and `scripting` permission are used to inspect
frames where Chromium permits injection. A frame is eligible only when it can
verify the unchanged top-level URL, has the same origin as the top-level page,
and is visibly embedded through a same-origin frame chain. Exactly one fillable
frame must remain; multiple plausible frames are ambiguous. Cross-origin,
hidden, detached and changed frames cannot be selected and never receive a Fill
payload.

Detection supports a combined username/password pair, one clearly dominant
username-like field without a password, or one password without a usable visible
username. Username-only candidates require an email input, explicit username/email
autocomplete, or login-like name/ID; generic search, contact, newsletter and OTP
fields are excluded. Text, email and tel username inputs are supported. Candidate
ranking prefers explicit username/email autocomplete, email type and login-like
identity, then phone signals, with nearby text fields used only as a password-linked
fallback. Search, coupon, address, city, personal-name, message, comment, newsletter
and verification identities are rejected. Password ranking prefers current-password
semantics and a same-form or nearby-container username relationship. Hidden, inert,
disabled (including fieldsets), readonly, zero-box, transparent and new-password
fields are excluded. Equally plausible visible login forms abort. Nearby
formless/container-associated pairs remain supported. Detection traverses the
document and bounded open Shadow DOM trees, applying visibility through their
host chain; closed shadow roots remain inaccessible and are never bypassed. It
supports bounded same-origin iframe inspection but not cross-origin iframe fill.

Verification-code detection recognizes `autocomplete=one-time-code`, OTP/code/
verification/token/MFA/2FA names and IDs, four-to-eight-character numeric inputs,
and groups of at least four visible one-character inputs in one form or nearby
container. Those inputs are excluded from username and password candidates. An
OTP-only page reports the non-fillable `challenge` state. A safe combined,
username-only, or password-only credential stage takes priority over unrelated
OTP UI; OTP inputs receive no value or input/change events. KeyNest does not
generate, retrieve, or fill OTP/TOTP values.

Website-challenge detection recognizes visible generic CAPTCHA markers, bot or
human-verification language, device-approval prompts, and security-verification
controls. It reports the fixed `challenge` stage with an internal nonsecret kind
so the popup can distinguish website verification from OTP. Clear standard
login inputs take precedence and remain manually fillable, but KeyNest never
clicks, checks, submits, solves, invokes, or bypasses a challenge control. When
only challenge UI is present, the popup asks the user to complete the website's
verification step and retry.

Only when no safe standard credential or challenge stage exists does detection
recognize an explicit passwordless/passkey state from a visible usable input
whose autocomplete tokens include `webauthn`, or from a visible enabled
interactive control explicitly labelled for passkeys, passwordless sign-in,
WebAuthn, FIDO, security keys or magic links. Non-interactive copy and hidden/
disabled controls do not qualify. A visible dominant username—including one
with `autocomplete="username webauthn"`—a usable password, or a combined form
takes precedence over a passkey alternative. KeyNest only reports the fallback
state: it does not click, submit, invoke WebAuthn or choose **Use password
instead**. If the user exposes a password fallback on the site, opening the popup
or pressing Retry performs a fresh inspection and normal password filling is
available.

Values use the native HTMLInputElement value setter, followed by bubbling input
and change events. Combined stages write both fields; username-only and
password-only stages write only their detected field, and the unused payload
field is cleared before page injection. The chosen fields, stage and destination
are checked again after events. Only chosen input properties change; no credential
markup/attribute, script tag, click, Enter key, submission method, or page/native
messaging is used. Page-owned event handlers remain under the site's control.
No values are returned from the injected routine.

The password never enters the popup or a persistent/global cache. Worker and
injection references are cleared on completion, failure or cancellation; this
does not securely erase JS strings or browser IPC copies. Once an injection has
been handed to Chromium it cannot be revoked atomically by popup closure or a
later desktop lock. Document targeting, URL/visibility checks and the short
execution deadline constrain queued work. Already delivered values are accessible
to the destination page and cannot be recalled. Cancellation before dispatch
prevents injection; later results from canceled work never update the popup.

Rust remains authoritative for canonical HTTPS hostname matching and vault access.
Comparison lowercases the parsed hostname, removes one trailing dot, strips
exactly one leading `www.` label, then requires whole-host equality. Thus
`example.com` and `www.example.com` are equivalent, but `login.example.com` and
unrelated or lookalike hosts are not. A credential may still contain internally
stored explicit alternate login hosts in its encrypted payload. The normal
Add/Edit UI exposes only Website and preserves an existing hidden allowlist on
edit; it neither infers nor auto-approves new hosts. See the
[V2 host-handling notes](../docs/security/autofill-v2-host-handling.md). No fuzzy,
substring, public-suffix, registrable-domain, wildcard or arbitrary-subdomain
matching is added.
JavaScript cannot guarantee secure string erasure. This does not protect against
a compromised browser, arbitrary same-user malware or administrator/OS compromise.

## Contextual login-host approval

Normal exact-host matching always runs first. If it returns no approved login,
the popup shows the exact current hostname and a user-triggered **Review in
KeyNest** action. The request sent over Native Messaging contains only the V1
request envelope and current HTTPS page URL—never a credential ID, password, or
vault listing. Credential selection and final confirmation happen in the
unlocked desktop app, where both the saved Website and requested exact hostname
remain visible.

Rust appends an explicitly approved canonical hostname to the credential's
encrypted internal `allowed_login_hosts` list without replacing its Website or
existing entries. Equivalent duplicates are ignored. The normal Add/Edit form
keeps this technical field hidden and preserves it on edit. KeyNest makes no
provider, registrable-domain, suffix, substring, redirect, or subdomain
inference.

Approval never sends a password to the extension and never fills or submits the
page. After desktop confirmation, the user must click **Retry** in the popup and
then use the normal explicit staged Fill action. `requestFill` still performs its
own exact-host revalidation. Cancel, X, Escape, or backdrop dismissal persist no
approval. The feature adds no browser permission, network API, local HTTP server,
WebSocket, browser storage, or polling.

## Safe compatibility diagnostics

Production inspection remains minimal: the worker and popup receive only the
validated stage/frame selection they need, and no diagnostic report is created or
logged. `inspectFormDiagnostics` is an explicit developer-only snapshot for
unsupported-page investigation. It returns bounded structural data: detected
stage/kind, candidate counts, input type/autocomplete/id/name, visibility, score,
fixed rejection reason, and same-origin browser frame/document identifiers. It
never reads or returns `.value`, page text, credentials, fill payloads, Master
Password, Recovery Key or Vault Key data.

Use only a disposable/test page. First click the extension action to establish
the normal `activeTab` grant, open the extension service worker's DevTools, and
evaluate:

```js
const [tab] = await chrome.tabs.query({ active: true, lastFocusedWindow: true });
const { eligiblePage } = await import(chrome.runtime.getURL("src/protocol.js"));
const { inspectFormDiagnostics } = await import(chrome.runtime.getURL("src/fill-client.js"));
await inspectFormDiagnostics(chrome, {
  ...eligiblePage(tab.url), tabId: tab.id, windowId: tab.windowId,
});
```

The call performs one immediate, password-free inspection; it starts no observer,
native connection or network request. Results are not persisted by KeyNest, but
the DevTools console may retain evaluated output. Field `id` and `name` metadata
can still be site-sensitive, so review and redact a report before sharing it.

## Tests and acceptance

From the repository root:

```powershell
node --test tests/autofillExtension.test.mjs tests/autofillExtensionPolicy.test.mjs
node --test tests/autofillFill.test.mjs
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/Test-AutofillDom.ps1 -Browser Chrome
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/Test-AutofillDom.ps1 -Browser Edge
```

These use synthetic data, browser API mocks and a minimal DOM harness. They cover
policy, native protocol/schema, sender checks, missing/locked app, matches, stale
results, timeout/cancellation, cleanup and safe rendering. Static network/storage
checks are regression guards, not proof against deliberately obfuscated code.
The separate DOM runner uses installed headless browsers, a fresh disposable
profile, and local file fixtures. It starts no HTTP/debug server and registers no
extension/host. The file-access flag applies only to the disposable fixture
profile; browser sandboxing stays enabled. The test supplies a synthetic HTTPS
window location while using real DOM/layout/prototypes/events. It therefore
does not verify real HTTPS navigation, extension isolation or document-ID routing.
Both Chrome and Edge passed 74 DOM checks through V2 Checkpoint K plus the
login-stage priority regression. The complete
synthetic scenario mapping is recorded in the
[Checkpoint K matrix](../docs/security/autofill-v2-compatibility-matrix.md); it
does not claim real-site V2 support. Their initial launches failed
inside the tool sandbox; running the inspected fixture outside that tool sandbox
resolved Chromium subprocess access failures. No personal browser profile or
vault was used.

Manual extension smoke procedure (Chrome and Edge separately): load and register using
the actual ID; verify unavailable when the endpoint is absent; locked when the
desktop is locked; correct summaries after unlock/Retry; no matches for a distinct
hostname; multiple fake accounts listed; explicit Fill changes only the selected
form and never submits; locked Fill and domain mismatch refuse; navigation/reopen
drops old results; popup close leaves no idle native Port. Check browser extension
errors and network tooling without printing native messages or credentials.
Chrome passed ten automated native checks plus an absent-desktop check in G.
Both Chrome and Edge subsequently passed user-performed manual Fill, no-submit,
locked refusal, lookalike, storage, network and Console/log acceptance. Final
regressions passed. See the [completion report](../docs/security/autofill-v1-completion.md).
Edge's Extensions.triggerAction crash remains an automation limitation: it also
reproduced with a static extension containing no KeyNest integration. The
[investigation](../docs/security/autofill-g-edge-investigation.md) preserves the
evidence. No production workaround was introduced.

V1 remains Windows and Chrome/Edge only, manual Fill, main frame and exact HTTPS
host. The implemented V2 checkpoints add staged forms, explicit alternate hosts,
bounded dynamic/open-shadow/same-origin-frame detection, recognition-only
passkey/OTP/challenge states, safe developer diagnostics, and the synthetic
compatibility matrix.
There is no cross-origin iframe fill, page-load autofill, auto-submit, webpage
capture/save, passkey interaction, OTP fill, generated-password insertion, cloud
sync, or browser vault.
See the [security notes](../docs/security/autofill-v1.md) for the full threat model
and the tested desktop/native transport boundaries.
