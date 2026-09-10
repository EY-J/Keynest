# KeyNest Autofill V2 — Checkpoint F2 cross-origin iframe security review

**Status:** Review complete. Cross-origin iframe filling is **not approved and is not implemented** by this checkpoint.

## Scope and current boundary

KeyNest currently requests only `activeTab`, `scripting`, and `nativeMessaging`. Checkpoint F1 permits filling a visible login form in a same-origin frame after an explicit extension action. A cross-origin frame remains outside that boundary and must not receive credentials.

This review preserves all Autofill V1 invariants: local-only native messaging, exact-host authorization, explicit user action, no password storage in extension state, no automatic submission or navigation, no site-specific selectors, and no broad host permission.

## Permission analysis

`activeTab` is not sufficient for generic cross-origin iframe filling. Chrome grants temporary host access for the active tab's **main-frame origin** after a user gesture. Injecting into a frame whose origin differs from the main frame requires host access to that frame's origin in addition to the `scripting` permission.

The available permission approaches all materially change the current security posture:

| Approach | Consequence | Review outcome |
| --- | --- | --- |
| Static `host_permissions` | Persistent access to every declared frame origin and potentially new install/update warnings | Reject for generic support |
| `optional_host_permissions` requested at runtime | Gives the user a consent prompt and can limit each grant, but arbitrary identity-provider support requires declaring a broad eligible pattern such as `https://*/*` | Safer than static access, but still conflicts with KeyNest's narrow-permission invariant |
| Finite provider allowlist | Avoids a universal pattern but becomes brittle, provider-specific behavior | Reject; site-specific handling is out of scope |

`<all_urls>` must not be added. Declaring `https://*/*` as an optional host permission would avoid that literal but would still establish a broad permission capability and therefore does not satisfy the intent of the current invariant.

A safe implementation would also need browser-authoritative frame lifecycle data. Chrome's `webNavigation` API can return the URLs, frame IDs, parent relationships, and document IDs for a tab's frames, but using it would add the `webNavigation` permission. Self-reported DOM data alone is not an adequate replacement.

