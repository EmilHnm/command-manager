#requires -Version 7.0
<#
Read-only regression check against an existing pnpm project. It does not run
install scripts, modify node_modules, start the app, or touch its database.
Build first: cargo build --manifest-path src-tauri/Cargo.toml --example terminal_fs_probe
Example: ./scripts/test-windows-terminal-launch.ps1 -ProjectRoot G:/Work/ln-project
Requires an interactive Explorer desktop, Node, pnpm and PowerShell.
Redirection Guard is enabled ONLY in a short-lived diagnostic child process.
Reports are kept in a uniquely named temporary directory for inspection.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$ProjectRoot,
    [string]$RelativeFile = 'node_modules/concurrently/dist/bin/concurrently.js',
    [string]$ProbeExe = "$PSScriptRoot/../src-tauri/target/debug/examples/terminal_fs_probe.exe",
    [switch]$GuardedChild,
    [switch]$DesktopHandoff,
    [string]$ReportPath
)

$ErrorActionPreference = 'Stop'
$ProbeExe = (Resolve-Path -LiteralPath $ProbeExe).Path
$ProjectRoot = (Resolve-Path -LiteralPath $ProjectRoot).Path

function Invoke-Probe([string[]]$ProbeArgs) {
    $info = [System.Diagnostics.ProcessStartInfo]::new($ProbeExe)
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    foreach ($arg in $ProbeArgs) { $info.ArgumentList.Add($arg) }
    $process = [System.Diagnostics.Process]::Start($info)
    try {
        if (-not $process.WaitForExit(45000)) { throw 'Probe timed out' }
        # A guarded filesystem failure is an expected test result, not a harness error.
        return $process.ExitCode
    } finally { $process.Dispose() }
}

if ($GuardedChild) {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class CmProbeGuard {
    [DllImport("kernel32.dll", SetLastError=true)]
    public static extern bool SetProcessMitigationPolicy(int policy, ref uint flags, UIntPtr size);
}
'@
    [uint32]$flags = 1
    if (-not [CmProbeGuard]::SetProcessMitigationPolicy(16, [ref]$flags, [UIntPtr]4)) {
        throw "Cannot enable diagnostic guard: $([Runtime.InteropServices.Marshal]::GetLastWin32Error())"
    }
    $probeArgs = @($ProjectRoot, $RelativeFile, $ReportPath)
    if ($DesktopHandoff) { $probeArgs = @('--launch-from-installer') + $probeArgs }
    $probeExit = Invoke-Probe $probeArgs
    if ($DesktopHandoff -and $probeExit -ne 0) { throw "Desktop handoff failed: $probeExit" }
    exit 0
}

if (-not (Test-Path -LiteralPath (Join-Path $ProjectRoot $RelativeFile) -PathType Leaf)) {
    throw 'Baseline file is not readable outside the app; this probe requires an existing pnpm junction.'
}
$reportDirectory = Join-Path ([IO.Path]::GetTempPath()) ('cm-launch-probe-' + [guid]::NewGuid())
$null = New-Item -ItemType Directory -Path $reportDirectory
foreach ($mode in @('normal', 'guarded', 'desktop')) {
    $report = Join-Path $reportDirectory "$mode.txt"
    if ($mode -eq 'normal') {
        $null = Invoke-Probe @($ProjectRoot, $RelativeFile, $report)
    } else {
        $info = [System.Diagnostics.ProcessStartInfo]::new((Get-Process -Id $PID).Path)
        $info.UseShellExecute = $false
        $info.CreateNoWindow = $true
        foreach ($arg in @('-NoProfile', '-File', $PSCommandPath, '-GuardedChild',
            '-ProjectRoot', $ProjectRoot, '-RelativeFile', $RelativeFile,
            '-ProbeExe', $ProbeExe, '-ReportPath', $report)) { $info.ArgumentList.Add($arg) }
        if ($mode -eq 'desktop') { $info.ArgumentList.Add('-DesktopHandoff') }
        $process = [System.Diagnostics.Process]::Start($info)
        try {
            if (-not $process.WaitForExit(45000)) { throw "$mode launcher timed out" }
            if ($process.ExitCode -ne 0) { throw "$mode launcher failed: $($process.ExitCode)" }
        } finally { $process.Dispose() }
    }
    $deadline = [DateTime]::UtcNow.AddSeconds(45)
    while (-not (Test-Path -LiteralPath $report)) {
        if ([DateTime]::UtcNow -ge $deadline) { throw "No $mode report: $report" }
        Start-Sleep -Milliseconds 100
    }
    $text = Get-Content -LiteralPath $report -Raw
    $expectedGuard = if ($mode -eq 'guarded') { 'true' } else { 'false' }
    $expectedSuccess = if ($mode -eq 'guarded') { 'false' } else { 'true' }
    if (-not $text.StartsWith("redirection_guard=Some($expectedGuard)`nshell_success=$expectedSuccess`n")) {
        throw "Unexpected $mode result. See $report"
    }
    Write-Output "$mode PASS (guard=$expectedGuard, read/pnpm success=$expectedSuccess)"
}
Write-Output "Reports: $reportDirectory"
