[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$ArtifactPath,
    [Parameter(Mandatory = $true)][ValidatePattern('^[A-Fa-f0-9]{40}$')][string]$ExpectedSignerThumbprint,
    [Parameter(Mandatory = $true)][ValidatePattern('^[A-Fa-f0-9]{64}$')][string]$ExpectedSha256
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# Read-only verification; never downloads, installs, executes or alters artifacts/data.
$artifact = Get-Item -LiteralPath $ArtifactPath
if ($artifact.PSIsContainer -or $artifact.Extension -notin @('.exe', '.msi')) {
    throw 'Select a Windows executable or MSI artifact.'
}
$hash = (Get-FileHash -LiteralPath $artifact.FullName -Algorithm SHA256).Hash
if ($hash -ne $ExpectedSha256) { throw 'Release checksum does not match. Do not install.' }
$signature = Get-AuthenticodeSignature -LiteralPath $artifact.FullName
if ($signature.Status -ne 'Valid' -or $null -eq $signature.SignerCertificate) {
    throw 'Release signature is absent or invalid. Do not install.'
}
if ($signature.SignerCertificate.Thumbprint -ne $ExpectedSignerThumbprint) {
    throw 'Unexpected release publisher certificate. Do not install.'
}
if ($null -eq $signature.TimeStamperCertificate) {
    throw 'Release has no verified timestamp. Do not install.'
}
[pscustomobject]@{
    Artifact = $artifact.Name
    Signature = 'Valid'
    SignerThumbprint = $signature.SignerCertificate.Thumbprint
    Sha256 = $hash
}