References: [Chrome `activeTab`](https://developer.chrome.com/docs/extensions/develop/concepts/activeTab), [Chrome scripting API](https://developer.chrome.com/docs/extensions/reference/api/scripting), [Chrome permissions API](https://developer.chrome.com/docs/extensions/reference/api/permissions), and [Chrome webNavigation API](https://developer.chrome.com/docs/extensions/reference/api/webNavigation).

## Required frame-origin validation

If cross-origin support were ever approved, authorization would have to be bound to the actual browser document, not merely to an iframe element found in the top page.

At minimum, KeyNest would need to:

1. Obtain the top document URL and the target frame URL, frame ID, document ID, parent chain, and lifecycle state from browser-controlled extension APIs.
2. Require HTTPS and normalize both hosts using the same exact-host rules used by the native host. Wildcards, substring matches, inherited origins, opaque origins, and redirects must fail closed.
3. Reject DOM-supplied claims such as `iframe.src`, `postMessage` payloads, dataset values, or a frame's uncorroborated report as authorization evidence.
4. Revalidate the active top document and target frame immediately before the native fill request and again before secret injection. Any navigation or document-ID change must invalidate the authorization.
5. Inject only into the selected document ID, not all frames, and require the target script to confirm its own expected HTTPS origin before accepting a secret.
6. Preserve the existing visible/unique-candidate checks. Hidden, zero-area, detached, stale, or ambiguous frames must fail closed.

Cross-origin same-origin-policy restrictions mean code inside the frame cannot independently validate the top page. The extension worker must bind the browser-observed top document and frame document together and carry that binding through the fill transaction.

## Credential authorization model

Neither top-level-host-only nor frame-host-only authorization is sufficient.

- Top-level only is unsafe because it authorizes sending a credential to an unrelated embedded recipient.
- Frame only is unsafe because a malicious or lookalike top-level page could embed a legitimate authentication origin and induce credential release in an attacker-controlled context.

The minimum defensible model is an explicit credential authorization tuple:

```text
(exact top-level host, exact frame host)
```

For delivery, the target should additionally be pinned to the exact HTTPS origin and current document ID. The user would authorize that pair through trusted KeyNest desktop UI; a web page or content script must never create or broaden it.

The existing `allowed_login_hosts` field must not automatically authorize cross-origin recipients. It represents alternate top-level login hosts under the present model, not a delegation from any top-level page to an embedded credential recipient.

Implementing the tuple would require a versioned native-message protocol change so the extension sends both browser-observed URLs and the native host independently validates the stored top/frame authorization immediately before returning a password. This is not implemented in F2.

## Phishing and credential-confusion risks

Cross-origin filling materially expands who can receive a vault secret and in what context:

- An attacker-controlled page can embed a legitimate sign-in origin and surround it with deceptive instructions or overlays. Frame-host-only matching would treat the legitimate frame as sufficient even though the relying page is hostile.
- A compromised legitimate top-level site can add a hidden, tiny, covered, or visually misleading authentication frame. Geometry and visibility checks reduce risk but cannot reliably prove that a cross-origin frame is unobscured or that the user understands the recipient.
- The credential may belong to the top-level relying party, the embedded identity provider, or neither. Inferring ownership from DOM structure or field names creates credential-confusion risk.
- Redirects and frame replacement can swap the inspected document before injection unless authorization is bound to the current document ID and revalidated at the last possible moment.
- Runtime host-consent prompts may train users to approve broad site access without understanding that a password can be delivered to a different origin than the address bar displays.

An explicit top/frame authorization pair limits these attacks but does not remove the UI-redress, compromised-page, or consent-confusion risks.

## Increased extension attack surface

Cross-origin support would expand the trusted computing base beyond the current main-origin boundary. It would add permission-grant state, frame enumeration and navigation races, multi-origin authorization data, protocol fields, native validation paths, target-document selection, revocation behavior, and more renderer processes that may temporarily handle a secret.

Every added host grant also increases the set of pages on which an extension compromise could run code. New authorization and lifecycle branches create additional fail-open opportunities, especially during redirects, back/forward-cache restoration, prerender activation, and frame replacement. These changes would require dedicated JavaScript, browser-integration, protocol, and Rust regression tests plus a separate threat-model review.

## Browser-store implications

Chrome Web Store policy requires the narrowest permissions needed for the extension's current features. Adding static host access, a broad optional host pattern, or `webNavigation` would require updated permission justifications and may introduce new user-facing permission prompts or review scrutiny.

Authentication information, form data, website content, and browsing activity are treated as user data even when processing is local. Any cross-origin implementation would therefore require the privacy policy, store listing, in-product disclosures, and consent UX to accurately explain:

- which top-level and frame origins are inspected;
- when a password can be sent to an origin different from the address-bar origin;
- whether frame URLs or grants are retained;
- how authorization is revoked; and
- that no browsing or credential data is transmitted to KeyNest or another remote service.

References: [Chrome Web Store Program Policies](https://developer.chrome.com/docs/webstore/program-policies/policies), [User Data FAQ](https://developer.chrome.com/docs/webstore/program-policies/user-data-faq), and [Chrome extension user privacy guidance](https://developer.chrome.com/docs/extensions/develop/security-privacy/user-privacy).

## Decision

Do **not** implement generic cross-origin iframe filling under the current Autofill V2 permission and authorization model.

The only technically defensible future design would combine all of the following:

- an explicit runtime grant for the exact HTTPS frame origin;
- explicit per-credential authorization of the exact top-level-host/frame-host pair;
- browser-authoritative frame and document identity;
- native-host revalidation of both origins;
- document-pinned, last-moment injection;
- clear user consent, revocation, and store/privacy disclosures; and
- full extension, browser, protocol, and Rust security regressions.

Supporting arbitrary providers would still require a broad optional host declaration, which conflicts with KeyNest's current no-broad-host-permission posture. Unless that invariant is deliberately revised after explicit approval, the correct behavior is to keep cross-origin iframe forms unsupported and fail closed.

No extension permission, source code, native protocol, storage model, form behavior, or submission behavior was changed by this checkpoint.
