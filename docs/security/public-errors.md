# Public errors (Phase 25)

Rust retains typed internal errors. `PublicIpcError` uses static codes/messages and optional numeric cooldown only; it never serializes underlying SQL/I/O/crypto errors. Wrong password/recovery, damaged or unsupported data, locked state and storage failures remain distinct where the UI needs them. AEAD failure alone cannot distinguish a wrong wrapping secret from tampered wrapper ciphertext; do not invent a verification oracle.

All three frontend clients now select messages from `shared/security/publicErrors.ts` using known codes, ignoring transport-provided text and stacks. Unknown codes produce a generic error; cooldowns are finite and capped. Tests enumerate Rust public codes to prevent drift and inject sentinel paths/secrets into error text. Existing Rust wrong-password/recovery/tampered-data/storage and error-taxonomy tests remain authoritative for backend behavior. No authorization or storage behavior changes.
