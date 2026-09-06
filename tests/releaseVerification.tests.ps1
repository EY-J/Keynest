# Policy tests use mocked signature outcomes; real certificate acceptance is separate.
param([string]$ArtifactPath = (Join-Path $PSScriptRoot '../src-tauri/target/debug/keynest.exe'))
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$verify = Join-Path $PSScriptRoot '../scripts/Verify-Release.ps1'
$thumbprint = 'A' * 40
$hash = (Get-FileHash -LiteralPath $ArtifactPath -Algorithm SHA256).Hash
function Assert-Rejected([scriptblock]$Action, [string]$Expected) {
    try { & $Action | Out-Null } catch {
        if ($_.Exception.Message -notlike "*$Expected*") { throw }
        return
    }
    throw 'Verification unexpectedly accepted an invalid artifact.'
}
# First use the real Windows API on the local unsigned development build.
Assert-Rejected { & $verify $ArtifactPath $thumbprint $hash } 'signature is absent or invalid'
function Get-AuthenticodeSignature {
    param([string]$LiteralPath)
    return $global:keynestReleaseSignatureFixture
}
$global:keynestReleaseSignatureFixture = [pscustomobject]@{
    Status = 'Valid'; SignerCertificate = [pscustomobject]@{ Thumbprint = $thumbprint }
    TimeStamperCertificate = [pscustomobject]@{ Thumbprint = 'B' * 40 }
}
$result = & $verify $ArtifactPath $thumbprint $hash
if ($result.Signature -ne 'Valid') { throw 'Valid policy fixture rejected.' }
Assert-Rejected { & $verify $ArtifactPath $thumbprint ('0' * 64) } 'checksum does not match'
Assert-Rejected { & $verify $ArtifactPath ('C' * 40) $hash } 'Unexpected release publisher'
$global:keynestReleaseSignatureFixture.Status = 'HashMismatch'
Assert-Rejected { & $verify $ArtifactPath $thumbprint $hash } 'signature is absent or invalid'
$global:keynestReleaseSignatureFixture.Status = 'Valid'
$global:keynestReleaseSignatureFixture.TimeStamperCertificate = $null
Assert-Rejected { & $verify $ArtifactPath $thumbprint $hash } 'no verified timestamp'
Write-Output '6 release verification checks passed (5 simulated, 1 real unsigned rejection). No artifact executed.'
