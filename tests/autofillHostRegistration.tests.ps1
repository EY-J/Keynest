# Uses mocked registry functions and a workspace-local temporary fixture. No real
# browser registration, registry policy changes, or executable launches.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot '../scripts/AutofillHostRegistration.ps1')
$registry = @{}
$failRegistry = $false
function Get-KeyNestHostRegistration([string]$Subkey) { return $registry[$Subkey] }
function Set-KeyNestHostRegistration([string]$Subkey, [string]$ManifestPath) {
    if ($failRegistry) { throw 'Fixture registry failure' }
    $registry[$Subkey] = $ManifestPath
}
function Remove-KeyNestHostRegistration([string]$Subkey) { $registry.Remove($Subkey) }
function Assert-True([bool]$Condition) { if (-not $Condition) { throw 'Registration policy assertion failed.' } }
function Assert-Rejected([scriptblock]$Action) {
    $failed = $false
    try { & $Action | Out-Null } catch { $failed = $true }
    Assert-True $failed
}
$fixture = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot ('.autofill-registration-' + [guid]::NewGuid())))
$null = [IO.Directory]::CreateDirectory($fixture)
try {
    $exe = Join-Path $fixture 'host with spaces.exe'
    [IO.File]::WriteAllText($exe, 'fixture - never executed')
    $chrome = Get-KeyNestHostPlan Chrome $fixture
    $edge = Get-KeyNestHostPlan Edge $fixture
    Assert-True ($chrome.RegistrySubkey -eq 'Software\Google\Chrome\NativeMessagingHosts\com.eyy.keynest.autofill')
    Assert-True ($edge.RegistrySubkey -eq 'Software\Microsoft\Edge\NativeMessagingHosts\com.eyy.keynest.autofill')
    $manifest = New-KeyNestHostManifest @(('a' * 32), ('b' * 32)) $exe
    Assert-True ($manifest.allowed_origins.Count -eq 2)
    Assert-True ($manifest.path -eq $exe)
    foreach ($bad in @('*', 'guessed-id', ('A' * 32), ('z' * 32), ('a' * 31), ('a' * 33))) {
        Assert-Rejected { New-KeyNestHostManifest @($bad) $exe }
    }
    Assert-Rejected { New-KeyNestHostManifest @(('a' * 32)) (Join-Path $fixture 'missing.exe') }
    Install-KeyNestHost $chrome $manifest
    Install-KeyNestHost $edge (New-KeyNestHostManifest @(('c' * 32)) $exe)
    $bytes = [IO.File]::ReadAllBytes($chrome.ManifestPath)
    Assert-True ($bytes[0] -eq [byte][char]'{')
    $saved = Get-Content -LiteralPath $chrome.ManifestPath -Raw | ConvertFrom-Json
    Assert-True ($saved.allowed_origins[0] -eq ('chrome-extension://' + ('a' * 32) + '/'))
    Assert-True ($registry[$chrome.RegistrySubkey] -eq $chrome.ManifestPath)
    $before = [IO.File]::ReadAllText($chrome.ManifestPath)
    $failRegistry = $true
    Assert-Rejected { Install-KeyNestHost $chrome (New-KeyNestHostManifest @(('d' * 32)) $exe) }
    Assert-True ([IO.File]::ReadAllText($chrome.ManifestPath) -eq $before)
    $failRegistry = $false
    $registry[$chrome.RegistrySubkey] = 'C:\unrelated\manifest.json'
    Assert-Rejected { Uninstall-KeyNestHost $chrome }
    Assert-True ([IO.File]::Exists($chrome.ManifestPath))
    $registry[$chrome.RegistrySubkey] = $chrome.ManifestPath
    Uninstall-KeyNestHost $chrome
    Assert-True (-not [IO.File]::Exists($chrome.ManifestPath))
    Assert-True ([IO.File]::Exists($edge.ManifestPath))
    Uninstall-KeyNestHost $chrome # idempotent
    Uninstall-KeyNestHost $edge
    Write-Output 'PASS: exact IDs, local paths, UTF-8 manifest, separate browsers, rollback, ownership, and idempotent unregister (mock registry).'
} finally {
    # Delete only direct fixture files and the now-empty directory, never recurse.
    Get-ChildItem -LiteralPath $fixture -File | ForEach-Object { Remove-Item -LiteralPath $_.FullName }
    [IO.Directory]::Delete($fixture)
}
