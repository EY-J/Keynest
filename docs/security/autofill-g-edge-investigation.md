# Checkpoint G: Edge action crash investigation

Final disposition 2026-09-08: **automation limitation; G and V1 complete.**
Both Edge and Chrome manual acceptance passed, as explicitly reported by the
user. Final regressions and security review passed. See the
[completion record](autofill-v1-completion.md). No V2 or production workaround.

The investigation below is a historical record from 2026-09-07. Its pending
manual handoff and incomplete statuses are superseded by that completion record.
The browser crash remains unresolved. It also reproduced with a static extension
without KeyNest integration; actual Edge toolbar/native acceptance later passed.
A later human-assisted script run separately hit its desktop foreground-focus
guard, not the browser crash. Guided manual interaction completed acceptance;
this is not a claim that the entire --manual script passed.

## What actually crashed

The launched **`msedge.exe` browser process** exited with decimal `3221225477`
(`0xc0000005`) while `Extensions.triggerAction` was pending. Node caught the
disconnection, printed metadata, removed the disposable profile and exited with
ordinary test-failure code 1. Node did not suffer that access violation.

Two reproduced Edge cases:

| Probe | Browser PID | Node PID | Last completed command | Result |
| --- | --- | --- | --- | --- |
| Unchanged KeyNest extension | 19564 | 12000 | `Target.getTargets` | `msedge.exe` exit `0xc0000005` during `Extensions.triggerAction` |
| Minimal static popup, no KeyNest code | 9136 | 14000 | `Target.getTargets` | Same Edge exit during the same command |

The minimal fixture has only an MV3 manifest and static HTML popup: no worker,
script, permission, native host, vault or named pipe. No KeyNest desktop/host
process was running during these probes. There were no KeyNest host registrations
in HKCU **or HKLM**, in either 32-bit or 64-bit registry views, for either browser.
Therefore a stale manifest, executable path, extension ID, native-host crash or
named-pipe request does not explain these isolated crashes.

The failure is localized to Edge's handling of the test action command under
these launch conditions. **The internal root cause is not known.** No matching
fresh Application Error/Windows Error Reporting event or Edge dump was available.
Application logging was enabled; unrelated events existed. Edge stderr contained
zero bytes in the reproductions. No faulting module, offset or stack was obtained.
Unrelated historical Node crash artifacts were not attributed to this incident.

This is not a claim that only Node/test code crashes, or that manual Edge/native
operation has passed: **Edge itself exits**, and normal toolbar/native operation
still requires human verification. A full Chrome regression attempt also had an
intermittent browser exit with the same code; its last command was not captured
in that attempt. The next full Chrome run and absent-desktop run passed. The
available evidence does not establish that this failure is exclusive to Edge.

## Progressive isolation and Chrome comparison

| Stage | Edge 152.0.4191.66 | Chrome 152.0.7977.82 |
| --- | --- | --- |
| Fresh disposable browser launch | Passed | Passed |
| Load unchanged unpacked extension | Passed | Passed |
| Actual KeyNest extension ID | `ohjdijkmicaoalcbfgoajkgfnipbefai` | Same ID |
| Direct popup document rendering | Passed; browser remains responsive | Action popup passed |
| Invoke `Extensions.triggerAction` | Reproduced browser access violation | Minimal KeyNest action probe passed |
| Native status/query/Fill through real action | Blocked before this stage | Final regression passed |
| Actual document-ID injection and fake values | Not reached | Passed |

Direct popup navigation tests rendering only. It does not grant activeTab and
does not count as action, Native Messaging, query, Fill or manual acceptance.
No debug-origin or permission bypass was used to advance the Edge test.

Both runners load the same repository manifest/CSP and use the same launch flags
apart from browser executable and optional visible/headless mode. Each run has a
new temporary profile, with no personal browser profile. Native acceptance uses
the same fixed acceptance host executable in
`src-tauri/target/autofill-acceptance/debug/keynest-native-host.exe`; the selected
browser registration is generated from the **actual loaded ID**, with exact
`chrome-extension://<ID>/` origins. Its manifest lives inside that run's temporary
profile. Chrome and Edge have separate standard HKCU registration locations.
The shared current-user/session named pipe is unchanged. The crashing isolation
probes do not register or launch the native host at all.

Reproduce from the repository root:

