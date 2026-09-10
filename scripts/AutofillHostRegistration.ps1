# Shared development-registration helpers. No browser policies or HKLM writes.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Get-KeyNestHostPlan([string]$Browser, [string]$ManifestDirectory) {
    if ($Browser -notin @('Chrome', 'Edge')) { throw 'Select Chrome or Edge.' }
    $vendor = if ($Browser -eq 'Chrome') { 'Google\Chrome' } else { 'Microsoft\Edge' }
    $directory = [IO.Path]::GetFullPath($ManifestDirectory)
    if ($directory.StartsWith('\\')) { throw 'Use a local manifest directory.' }
    [pscustomobject]@{
        Browser = $Browser
        RegistrySubkey = "Software\$vendor\NativeMessagingHosts\com.eyy.keynest.autofill"
        Directory = $directory
        ManifestPath = [IO.Path]::Combine($directory, "com.eyy.keynest.autofill.$Browser.json")
    }
}

function New-KeyNestHostManifest([string[]]$ExtensionId, [string]$HostExePath) {
    if ($ExtensionId.Count -eq 0) { throw 'Supply the actual unpacked extension ID.' }
    foreach ($id in $ExtensionId) {
        if ($id -cnotmatch '^[a-p]{32}$') { throw 'Extension IDs must contain exactly 32 lowercase letters a-p.' }
    }
    $exe = Get-Item -LiteralPath $HostExePath -ErrorAction Stop
    if ($exe.PSIsContainer -or $exe.Extension -ine '.exe' -or $exe.FullName.StartsWith('\\')) {
        throw 'Select an existing local native host executable.'
    }
    [ordered]@{
        name = 'com.eyy.keynest.autofill'
        description = 'KeyNest Autofill Native Host'
        path = $exe.FullName
        type = 'stdio'
        allowed_origins = @($ExtensionId | Select-Object -Unique | ForEach-Object { "chrome-extension://$_/" })
    }
}

function Get-KeyNestHostRegistration([string]$Subkey) {
    $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($Subkey)
    if ($null -eq $key) { return $null }
    try { return $key.GetValue('') } finally { $key.Dispose() }
}

function Set-KeyNestHostRegistration([string]$Subkey, [string]$ManifestPath) {
    $key = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey($Subkey)
    try { $key.SetValue('', $ManifestPath, [Microsoft.Win32.RegistryValueKind]::String) }
    finally { $key.Dispose() }
}

function Remove-KeyNestHostRegistration([string]$Subkey) {
    $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($Subkey, $true)
    if ($null -eq $key) { return }
    try {
        $key.DeleteValue('', $false)
        $empty = $key.ValueCount -eq 0 -and $key.SubKeyCount -eq 0
    } finally { $key.Dispose() }
    # Never recursively delete registry keys or remove other values.
    if ($empty) { [Microsoft.Win32.Registry]::CurrentUser.DeleteSubKey($Subkey, $false) }
}

function Assert-KeyNestHostOwnership($Plan) {
    $current = Get-KeyNestHostRegistration $Plan.RegistrySubkey
    if ($null -ne $current -and $current -ine $Plan.ManifestPath) {
        throw 'This browser already has a different KeyNest host registration. Preserve or unregister it explicitly first.'
    }
    if (Test-Path -LiteralPath $Plan.ManifestPath) {
        $manifest = Get-Content -LiteralPath $Plan.ManifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
        if ($manifest.name -ne 'com.eyy.keynest.autofill' -or $manifest.type -ne 'stdio') {
            throw 'The manifest path contains an unrelated file.'
        }
    }
}

function Install-KeyNestHost($Plan, $Manifest) {
    Assert-KeyNestHostOwnership $Plan
    $hadManifest = Test-Path -LiteralPath $Plan.ManifestPath
    $previous = if ($hadManifest) { [IO.File]::ReadAllBytes($Plan.ManifestPath) } else { $null }
    $null = [IO.Directory]::CreateDirectory($Plan.Directory)
    $temporary = [IO.Path]::Combine($Plan.Directory, ([guid]::NewGuid().ToString() + '.tmp'))
    try {
        $json = $Manifest | ConvertTo-Json -Depth 4
        [IO.File]::WriteAllText($temporary, $json, [Text.UTF8Encoding]::new($false))
        if ($hadManifest) { [IO.File]::Replace($temporary, $Plan.ManifestPath, $null) }
        else { [IO.File]::Move($temporary, $Plan.ManifestPath) }
        try { Set-KeyNestHostRegistration $Plan.RegistrySubkey $Plan.ManifestPath }
        catch {
            if ($hadManifest) { [IO.File]::WriteAllBytes($Plan.ManifestPath, $previous) }
            else { [IO.File]::Delete($Plan.ManifestPath) }
            throw
        }
    } finally {
        if ([IO.File]::Exists($temporary)) { [IO.File]::Delete($temporary) }
    }
}

function Uninstall-KeyNestHost($Plan) {
    Assert-KeyNestHostOwnership $Plan
    Remove-KeyNestHostRegistration $Plan.RegistrySubkey
    # Only the fixed browser-specific manifest file; no directory recursion.
    if ([IO.File]::Exists($Plan.ManifestPath)) { [IO.File]::Delete($Plan.ManifestPath) }
}
