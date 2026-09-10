[CmdletBinding(SupportsShouldProcess)]
param(
    [Parameter(Mandatory = $true)][ValidateSet('Chrome', 'Edge')][string]$Browser,
    [string]$ManifestDirectory = (Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'KeyNest\Autofill')
)
. (Join-Path $PSScriptRoot 'AutofillHostRegistration.ps1')
$plan = Get-KeyNestHostPlan $Browser $ManifestDirectory
Assert-KeyNestHostOwnership $plan
if ($PSCmdlet.ShouldProcess("HKCU\$($plan.RegistrySubkey)", 'Unregister KeyNest Autofill and remove its browser-specific manifest')) {
    Uninstall-KeyNestHost $plan
    Write-Output "Unregistered KeyNest Autofill for $Browser."
}
