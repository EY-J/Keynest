[CmdletBinding(SupportsShouldProcess)]
param(
    [Parameter(Mandatory = $true)][ValidateSet('Chrome', 'Edge')][string]$Browser,
    [Parameter(Mandatory = $true)][string[]]$ExtensionId,
    [Parameter(Mandatory = $true)][string]$HostExePath,
    [string]$ManifestDirectory = (Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'KeyNest\Autofill')
)
. (Join-Path $PSScriptRoot 'AutofillHostRegistration.ps1')
$plan = Get-KeyNestHostPlan $Browser $ManifestDirectory
$manifest = New-KeyNestHostManifest $ExtensionId $HostExePath
Assert-KeyNestHostOwnership $plan
if ($PSCmdlet.ShouldProcess("HKCU\$($plan.RegistrySubkey)", 'Write exact-origin manifest and register KeyNest Autofill host')) {
    Install-KeyNestHost $plan $manifest
    Write-Output "Registered KeyNest Autofill for $Browser. Manifest: $($plan.ManifestPath)"
}
