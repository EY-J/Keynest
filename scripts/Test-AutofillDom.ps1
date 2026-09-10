[CmdletBinding()]
param([ValidateSet('Chrome', 'Edge')][string]$Browser = 'Edge')
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$browserPath = if ($Browser -eq 'Chrome') { Join-Path $env:ProgramFiles 'Google\Chrome\Application\chrome.exe' }
else { Join-Path ${env:ProgramFiles(x86)} 'Microsoft\Edge\Application\msedge.exe' }
if (-not (Test-Path -LiteralPath $browserPath)) { throw 'Selected native browser is not installed at its standard location.' }
$fixturePath = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../tests/fixtures/autofill-dom.html'))
$fixtureUri = [Uri]::new($fixturePath).AbsoluteUri
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
$profile = [IO.Path]::GetFullPath((Join-Path $tempRoot ('keynest-autofill-dom-' + [guid]::NewGuid())))
$null = [IO.Directory]::CreateDirectory($profile)
$process = [Diagnostics.Process]::new()
try {
    $process.StartInfo = [Diagnostics.ProcessStartInfo]::new()
    $process.StartInfo.FileName = $browserPath
    $process.StartInfo.UseShellExecute = $false
    $process.StartInfo.CreateNoWindow = $true
    $process.StartInfo.WindowStyle = [Diagnostics.ProcessWindowStyle]::Hidden
    $process.StartInfo.RedirectStandardOutput = $true
    $process.StartInfo.RedirectStandardError = $true
    # Offline file-module fixture only. No remote debugging port or HTTP server.
    # The temporary profile and file access flag never change installed profiles.
    $process.StartInfo.Arguments = '--headless --disable-gpu --disable-background-networking --disable-extensions --no-first-run --no-default-browser-check --allow-file-access-from-files --virtual-time-budget=3000 --dump-dom --user-data-dir="{0}" "{1}"' -f $profile, $fixtureUri
    $null = $process.Start()
    $stdout = $process.StandardOutput.ReadToEndAsync()
    $stderr = $process.StandardError.ReadToEndAsync()
    if (-not $process.WaitForExit(30000)) { $process.Kill(); $process.WaitForExit(); throw 'Native DOM fixture timed out.' }
    $output = $stdout.GetAwaiter().GetResult()
    $null = $stderr.GetAwaiter().GetResult() # do not print browser diagnostic text
    if ($process.ExitCode -ne 0 -or $output -notmatch 'data-result="passed"') {
        $failureNames = [regex]::Match($output, 'data-failures="([^"]*)"').Groups[1].Value
        throw ('Native DOM fixture failed. ' + [Net.WebUtility]::HtmlDecode($failureNames))
    }
    $count = [regex]::Match($output, 'data-count="(\d+)"').Groups[1].Value
    Write-Output "PASS: $count DOM checks in headless $Browser with disposable local fixtures. Native extension/host acceptance was not exercised."
} finally {
    $process.Dispose()
    # Validate the resolved absolute, uniquely named target before recursive cleanup.
    $resolvedProfile = [IO.Path]::GetFullPath($profile)
    $prefix = $tempRoot.TrimEnd('\') + '\keynest-autofill-dom-'
    if (-not $resolvedProfile.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase)) { throw 'Refusing cleanup outside the disposable DOM profile.' }
    Remove-Item -LiteralPath $resolvedProfile -Recurse -Force
}
