# Phase 8: clipboard ownership audit

The existing Rust service meets the requested model; no clipboard architecture rewrite was needed.

- Each successful copy owns a zeroizing in-memory value and checked monotonic generation. A later copy replaces and drops the old value. Timer closures retain a generation/service handle, not a separate plaintext copy.
- Expiry checks the generation before accessing the clipboard. Stale jobs do nothing; changing the setting applies to subsequent copies. Old threads may sleep to their original deadline, but cannot clear newer generations.
- Manual/automatic lock takes and discards ownership, then compares before clearing. Failed reads/clears never cause a blind clear, and ownership drops even on errors.
- Windows uses `OpenClipboard` through compare/`EmptyClipboard`, preventing an external writer from changing contents between those steps. Clipboard/global-memory handles have scope guards on failure paths.
- Only explicit Copy Recovery Key calls the same protected service. Creating/regenerating/displaying a key does not copy it automatically. Password copies use record ID, not plaintext returned to React.
- Nothing is persisted or logged. Tests use fixture clipboard ports, not the user's real clipboard.

Validation covers normal expiry, replacement text, repeated scheduled copies, lock invalidation, generation exhaustion, errors, native guard balancing, compare/write races and bounded exit cleanup. New tests exercise the production scheduling path with a controlled scheduler.

Limitations: ownership is exact text equality, not proof of which app last copied identical text. An externally copied identical secret is therefore eligible for clearing. Clipboard history/managers or synced/external copies are not erased. OS contention/failure can leave the clipboard value behind. Non-Windows default ports lack the Windows atomic compare-and-clear guarantee and need platform-specific hardening before equivalent claims.
