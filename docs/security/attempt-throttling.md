# Unlock and recovery throttling (roadmap Phase 4)

Rust owns one shared in-memory failure counter for Master Password unlock and
Recovery Key attempts. Switching UI flows does not obtain another attempt budget.
The existing counter is retained; its former 1/2/4/5-second schedule has been
replaced with this bounded policy:

| Completed failed attempts | Delay before another attempt |
| --- | --- |
| 1–3 | No additional delay beyond verification cost |
| 4 | 2 seconds |
| 5 | 5 seconds |
| 6 | 10 seconds |
| 7 and later | 30 seconds |

Malformed recovery keys count as failures but are rejected without running a KDF.
Invalid new-password policy inputs do not attempt recovery authentication and do
not change the counter. Missing/damaged/unconfigured profiles retain the existing
safe public error categories.

## Enforcement and timing

Both operations acquire the auth mutex, sample a monotonic clock, and check the
remaining delay before verifying secrets. Following failed verification they
sample the clock again and record the failure time. KDF work and time queued for
locks therefore do not consume the new cooldown. The triggering fourth/later
failure returns `throttled` and `retryAfterMs` immediately, so UI feedback does not
need another request. Blocked attempts do not increment the counter or extend
the delay, including attempts with valid credentials.

The failure count saturates at `u32::MAX`. Remaining time uses saturating duration
subtraction, avoiding deadline addition overflow, and rounds up to milliseconds.
Delay never exceeds 30 seconds. The service does not sleep, spawn a delay worker,
or hold `SecurityOperationGate` while waiting for a cooldown. Existing command
lock ordering remains operation gate then auth mutex. Normal KDF work still runs
under the existing serialization boundary.

Successful unlock resets the counter and timestamp. Successful recovery resets
them only after the replacement profile commits. Manual lock and recovery-status
queries do not reset failures. Invalid reset confirmation and authenticated reset
while locked do not bypass authorization or clear the cooldown. An explicitly
confirmed destructive reset retains existing semantics: it erases the vault,
then resets transient state; it never grants access to the previous secrets.

## Frontend

Unlock and recovery honor the returned bounded delay with disabled submission and
`Please wait…` feedback. Recovery keeps its timer when the dialog closes/reopens
within the mounted unlock screen. Both clear submitted secret fields on error.
Unlock also guards duplicate pending submissions. Timers are cleaned up on
unmount and capped at 30,000 ms; Rust remains authoritative if UI state is lost,
modified, or a direct command is invoked. No secrets enter timer state.

## Validation and limits

Clock-injected tests cover both schedules, exact expiry, eventual valid access,
success resets, mixed-flow attempts, invalid resets, saturated counters,
submillisecond rounding, unchanged profile/vault bytes, and post-KDF timing.
A command-level concurrent test confirms throttled calls return without sleeping
under the gate and preserve locked state and reset authorization.
Mocked frontend tests cover both timers, clearing, blocked retries, expiry,
recovery-dialog reopening, and excessively large delay metadata.

This is local online-attempt friction, not offline brute-force protection.
Restarting the application resets transient counters. An attacker able to copy
the encrypted profile can guess outside KeyNest; data-at-rest defense remains
the KDF, encryption, and secret strength. No permanent lockout, failure-triggered
deletion, persisted throttle metadata, or guarantee against privileged malware
is introduced. A malicious local caller can still cause inconvenience by making
new failures after each cooldown. Manual desktop UI testing was not performed.
