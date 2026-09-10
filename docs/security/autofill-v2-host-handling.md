# Autofill V2 hostname handling

The normal Add Credential and Edit Credential forms expose only the optional
Website field. The internal `allowed_login_hosts` allowlist remains part of the
Rust model, encrypted record payload, IPC types, and Autofill matcher, but it is
not shown as a technical comma-separated field in the normal credential UI.

New credentials submit an empty allowlist. Editing an existing credential keeps
its initially loaded allowlist unchanged, including when the Website, username,
password, name, or tags are edited. This preserves previously approved alternate
login hosts without silently creating or deleting trust relationships.

## Canonical comparison

Autofill parses both the current HTTPS URL and each saved Website/allowed host,
then applies one shared comparison-host transformation:

1. Remove one trailing dot, when present.
2. Convert the hostname to lowercase.
3. Remove exactly one leading `www.` label.
4. Compare the entire remaining hostname for equality.

Consequently, `example.com` and `www.example.com` are equivalent. The rule does
not remove other subdomains: `login.example.com` remains distinct from
`example.com`, while `www.login.example.com` canonicalizes to
`login.example.com`. It uses no substring, suffix, fuzzy, public-suffix, or
registrable-domain matching.

The same matcher is called independently by `queryMatches` and `requestFill`.
A successful query never authorizes a later fill; requestFill reloads the selected
record, rechecks lock state, reparses the current URL, and compares the current
canonical host again.

Explicit alternate hosts continue to work as stored exact allowlist entries,
subject only to the same one-label `www.` canonicalization. No relationship is
inferred between unrelated domains. For example, `gmail.com` does not authorize
`accounts.google.com` unless `accounts.google.com` is already stored in the
credential's internal allowlist.

## Contextual exact-host approval

Cross-domain login hosts require an explicit user decision because KeyNest does
not infer organizational relationships. A credential saved for `gmail.com`, for
example, does not match `accounts.google.com` merely because both names contain
`google`. Provider mappings, suffix matching, fuzzy matching, wildcards, redirect
learning, and automatic subdomain approval are not used.

When normal matching produces no credential, the extension can offer **Review in
KeyNest**. That explicit action sends a versioned `requestHostApproval` message
containing the current parsed HTTPS page URL and a request ID. It contains no
credential ID, password, or vault listing. The native host forwards the bounded
message through the existing Windows named pipe; no network transport or second
IPC mechanism is involved.

The desktop reparses the URL with the Autofill HTTPS parser, applies the existing
lowercase/trailing-dot/one-leading-`www.` canonicalization, and retains one
short-lived pending request in memory only. The unlocked desktop modal displays
the exact requested hostname and password-free credential summaries. It never
guesses or preselects a credential. The confirmation view displays both the
saved Website and requested login host and warns the user to approve only a host
they recognize and trust.

Only **Allow Host** performs a Rust vault update. Rust reloads the selected
credential while holding the shared security-operation gate, appends the exact
canonical hostname to the encrypted `allowed_login_hosts` list, and uses the
existing atomic vault update. Existing hosts and the primary Website are
preserved. An already equivalent entry is an idempotent success and is not
duplicated or rewritten. Cancel, close X, Escape, and backdrop dismissal all use
the cancel operation, clear the pending request, and do not mutate the vault.

After approval, the extension does not receive a credential and does not fill,
navigate, or submit. The user returns to the extension and clicks **Retry**, which
runs the normal `queryMatches` flow again. A later explicit staged Fill still
causes `requestFill` to reload the selected record and independently revalidate
the current host against the primary Website and exact allowlist. Normal Add/Edit
continues to hide the technical allowlist and preserves it across edits.
