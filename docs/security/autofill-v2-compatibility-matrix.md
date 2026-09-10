# Autofill V2 Checkpoint K synthetic compatibility matrix

Date: 2026-09-08

Status: Checkpoint K automated fixtures complete. This document records synthetic
regressions only; it is not a real-site support claim.

All DOM fixtures are small, manually constructed test cases using generic semantic
HTML. They do not copy production site HTML, and provider names appear only in test
descriptions. No provider-specific selector or behavior was added to extension code.

## Automated matrix

| Required scenario | Synthetic regression | Expected result |
| --- | --- | --- |
| Facebook combined login | `Facebook-like React wrappers accept username webauthn and leave unrelated fields untouched` | Combined username/password fill; unrelated search stays unchanged; no submit |
| GitHub combined login | `GitHub-style combined login fills semantic username and password fields only` | Combined username/password fill; no click or submit |
| Instagram SPA-like login | `Instagram-style SPA login mounts late and is filled only on a fresh explicit action` | Bounded inspection detects the late form without filling; a fresh explicit Fill writes both fields; no submit |
| Discord + QR alternative | `Discord-style login ignores a non-form QR alternative` | Combined login fills; QR alternative is untouched |
| Google username-only | `Google-style username step fills only the semantic account field` | Username-only stage; no Next click or submit |
| Google password-only | `Google-style password step fills only the current password field` | Password-only stage; displayed account label is untouched |
| Microsoft username-only | `Microsoft-style username step fills only the semantic email field` | Username-only stage; sign-in alternative is not activated |
| Microsoft password-only | `Microsoft-style password step ignores the displayed account label` | Password-only stage; displayed account label is untouched |
| Amazon staged login | `Amazon-style staged login requires a fresh explicit fill for each DOM stage` | Username and password stages each require a separate fresh explicit Fill; neither stage submits |
| LinkedIn modal login | `LinkedIn-style modal wins over a hidden background login` | Visible modal fills; hidden background fields remain empty |
| Generic SSO | `generic SSO form fills a strongly associated combined login` | Strongly associated username/password fill; tenant field remains unchanged |
| Open Shadow DOM | `username and password in one open shadow root fill` plus bounded/closed-root regressions | Combined fields in an open root fill; closed roots remain unsupported; traversal is bounded |
| Same-origin iframe | `same-origin iframe login fills without submitting` | Visible same-origin frame fills; changed-origin and hidden-frame attempts fail; no submit |
| OTP-only | `generic six-digit numeric input is a challenge and cannot receive credentials` plus grouped/semantic OTP regressions | Challenge stage; no credential field receives a value |
| Passkey-only | `passkey-only UI is recognized without interaction` | Passwordless stage; no click, keypress, or submit |
| Login + signup visible together | `visible signup new-password fields do not compete with a current-password login` | Current-password login fills; signup and confirmation fields remain empty |
| Hidden responsive duplicate form | `hidden responsive duplicate login is ignored` | Visible login fills; hidden duplicate remains empty |
| Lookalike-domain rejection | Rust `suffix_lookalike_and_unlisted_subdomains_never_match` and request-fill mismatch regressions; JavaScript parsed-host regression | One leading `www.` is the only automatic hostname equivalence; suffix lookalikes, deceptive URLs, Unicode lookalikes, and other unlisted subdomains do not match; requestFill returns `DOMAIN_MISMATCH` |

## Verification results

- Focused Autofill JavaScript/extension suite: 43 passed, 0 failed.
- Complete JavaScript suite: 104 passed, 0 failed.
- Headless Chrome DOM matrix: 74 passed, 0 failed.
- Headless Edge DOM matrix: 74 passed, 0 failed.
- Relevant Rust autofill/native-pipe regressions: 50 passed, 0 failed, 2 ignored
  interactive/fixture helpers (207 unrelated tests filtered out).
- `npm run build`: passed. Vite emitted its existing advisory that one minified
  chunk is larger than 500 kB.

## Security boundary represented by the matrix

- Every write still begins with the user's explicit popup Fill action.
- No fixture expects or permits automatic submit, Next/Sign In activation, or an
  Enter keypress.
- Canonical HTTPS whole-host matching and independent requestFill revalidation
  remain authoritative in Rust. Canonicalization removes exactly one leading
  `www.` label; alternate hosts remain explicit entries only.
- Open Shadow DOM and same-origin iframe coverage does not enable closed-root or
  cross-origin iframe access.
- OTP, passkey, CAPTCHA, and other challenge states are recognition-only.
- Extension permissions remain `activeTab`, `scripting`, and `nativeMessaging`.
- No cloud/API path, localhost HTTP/WebSocket server, extension storage, secret
  logging, or crypto/recovery/lock behavior is introduced by Checkpoint K.

## Real-site status

No Checkpoint K real-site manual tests were run. Facebook, GitHub, Instagram,
Discord, Google, Microsoft/Outlook, Amazon, and LinkedIn therefore remain
**not manually verified for Autofill V2 compatibility**. Existing Autofill V1
manual acceptance evidence is separate and must not be treated as V2 provider
verification. The specification's real-site matrix is the next distinct activity
and requires disposable/test accounts where possible.
