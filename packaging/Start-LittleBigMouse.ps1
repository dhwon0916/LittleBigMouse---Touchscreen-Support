# Starts the daemon shipped beside this script, then the UI.
[CmdletBinding()]
param([switch]$Elevated, [switch]$NoPause)
$ErrorActionPreference = 'Stop'
$errorPath = Join-Path $PSScriptRoot 'launch-error.txt'
function Write-LaunchStep([string]$Message) {
    Add-Content -LiteralPath (Join-Path $PSScriptRoot 'launch-progress.txt') -Value ("{0:o} PID={1} {2}" -f [DateTime]::Now, $PID, $Message)
}
try {
    Write-LaunchStep 'Launcher entered'
    $uiPath = Join-Path $PSScriptRoot 'LittleBigMouse.Ui.Avalonia.exe'
    $hookPath = Join-Path $PSScriptRoot 'LittleBigMouse.Hook.exe'
    if (!(Test-Path -LiteralPath $uiPath -PathType Leaf) -or !(Test-Path -LiteralPath $hookPath -PathType Leaf)) {
        throw 'Missing application files. Extract the entire release ZIP before running Start-LittleBigMouse.cmd.'
    }
    $principal = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
    if (!$principal.IsInRole([Security.Principal.WindowsBuiltinRole]::Administrator)) {
        Write-LaunchStep 'Requesting administrator access'
        if ($Elevated) { throw 'Administrator access was not granted.' }
        $child = Start-Process -FilePath "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" -Verb RunAs -WindowStyle Hidden -PassThru -ArgumentList @(
            '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', ('"' + $PSCommandPath + '"'), '-Elevated', '-NoPause'
        )
        $child.WaitForExit()
        if ($child.ExitCode -ne 0) {
            $details = if (Test-Path -LiteralPath $errorPath) { Get-Content -LiteralPath $errorPath -Raw } else { 'The elevated launcher failed before writing its error log.' }
            throw $details
        }
        exit 0
    }
    $session = (Get-Process -Id $PID).SessionId
    Write-LaunchStep 'Administrator access granted; stopping older instances'
    $old = @(Get-Process -Name 'LittleBigMouse.Ui.Avalonia','LittleBigMouse.Hook','lbm-hook' -ErrorAction SilentlyContinue |
        Where-Object { $_.SessionId -eq $session })
    foreach ($process in $old) {
        if (!$process.HasExited) {
            $process | Stop-Process -Force
            if (!$process.WaitForExit(5000)) { throw 'An older LittleBigMouse instance did not stop.' }
        }
    }
    Remove-Item Env:LBM_TOUCH_TRACE -ErrorAction SilentlyContinue
    # The explicit launcher invocation trusts these bundled executables. Remove
    # their download marker so ShellExecute cannot hide a SmartScreen prompt
    # behind the hidden daemon window. No machine security policy is changed.
    Unblock-File -LiteralPath $hookPath, $uiPath
    Write-LaunchStep 'Starting bundled daemon'
    $daemon = Start-Process -FilePath $hookPath -WorkingDirectory $PSScriptRoot -WindowStyle Hidden -PassThru
    Start-Sleep -Milliseconds 1000
    $daemon.Refresh()
    if ($daemon.HasExited) { throw 'The bundled daemon exited during startup.' }
    if (![string]::Equals($daemon.MainModule.FileName, $hookPath, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'A different daemon started instead of the bundled version.'
    }
    Write-LaunchStep 'Starting UI'
    $ui = Start-Process -FilePath $uiPath -WorkingDirectory $PSScriptRoot -PassThru
    Start-Sleep -Milliseconds 1000
    $ui.Refresh()
    if ($ui.HasExited) { throw 'The application UI exited during startup.' }
    @('PID=' + $daemon.Id, 'Path=' + $daemon.MainModule.FileName, 'Version=' + $daemon.MainModule.FileVersionInfo.ProductVersion) |
        Set-Content -LiteralPath (Join-Path $PSScriptRoot 'running-release.txt')
    if (Test-Path -LiteralPath $errorPath) { Remove-Item -LiteralPath $errorPath }
    Write-LaunchStep 'Startup complete'
}
catch {
    $details = $_ | Out-String
    try { Set-Content -LiteralPath $errorPath -Value $details } catch { }
    Write-Host $details -ForegroundColor Red
    if (!$NoPause -and !$Elevated) { Read-Host 'Press Enter to close' | Out-Null }
    exit 1
}
