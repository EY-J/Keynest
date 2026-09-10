# Autofill native host development setup

Checkpoint D provides the Windows native host and per-user registration scripts.
Checkpoints E/F provide the loadable MV3 extension with explicit Fill. The browser
setup below can now be followed for status, matching and filling on Chromium 106+.
No extension ID is guessed. Checkpoint G and V1 are complete (2026-09-08).
Both browsers passed user-performed manual acceptance and final regressions passed.
See the [completion record](../security/autofill-v1-completion.md).

## Build

From the repository root on Windows with the existing KeyNest Rust/Tauri toolchain:

```powershell
cargo build --manifest-path src-tauri/Cargo.toml --locked --offline --bin keynest-native-host
```

The console-subsystem executable is
`src-tauri/target/debug/keynest-native-host.exe`. It uses binary Rust stdin/stdout
for Chromium Native Messaging; stdout contains protocol frames only. It never
starts Tauri or loads/decrypts a vault. Cargo's default run target remains the
`keynest` desktop executable. Start a desktop build containing Checkpoint C to
make its local pipe available. The host never launches or unlocks KeyNest.

For an optimized executable, add `--release` and use `target/release` instead.
This is a development build procedure, not installer integration. Existing
desktop bundles have not been changed to install/register the host; production
packaging and executable signing remain outside this V1 development acceptance.
Do not relocate the registered host executable without rerunning registration.

## Obtain actual extension IDs

1. Open `chrome://extensions` or `edge://extensions`, enable Developer mode and
   load the repository's `browser-extension` directory unpacked. It requires no
   separate build step or dependency installation.
2. Copy the actual extension ID shown by that browser. It must be 32 lowercase
   characters from `a` through `p`. The IDs may differ between Chrome and Edge.
3. Run registration with that actual ID. An unpacked extension moved to another
   path/profile may receive a different ID; register the observed ID again.

No stable signing key or store publishing is part of V1 development setup.

## Register Chrome or Edge

Run in PowerShell from the repository root as your ordinary Windows user:

```powershell
$nativeHost = (Resolve-Path 'src-tauri/target/debug/keynest-native-host.exe').Path
$chromeId = Read-Host 'Actual unpacked Chrome extension ID'
./scripts/register-autofill-host.ps1 -Browser Chrome -ExtensionId $chromeId -HostExePath $nativeHost

$edgeId = Read-Host 'Actual unpacked Edge extension ID'
./scripts/register-autofill-host.ps1 -Browser Edge -ExtensionId $edgeId -HostExePath $nativeHost
```

Each script supports `-WhatIf`. If local policy blocks these inspected development
scripts, invoke them using a process-scoped PowerShell `-ExecutionPolicy Bypass`
option only if permitted by your machine's policy; no persistent policy change or
administrator privileges are required by the scripts.

The host name is `com.eyy.keynest.autofill`. Generated manifests use an absolute
local executable path, `type: "stdio"` and exact
`chrome-extension://<actual-ID>/` origins. Wildcards and malformed IDs are rejected.
`-ExtensionId` also accepts an array of exact IDs when both are deliberately
authorized; a single browser-specific ID is sufficient for separate registrations.
The JSON template in this directory has an empty allowlist until real IDs are
supplied. It is a reference template, not an installable manifest.

Default manifests are UTF-8 without BOM in `%LOCALAPPDATA%\KeyNest\Autofill`:

- `com.eyy.keynest.autofill.Chrome.json`
- `com.eyy.keynest.autofill.Edge.json`

The default value at each of these HKCU keys points to its corresponding manifest:

- `Software\Google\Chrome\NativeMessagingHosts\com.eyy.keynest.autofill`
- `Software\Microsoft\Edge\NativeMessagingHosts\com.eyy.keynest.autofill`

