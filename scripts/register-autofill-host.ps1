[CmdletBinding(SupportsShouldProcess)]
param(
    [Parameter(Mandatory = $true)][ValidateSet('Chrome', 'Edge')][string]$Browser,
    [Parameter(Mandatory = $true)][string]$HostExePath,
    [string]$ExtensionManifestPath,
    [string]$ManifestDirectory = (Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'KeyNest\Autofill')
)
. (Join-Path $PSScriptRoot 'AutofillHostRegistration.ps1')
if ([string]::IsNullOrWhiteSpace($ExtensionManifestPath)) {
    $ExtensionManifestPath = Join-Path $PSScriptRoot '../browser-extension/manifest.json'
}
$extensionId = Get-KeyNestExtensionId $ExtensionManifestPath
$plan = Get-KeyNestHostPlan $Browser $ManifestDirectory
$manifest = New-KeyNestHostManifest @($extensionId) $HostExePath
Assert-KeyNestHostOwnership $plan
if ($PSCmdlet.ShouldProcess("HKCU\$($plan.RegistrySubkey)", 'Write exact-origin manifest and register KeyNest Autofill host')) {
    Install-KeyNestHost $plan $manifest
    Write-Output "Registered KeyNest Autofill for $Browser extension $extensionId. Manifest: $($plan.ManifestPath)"
}
