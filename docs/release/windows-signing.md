# Windows release signing (Phase 26)

Implementation prepared; production signing acceptance pending certificate acquisition. No certificate, private key, signing identity or signed release is supplied by this repository.

## Build configuration

Use a publisher-owned Windows code-signing credential backed by its provider's hardware/managed signing service. Tauri supports a custom `bundle.windows.signCommand` for providers; use its documented integration without putting authentication secrets in arguments. A compatible Windows certificate-store provider can use `certificateThumbprint`, `digestAlgorithm`, `timestampUrl` and `tsp`. The bundled CLI schema was checked for these fields. [Tauri Windows signing](https://v2.tauri.app/distribute/sign/windows/).

Copy `docs/release/tauri.signing.example.json` to the ignored `src-tauri/tauri.signing.local.json`, fill the real certificate thumbprint and provider RFC3161 URL, or replace the Windows section with the provider's custom signing configuration. The example is deliberately unusable until configured. Never change ordinary development config to require a credential.

From the repository root, build with:

```powershell
npm.cmd run tauri -- build --config src-tauri/tauri.signing.local.json
```

Keep keys, PFX/P12 exports, passwords and tokens outside the repo. Ignore rules are accident prevention, not secret protection. Future CI must use protected secret stores or short-lived signing identity, restrict release approval, and avoid echoing credentials. No signing CI or provider account is provisioned here. Normal `npm.cmd run tauri -- dev` and unsigned local builds remain unchanged.

## Verify before publishing

Verify both the application executable and every installer, not just the outer package. Microsoft's SDK supports Authenticode verification with `signtool verify /pa /all /v <artifact>`. Independently confirm the expected publisher and timestamp; signature validity alone does not identify KeyNest. [Microsoft SignTool](https://learn.microsoft.com/en-us/windows/win32/seccrypto/signtool).

For a read-only local gate, use:

```powershell
.\scripts\Verify-Release.ps1 -ArtifactPath 'C:\Releases\KeyNest-setup.exe' -ExpectedSignerThumbprint '<40-hex trusted publisher certificate fingerprint>' -ExpectedSha256 '<64-hex published release checksum>'
```

The script fails closed on missing/invalid signature, wrong publisher/hash or missing timestamp. Obtain expected values through an independently authenticated release channel, not from the downloaded file itself. Hashes alone are not proof of authenticity. Verification may require Windows certificate-chain/revocation services; inability to establish trust is not permission to bypass it. KeyNest vault operation itself remains offline.

## Release classification / checklist

- Development: unsigned, local use only; not a production distribution.
- Unsigned test build: explicitly labeled testing only, disposable fixture data.
- Signed production: publisher credential configured, app and installer signatures/timestamp independently verified, checksum recorded, install/upgrade tested on supported Windows with disposable profiles, security/manual blockers closed, approved release published.
- Never advertise SmartScreen reputation or a signed production artifact before verifying it. Test-policy mocks are not real signing acceptance.
