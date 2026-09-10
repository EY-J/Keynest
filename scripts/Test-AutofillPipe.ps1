# Disposable-fixture IPC smoke client. Never use against a personal vault.
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^keynest-autofill-v1-S-[0-9-]+-test-[0-9]+-[0-9]+$')]
    [string]$PipeName,
    [Parameter(Mandatory = $true)][ValidateSet('Unlocked', 'Locked')][string]$State
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Read-ExactFixtureBytes($Pipe, [byte[]]$Bytes) {
    $offset = 0
    while ($offset -lt $Bytes.Length) {
        $read = $Pipe.ReadAsync($Bytes, $offset, $Bytes.Length - $offset)
        if (-not $read.Wait(12000)) { $Pipe.Dispose(); throw 'Fixture IPC read timed out.' }
        $count = $read.GetAwaiter().GetResult()
        if ($count -eq 0) { throw 'Fixture IPC ended before its frame completed.' }
        $offset += $count
    }
}

function Invoke-FixtureRequest([hashtable]$Request) {
    # Explicit rights match the server's restricted DACL; GenericWrite would also
    # request permission to create additional pipe instances.
    $rights = [System.IO.Pipes.PipeAccessRights]::ReadData -bor
        [System.IO.Pipes.PipeAccessRights]::WriteData -bor
        [System.IO.Pipes.PipeAccessRights]::Synchronize
    $pipe = [System.IO.Pipes.NamedPipeClientStream]::new(
        '.', $PipeName, $rights, [System.IO.Pipes.PipeOptions]::Asynchronous,
        [System.Security.Principal.TokenImpersonationLevel]::Identification,
        [System.IO.HandleInheritability]::None)
    try {
        $pipe.Connect(3000)
        $Request.version = 1
        $Request.requestId = [guid]::NewGuid().ToString()
        $body = [Text.Encoding]::UTF8.GetBytes(($Request | ConvertTo-Json -Compress))
        $prefix = [BitConverter]::GetBytes([uint32]$body.Length)
        foreach ($bytes in @($prefix, $body)) {
            $write = $pipe.WriteAsync($bytes, 0, $bytes.Length)
            if (-not $write.Wait(3000)) { throw 'Fixture IPC write timed out.' }
            $null = $write.GetAwaiter().GetResult()
        }
        $prefix = [byte[]]::new(4)
        Read-ExactFixtureBytes $pipe $prefix
        $length = [BitConverter]::ToUInt32($prefix, 0)
        if ($length -lt 1 -or $length -gt 65536) { throw 'Fixture response length invalid.' }
        $body = [byte[]]::new($length)
        Read-ExactFixtureBytes $pipe $body
        $response = [Text.Encoding]::UTF8.GetString($body) | ConvertFrom-Json
        $receipt = $pipe.WriteAsync([byte[]]@(6), 0, 1)
        if (-not $receipt.Wait(3000)) { throw 'Fixture receipt timed out.' }
        $null = $receipt.GetAwaiter().GetResult()
        if ($response.version -ne 1 -or $response.requestId -ne $Request.requestId) {
            throw 'Fixture response correlation failed.'
        }
        return $response
    } finally { $pipe.Dispose() }
}

$status = Invoke-FixtureRequest @{ type = 'status' }
if (-not $status.ok -or $status.data.state -ne $State.ToLowerInvariant()) {
    throw 'Fixture status did not match the expected state.'
}
$page = 'https://fixture.example.test/login'
$matches = Invoke-FixtureRequest @{ type = 'queryMatches'; pageUrl = $page }
if ($State -eq 'Locked') {
    if ($matches.ok -or $matches.error.code -ne 'VAULT_LOCKED') { throw 'Locked query was not refused.' }
    $fill = Invoke-FixtureRequest @{ type = 'requestFill'; pageUrl = $page; credentialId = ('a' * 32) }
    if ($fill.ok -or $fill.error.code -ne 'VAULT_LOCKED') { throw 'Locked fill was not refused.' }
    Write-Output 'PASS: native IPC status, locked query refusal, and locked fill refusal.'
} else {
    if (-not $matches.ok -or @($matches.data.matches).Count -ne 1) { throw 'Fixture matching failed.' }
    $match = $matches.data.matches[0]
    if ($match.username -ne 'fixture-user' -or $match.PSObject.Properties.Name -contains 'password') {
        throw 'Fixture summary contract failed.'
    }
    $fill = Invoke-FixtureRequest @{ type = 'requestFill'; pageUrl = $page; credentialId = $match.credentialId }
    if (-not $fill.ok -or $fill.data.username -ne 'fixture-user' -or
        $fill.data.password -ne 'disposable IPC fixture password') { throw 'Fixture fill failed.' }
    $mismatch = Invoke-FixtureRequest @{ type = 'requestFill'; pageUrl = 'https://fixture.example.test.evil.test'; credentialId = $match.credentialId }
    if ($mismatch.ok -or $mismatch.error.code -ne 'DOMAIN_MISMATCH') { throw 'Fixture domain mismatch was not refused.' }
    $none = Invoke-FixtureRequest @{ type = 'queryMatches'; pageUrl = 'https://other.example.test' }
    if ($none.ok -or $none.error.code -ne 'NO_MATCHES') { throw 'Fixture no-match query failed.' }
    Write-Output 'PASS: native IPC status, password-free match, selected fill, domain mismatch, and no matches.'
}
# Fixture-only strings cannot be securely erased by PowerShell/.NET. Never print
# payloads or use real credentials here; only pass/fail summaries reach stdout.
