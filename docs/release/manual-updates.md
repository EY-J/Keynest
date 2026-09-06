# Manual signed updates (Phase 27)

KeyNest uses manual signed releases. There is no in-app update check, background download, updater plugin, new network permission or automatic installation. Normal local vault use does not depend on a release server. Current production distribution remains blocked on Phase 26's real signing acceptance and outstanding native security checks.

## Publisher procedure

1. Build from the reviewed source revision with the publisher-owned signing configuration described in `windows-signing.md`. Retain the source revision, application version, supported Windows versions, format compatibility and release notes.
2. Verify the application executable and each installer. Run `scripts/Verify-Release.ps1` against the intended publisher fingerprint and SHA-256 hash, then inspect SDK signature verification and actual installation on disposable profiles. Publish artifacts, their hashes and the expected signer fingerprint through an authenticated official release channel. No release channel is invented or configured here.
3. Certificate rotation needs a trusted announcement identifying the new publisher fingerprint; never accept a replacement fingerprint only because an unverified download says to.
4. Release qualification must demonstrate old-profile unlock/recovery and vault reads, unchanged data on rejected/failed upgrades, and normal operation without internet. Do not publish as production before real signature/installer/upgrade acceptance. Signing and optional certificate verification may use network services outside the vault application.

## User procedure

1. Finish writes, lock and exit KeyNest. Keep a separate consistent copy of the complete encrypted application-data directory while KeyNest is closed, plus the independently stored Recovery Key. Never copy only `vault.enc` without its matching `profile.json`. Protect backups as sensitive encrypted data; do not put plaintext credentials in a backup. This is operational guidance, not a new export/import feature.
2. Obtain the intended signed release and expected publisher fingerprint/checksum through the authenticated official channel. Run the read-only verification command from `windows-signing.md`; reject missing/invalid trust, tampering, wrong publisher or missing timestamp. A checksum from the same untrusted download is not sufficient proof.
3. Install only after verification, from a location other applications cannot alter between verification and installation. The verifier deliberately never runs the file; it cannot protect against local malware or post-verification replacement. Do not disable Windows trust checks because a release is unsigned or validation fails. If offline certificate trust cannot be established, postpone installation; the existing vault remains usable offline.
4. Upgrade in place, preserving app identifier `com.eyy.keynest` and its data directory. Do not uninstall/reset/delete local data as an upgrade step. If installation fails, stop and preserve the original data and encrypted copy for recovery; do not retry by resetting the vault.

## Compatibility and rollback

This queue does not change profile or vault formats. The current implementation reads profile versions 1 (legacy) and 2 (recovery), and vault schema/record version 1. Unknown profile/schema versions fail closed rather than resetting user data. Tests verify reopening supported vaults and rejecting future versions without rewriting the encrypted file; existing profile compatibility and atomic replacement tests remain in the Rust suite.

Future format changes require explicit versioning, backward-reader/migration tests and atomic failure handling before release. An older binary is **not** assumed able to read a newer format. Roll back only to a verified compatible version, or restore the entire matching pre-upgrade encrypted data set while all KeyNest processes are closed, retaining the failed/newer copy separately. A rollback loses subsequent edits and may restore an older password/recovery wrapper; use the matching old credentials. Do not mix profile and vault files from different snapshots. Do not automate downgrade or data deletion.

Actual installer/OS failure behavior is a manual release gate, not proven by unit tests. Automated tests cover verifier policy and unchanged current storage, not a certificate-backed installer or future format migration. No automatic updater tests apply because no updater is installed; adding one requires separate authorization and signed-update verification design.