Scripts use the invoking PowerShell process's registry view. Use the same
PowerShell architecture for registration and removal. Chrome checks the 32-bit
view before the 64-bit view; an old registration in another view can take
precedence. Inspect/remove that registration explicitly instead of creating
conflicting entries. The scripts never modify HKLM, browser policies, or unrelated
registry values. See the official [Chrome Native Messaging documentation](https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging)
and [Edge Native Messaging documentation](https://learn.microsoft.com/en-us/microsoft-edge/extensions/developer-guide/native-messaging).

`-ManifestDirectory` selects another local directory. Registration refuses to
overwrite a host registration pointing elsewhere or an unrelated manifest file.
It restores the previous manifest if setting the registry value fails. Register
Chrome and Edge separately to avoid one browser silently changing the other's
allowlist. Browser origin arguments are format-checked by the executable; the
browser's exact-origin manifest allowlist is the authorization boundary, not an
authentication mechanism against arbitrary same-user processes.

## Unregister

```powershell
./scripts/unregister-autofill-host.ps1 -Browser Chrome
./scripts/unregister-autofill-host.ps1 -Browser Edge
```

If registration used `-ManifestDirectory`, pass the same value on removal. Removal
is idempotent and deletes only that browser's KeyNest default registry value and
fixed manifest file. It removes the host registry key only if otherwise empty.
It does not delete the executable, extension, directories, or vault data.

## Validation and troubleshooting

Before diagnosing a message that works for status/query but fails for a newer
operation, inspect the browser's HKCU registration and the referenced manifest.
The manifest must point to the native host built from the same source revision as
the desktop. Disposable acceptance runs use the same per-user registry key and a
host under `target/autofill-acceptance`; if an interrupted run leaves that exact
temporary registration behind, unregister it with its original
`-ManifestDirectory`, rebuild, and restore the normal `target/debug` registration.
The native host is spawned per browser request, so rebuilding its executable is
sufficient; restart `npm run tauri dev` for Rust/React desktop changes, and use
the browser's Extensions page Reload button for extension source changes.

Open the extension on an HTTPS website. It requests status and matching summaries
on open or Retry only, and requests a password only after an explicit Fill click.
See the
[extension README](../../browser-extension/README.md) for the Chrome/Edge shell
smoke procedure and lifecycle behavior.

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline
powershell.exe -NoProfile -ExecutionPolicy Bypass -File tests/autofillHostRegistration.tests.ps1
node --test tests/autofillExtension.test.mjs tests/autofillExtensionPolicy.test.mjs
node --test tests/autofillFill.test.mjs
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/Test-AutofillDom.ps1 -Browser Chrome
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/Test-AutofillDom.ps1 -Browser Edge
```

Rust tests cover framing, malformed input, request correlation, safe errors,
response schema validation, and the built executable's real binary stdin/stdout
and clean EOF. The executable tests use invalid synthetic requests and never
connect to a personal vault. The PowerShell test mocks registry access and uses
temporary local files; it does not register Chrome or Edge. Checkpoint C's
separate-process pipe smoke procedure is in [the security notes](../security/autofill-v1.md).

A missing native host/invalid registration is a browser-side
`NATIVE_HOST_UNAVAILABLE` condition. A missing desktop pipe gives `app_not_running`
for status and `APP_NOT_RUNNING` for queries/fills; this means the endpoint is
unavailable, not proof that no desktop process exists. Busy, denied, timed-out,
or malformed pipe replies become `IPC_UNAVAILABLE` with fixed public text.
Unlock KeyNest in the desktop; no browser request can perform an unlock.

Do not use real personal credentials for acceptance. At G, execute the full
Chrome and Edge procedures in sections 34/35 of `KEYNEST_AUTOFILL_V1_SPEC.md` using
disposable credentials, including lock, mismatch, no submit, storage, network and
log checks. Those native extension/host acceptance procedures have not run through F.

## G disposable native acceptance tooling

Current results are in the [final G record](../security/autofill-v1-completion.md).
Chrome passed ten automated native checks and an absent-desktop check. Both
browsers passed user-performed manual acceptance. Edge's DevTools action crash
is an automation limitation reproduced with a static extension without KeyNest.

The runner uses experimental Chrome/Edge DevTools extension commands over
inherited pipes. It was exercised with Chromium 152 builds; those commands are
test-tool requirements, separate from the extension's minimum browser version.
It needs an interactive Windows session for desktop UI automation. It never
uses an HTTP or WebSocket server or the personal browser profile.

Close the normal KeyNest desktop yourself before starting the isolated desktop:
both intentionally use the real current-user/session Autofill endpoint. Use only
the fixed acceptance build/data directory below. From the repository root:

```powershell
npm.cmd run build
Push-Location src-tauri
try {
    $priorTauriConfig = $env:TAURI_CONFIG
    $env:TAURI_CONFIG = Get-Content -LiteralPath ../tests/fixtures/tauri.autofill-acceptance.json -Raw
    cargo build --locked --offline --features tauri/custom-protocol --bins --target-dir target/autofill-acceptance
    if ($LASTEXITCODE -ne 0) { throw 'Acceptance build failed.' }
} finally {
    $env:TAURI_CONFIG = $priorTauriConfig
    Pop-Location
}
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline seed_native_browser_acceptance_vault -- --ignored --nocapture
if ($LASTEXITCODE -ne 0) { throw 'Acceptance fixture creation failed.' }
$acceptanceProcessId = & ./scripts/Control-AutofillDesktop.ps1 -Action Start
try {
    node scripts/Test-AutofillBrowser.mjs Chrome $acceptanceProcessId
    # Currently blocked by Edge's Extensions.triggerAction crash:
    # node scripts/Test-AutofillBrowser.mjs Edge $acceptanceProcessId
} finally {
    ./scripts/Control-AutofillDesktop.ps1 -Action Stop -DesktopProcessId $acceptanceProcessId
}
node scripts/Test-AutofillBrowser.mjs Chrome $acceptanceProcessId --unavailable
```

The seeder refuses an existing acceptance data directory. Its passwords are
synthetic fixture constants and must never be used for personal credentials.
The native runner reports only pass labels, browser/extension versions and IDs,
and fixed product status text; field comparisons return booleans. Each run
temporarily registers its actual extension ID in the selected browser's HKCU
location using a manifest inside its temporary profile, then unregisters before
profile deletion. Existing foreign registrations cause refusal, not overwrite.
Keep the normal desktop closed until the absent-desktop check finishes.

After the acceptance desktop has exited, remove only the disposable data:

```powershell
foreach ($fixtureBase in @($env:APPDATA, $env:LOCALAPPDATA)) {
    $fixtureParent = [IO.Path]::GetFullPath($fixtureBase)
    $fixturePath = [IO.Path]::GetFullPath((Join-Path $fixtureParent 'com.eyy.keynest.autofill-acceptance'))
    if ((Split-Path -Parent $fixturePath) -ine $fixtureParent -or
        (Split-Path -Leaf $fixturePath) -cne 'com.eyy.keynest.autofill-acceptance') { throw 'Unsafe fixture path.' }
    if (Test-Path -LiteralPath $fixturePath) {
        $resolvedFixture = (Resolve-Path -LiteralPath $fixturePath).Path
        if ($resolvedFixture -ine $fixturePath -or
            ((Get-Item -LiteralPath $resolvedFixture).Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Unexpected fixture path.' }
        Remove-Item -LiteralPath $resolvedFixture -Recurse -Force
    }
}
```

On the recorded G run, the disposable desktop/data and temporary Chrome/Edge
registrations were cleaned up. The normal KeyNest application can be reopened.

## Human-assisted G acceptance

The follow-up [crash investigation](../security/autofill-g-edge-investigation.md)
isolated an Edge browser exit during the DevTools action command, including with
a minimal extension containing no KeyNest code. Direct popup rendering passes;
normal Edge toolbar/native operation subsequently passed manual acceptance. `--manual` runs a
visible disposable browser and waits for **your actual toolbar and Fill clicks**,
without that DevTools action command. It retains production permissions/CSP and
the normal native path. A human run passed initial checks but stopped at the desktop foreground-focus
guard. Guided manual interaction subsequently completed acceptance. This does
not establish that the entire --manual script passes on this setup.

For a new run, prepare the isolated acceptance executable and encrypted fake
vault and confirm the desktop state first. **Do not rerun the seeder against
that existing fixture.** For a later clean checkout, first use the isolated build
and seed instructions above. Never point this runner at the personal desktop.

1. Close normal KeyNest. From a PowerShell terminal in the repository root, run
   the block below. It starts the visibly separate acceptance desktop. The test
   helper unlocks only that fixture through its normal protected UI using its
   synthetic master password; it does not bypass authentication.

   ```powershell
   $acceptanceProcessId = powershell -NoProfile -ExecutionPolicy Bypass -File scripts/Control-AutofillDesktop.ps1 -Action Start -Visible
   if ($LASTEXITCODE -ne 0) { throw 'Acceptance desktop could not start.' }
   try {
       node scripts/Test-AutofillBrowser.mjs Edge $acceptanceProcessId --manual
       if ($LASTEXITCODE -ne 0) { throw 'Edge acceptance stopped; report the failed stage.' }
       node scripts/Test-AutofillBrowser.mjs Chrome $acceptanceProcessId --manual
       if ($LASTEXITCODE -ne 0) { throw 'Chrome acceptance stopped; report the failed stage.' }
   } finally {
       powershell -NoProfile -ExecutionPolicy Bypass -File scripts/Control-AutofillDesktop.ps1 -Action Stop -DesktopProcessId $acceptanceProcessId
   }
   ```

   The process-scoped PowerShell invocation does not change machine/user
   execution-policy settings. The runner refuses a conflicting host registration
   and registers the actual loaded ID only for the duration of each test.

2. Arrange the terminal and disposable browser so both are visible. At each
   **ACTION** prompt, click KeyNest Autofill in the browser's Extensions menu or
   toolbar. Keep the popup open. **Do not switch to the terminal or press Enter
   after opening the popup or clicking Fill**: the runner detects these actions.
   It allows two minutes for each requested click. Do not invoke the extension
   from a different browser window/profile or open its HTML directly.
3. Observe the locked message first, then the fake matching accounts after the
   fixture unlocks. At the Fill prompt, choose **Autofill fixture A**. Confirm
   the username and password fields populate and no submission/navigation occurs.
   The runner compares fake values using booleans and checks events/attributes;
   it does not print usernames, passwords or native messages. Confirm reopened
   Autofill reports locked after desktop lock; lookalike/different hosts offer no
   accounts; ambiguous/missing forms refuse. The fixture documents are supplied
   locally through inherited-pipe interception, with no web server or remote test
   endpoint. These are not real-site/TLS/framework acceptance tests.
4. At the final review pause, inspect KeyNest's **service-worker and popup**
   DevTools in this disposable profile. Check Network for KeyNest HTTP/WebSocket
   traffic; Console/extension errors for secrets; Application for localStorage,
   sessionStorage, IndexedDB and Cache Storage. `chrome.storage` must remain
   unavailable because no storage permission exists. Do not inspect or dump raw
   native-message objects, input values, key material or records into logs.
   Monitoring attached after startup is not a full trace: if you cannot verify
   startup traffic, native/browser stderr or another log source, report that
   check as **pending**, rather than passing it. Do not open personal browser or
   desktop data. Press Enter only when ready to clean up this review session.
5. Report Chrome and Edge separately: browser version/actual extension ID,
   load/popup/status/matching/Fill/no-submit/lock/domain results, storage/network/
   log review results, and any fixed failure stage or exit code. Send **pass,
   fail or pending**, with no credentials, usernames, payloads or secret-bearing
   screenshots. The final native script summary does not itself complete the
   human security review. After reporting, use the guarded fixture cleanup above
   when the acceptance desktop is stopped; normal KeyNest may be reopened.

Checkpoint G is complete; this procedure remains for future regression runs.
If Edge's real toolbar path crashes during a future run, stop that test
and report the stage; do not change production security checks to proceed.
