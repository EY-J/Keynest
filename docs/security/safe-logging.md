# Safe logging (Phase 24)

No plaintext passwords, recovery/key material, decrypted records (including metadata), clipboard contents, request payloads or serialized errors may be logged. Use fixed diagnostic codes/messages, never partial masking. No remote telemetry, log upload or application logging plugin is configured.

Audit: application Rust/React sources contain no console, println, dbg, log or tracing calls; the sole diagnostic is now the fixed panic message in `diagnostics.rs`. Existing startup/thread-spawn expect calls cannot print their underlying errors through the application's replacement panic hook. The hook omits payload, location and backtrace in development and production. This intentionally sacrifices detailed panic diagnostics; reproduce failures with disposable fixtures in tests. Do not reinstall the default hook or log IPC arguments.

Vault keys and plaintext crypto payloads have no Debug; credential inputs/details and recovery responses redact their sensitive fields. Summary Debug now also redacts decrypted metadata. Internal typed storage errors are not diagnostics and must not be dumped. Rust test assertions/unwraps use disposable fixtures only, never a real vault.

Sentinel tests cover panic diagnostic output, decrypted summaries, credential/recovery Debug and clipboard/public error text. No application file logging is added. Third-party native crash reporting, OS dumps and malware are outside this guarantee; the hook is not memory protection and does not suppress independent dependency output. Review logging changes and dependencies before introducing new diagnostics.