```powershell
node scripts/probe-autofill-browser.mjs Edge --stage=launch
node scripts/probe-autofill-browser.mjs Edge --stage=load
node scripts/probe-autofill-browser.mjs Edge --stage=popup
node scripts/probe-autofill-browser.mjs Edge --stage=action
node scripts/probe-autofill-browser.mjs Edge --stage=action --minimal
node scripts/probe-autofill-browser.mjs Chrome --stage=action
```

These use inherited DevTools pipes, not TCP/HTTP/WebSocket servers. The static
minimal fixture is generated inside its disposable profile and removed afterward.
`--headed` is available; the previous G run also reproduced Edge's action crash
in that mode. Raw stderr, native messages, usernames and passwords are never
printed by the diagnostic probe. Only fixed stage labels, executable name, PIDs,
numeric stderr marker/byte counts and exit/command metadata are emitted.

## Changes and validation this run

- `scripts/autofill-browser-driver.mjs`: bounded diagnostic metadata; no raw stderr
  retention/output. Browser and Node exit failures can now be distinguished.
- `scripts/probe-autofill-browser.mjs`: separate launch/load/direct-popup/action
  stages and a minimal extension control case, with explicit cleanup evidence.
- `scripts/Test-AutofillBrowser.mjs`: diagnostic metadata and a **human-assisted**
  `--manual` mode that waits for actual toolbar/Fill clicks. It never calls
  `Extensions.triggerAction` in that mode. A headed popup is reopened after lock;
  that check is correctly described as locked refusal, not a stale open-popup Fill.
- `scripts/Control-AutofillDesktop.ps1`: bounded readiness wait for first-run
  WebView startup, plus an opt-in visible window for human testing.
- This report, the development setup guide and status links were updated.

No extension/Rust production code, dependencies, permissions, CSP, validation,
cryptography, recovery, password or lock behavior changed in this run.

Final Chrome run: ten native checks passed, plus one separate absent-desktop
check. The preceding startup-selector failure was corrected in the test helper;
the subsequent intermittent Chrome access violation is recorded above rather
than hidden by the successful rerun. Successful full-run browser stderr had one
ERROR marker (195 bytes); raw text was not retained, so it is **not** declared
audited or harmless. Attached extension Runtime/Log events contained no fixture
secret markers, within the previously documented monitoring limits.

All **88 JavaScript tests**, **two actual native-host stdio integration tests**,
the isolated fixture seeder, Rust formatting, changed JS syntax and PowerShell
parsing passed. Mocked host-registration tests passed using their documented
process-scoped execution-policy invocation after the default invocation was
blocked by script execution policy. The earlier 255-test Rust suite, frontend
build and Clippy results remain prior evidence; those full commands were not
repeated in this test-only investigation. The new human-assisted branch has been
syntax-checked, **not run by a human or accepted yet**.

## Security review status

Preliminary source inspection and the current policy tests confirm:

- Extension permissions remain exactly activeTab, scripting and nativeMessaging;
  no host permissions, `<all_urls>`, remote imports, network primitives, storage
  primitives, logging calls or automatic submission calls were found.
- Native host/pipe production code adds no network listener/client or plaintext
  file persistence. The seeder/interactive fixture's fixed diagnostic output is
  test-only. The shared production panic hook emits only fixed public text.
- Native framing remains 1..65536 bytes with strict schemas/correlation, fixed
  errors and zeroizing buffers. The executable tests verified protocol-only
  stdout, empty stderr, malformed bounds and clean EOF.
- Current registration checks found no stale entries; the registration helper's
  exact-origin/ownership behavior passed its mocked tests.

This is **not the final cross-browser network/storage/log acceptance**. Edge
runtime evidence and the user's Chrome/Edge visual/manual review remain pending.
Runtime monitoring begins after target attachment and does not inspect all OS,
browser or native diagnostic sources. Storage inspection and static absence of
storage calls do not prove JS memory erasure. No real secrets should be used or
copied into a result report.

At handoff, the acceptance desktop is stopped, both temporary registrations are
absent and no disposable browser profile remains. The separate **encrypted fake
vault and acceptance build are intentionally retained** for the user's manual
run; no personal vault was accessed. Follow the
[manual procedure](../release/browser-autofill-dev-setup.md#human-assisted-g-acceptance)
and report pass/fail/pending before G can be marked complete.
