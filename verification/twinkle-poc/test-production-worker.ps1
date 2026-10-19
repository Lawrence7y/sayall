$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$workspace = Join-Path (Split-Path -Parent $root) 'remote-mic-app-windows'
function Read-Hardware {
    $output = @(& (Join-Path $PSScriptRoot 'probe.exe'))
    if ($LASTEXITCODE -ne 0) { throw 'Read-only probe failed' }
    $text = $output -join "`n"
    if ($text -notmatch 'hardware readable_count=1' -or ([regex]::Matches($text,'type=ddcci')).Count -ne 1) { throw 'Requires exactly one controllable/readable display' }
    if ($text -notmatch 'hardware brightness_raw=(\d+) max=100') { throw 'Unsupported hardware scale' }
    return [int]$Matches[1]
}
$baseline = Read-Hardware
if ($baseline -lt 5 -or $baseline -gt 94) { throw 'Baseline near boundary' }
# Never use this equal-value restore on a multi-monitor setup with different initial levels.
try {
    & (Join-Path $workspace 'target\debug\examples\twinkle_all_probe.exe') --exercise
    if ($LASTEXITCODE -ne 0) { throw 'Production worker failed' }
    Start-Sleep -Seconds 1
    $after = Read-Hardware
    "production_hardware before=$baseline after=$after expected=$($baseline+3)"
    if ($after -ne $baseline+3) { throw 'Production worker did not accumulate three steps' }
    'production_hardware result=passed'
} finally {
    $si = New-Object Diagnostics.ProcessStartInfo
    $si.FileName = "$env:LOCALAPPDATA\Programs\twinkle-tray\Twinkle Tray.exe"
    $si.Arguments = "--All --Set=$baseline"
    $si.UseShellExecute = $false
    $si.CreateNoWindow = $true
    $process = [Diagnostics.Process]::Start($si)
    try {
        if (!$process.WaitForExit(8000)) { $process.Kill(); throw 'Restore CLI timeout' }
        if ($process.ExitCode -ne 0) { throw 'Restore CLI failed' }
    } finally { $process.Dispose() }
    Start-Sleep -Seconds 1
    $restored = Read-Hardware
    "production_restore initial=$baseline actual=$restored hardware_confirmed=$($restored -eq $baseline)"
    if ($restored -ne $baseline) { throw 'RESTORATION FAILED: check display brightness manually' }
}
