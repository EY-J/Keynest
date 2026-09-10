[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][ValidateSet('Start', 'Inspect', 'InspectApproval', 'CancelApproval', 'Unlock', 'Lock', 'Stop')][string]$Action,
    [int]$DesktopProcessId = 0,
    [ValidatePattern('^[a-z0-9.-]{1,253}$')][string]$RequestedHost = '',
    [switch]$Visible
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$exe = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../src-tauri/target/autofill-acceptance/debug/keynest.exe'))
if ($Action -eq 'Start') {
    $sid = [Security.Principal.WindowsIdentity]::GetCurrent().User.Value
    $session = (Get-Process -Id $PID).SessionId
    if ([IO.File]::Exists(('\\.\pipe\keynest-autofill-v1-' + $sid + '-' + $session))) { throw 'Existing desktop owns Autofill; refusing to substitute an acceptance desktop.' }
    $windowStyle = if ($Visible) { 'Normal' } else { 'Hidden' }
    $desktop = Start-Process -FilePath $exe -PassThru -WindowStyle $windowStyle
    Write-Output $desktop.Id
    exit
}
$desktop = Get-Process -Id $DesktopProcessId
if ($desktop.Path -ine $exe) { throw 'Refusing to control a non-acceptance desktop process.' }
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$condition = [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ProcessIdProperty, $DesktopProcessId)
$roots = [Windows.Automation.AutomationElement]::RootElement.FindAll([Windows.Automation.TreeScope]::Children, $condition)
$root = $null
foreach ($candidate in $roots) {
    if ($candidate.Current.ClassName -eq 'Tauri Window') { $root = $candidate; break }
}
if (-not $root) { throw 'Acceptance desktop window is unavailable.' }
function Find-Control([string]$Name) {
    $filter = [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty, $Name)
    $result = $root.FindFirst([Windows.Automation.TreeScope]::Descendants, $filter)
    if (-not $result) { throw 'Expected acceptance desktop control is unavailable.' }
    return $result
}
function Invoke-Control($Control) {
    try { $Control.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke() }
    catch { $Control.GetCurrentPattern([Windows.Automation.LegacyIAccessiblePattern]::Pattern).DoDefaultAction() }
}
function Wait-Control([string]$Name) {
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    do {
        try { return Find-Control $Name } catch { Start-Sleep -Milliseconds 100 }
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'Expected acceptance desktop control did not appear.'
}
if ($Action -in @('Lock', 'Unlock')) {
    # A fresh isolated WebView profile may still be starting after the window exists.
    $readyDeadline = [DateTime]::UtcNow.AddSeconds(10)
    $ready = $false
    do {
        foreach ($name in @('Unlock KeyNest', 'Open navigation')) {
            try { $null = Find-Control $name; $ready = $true; break } catch { }
        }
        if (-not $ready) { Start-Sleep -Milliseconds 100 }
    } while (-not $ready -and [DateTime]::UtcNow -lt $readyDeadline)
    if (-not $ready) { throw 'Acceptance desktop UI did not become ready.' }
}
switch ($Action) {
    'Inspect' {
        # Control names only; never read input/output values or Recovery Key text.
        foreach ($name in @('Master password', 'Unlock KeyNest', 'Lock KeyNest', 'Lock KeyNest (Ctrl+Shift+L)', 'Vault')) {
            try { $null = Find-Control $name; Write-Output "Available: $name" } catch { }
        }
    }
    'InspectApproval' {
        if (-not $RequestedHost) { throw 'RequestedHost is required for approval inspection.' }
        $null = Wait-Control 'Approve login host'
        $null = Wait-Control $RequestedHost
        $null = Wait-Control 'Cancel host approval'
        Write-Output "Acceptance desktop shows the approval modal for $RequestedHost."
    }
    'CancelApproval' {
        $button = Wait-Control 'Cancel host approval'
        Invoke-Control $button
        $deadline = [DateTime]::UtcNow.AddSeconds(3)
        do {
            try { $null = Find-Control 'Approve login host'; Start-Sleep -Milliseconds 100 }
            catch { Write-Output 'Acceptance host approval cancelled through its UI.'; exit }
        } while ([DateTime]::UtcNow -lt $deadline)
        throw 'Acceptance host approval modal did not close.'
    }
    'Unlock' {
        $passwordFilter = [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::IsPasswordProperty, $true)
        $fields = $root.FindAll([Windows.Automation.TreeScope]::Descendants, $passwordFilter)
        if ($fields.Count -ne 1) { throw 'Expected exactly one protected acceptance password input.' }
        $field = $fields[0]
        $value = $field.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern)
        $value.SetValue('copper-planet-72-MINT!autofill')
        $button = Find-Control 'Unlock KeyNest'
        Invoke-Control $button
        Write-Output 'Acceptance desktop unlock invoked through its UI.'
    }
    'Lock' {
        try { $null = Find-Control 'Unlock KeyNest'; Write-Output 'Acceptance desktop is already locked.'; exit } catch { }
        Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class AcceptanceForeground {
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint dx, uint dy, uint data, UIntPtr extra);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr window, int command);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr window);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr window, out uint processId);
}
'@
        $windowHandle = [IntPtr]$root.Current.NativeWindowHandle
        $null = [AcceptanceForeground]::ShowWindow($windowHandle, 9)
        $null = [AcceptanceForeground]::SetForegroundWindow($windowHandle)
        [uint32]$foregroundProcess = 0
        $null = [AcceptanceForeground]::GetWindowThreadProcessId([AcceptanceForeground]::GetForegroundWindow(), [ref]$foregroundProcess)
        if ($foregroundProcess -ne $DesktopProcessId) { throw 'Acceptance desktop does not own keyboard focus.' }
        $menu = Find-Control 'Open navigation'
        $rectangle = $menu.Current.BoundingRectangle
        if ($rectangle.Width -le 0 -or $rectangle.Height -le 0) { throw 'Acceptance navigation has no visible bounds.' }
        $null = [AcceptanceForeground]::SetCursorPos([int]($rectangle.X + $rectangle.Width / 2), [int]($rectangle.Y + $rectangle.Height / 2))
        [AcceptanceForeground]::mouse_event(2, 0, 0, 0, [UIntPtr]::Zero)
        [AcceptanceForeground]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero)
        Start-Sleep -Milliseconds 300
        $button = Find-Control 'Lock KeyNest'
        Invoke-Control $button
        Write-Output 'Acceptance desktop lock invoked through its existing navigation UI.'
    }
    'Stop' {
        $root.GetCurrentPattern([Windows.Automation.WindowPattern]::Pattern).Close()
        Write-Output 'Acceptance desktop close requested through its window.'
    }
}
