$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class SayAllLaunchSignals {
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
    public static extern IntPtr OpenEvent(uint access, bool inherit, string name);
    [DllImport("kernel32.dll", SetLastError=true)]
    public static extern bool SetEvent(IntPtr handle);
    [DllImport("kernel32.dll")]
    public static extern bool CloseHandle(IntPtr handle);
    public static bool Signal(string name) {
        var handle=OpenEvent(2,false,name);
        if(handle==IntPtr.Zero) return false;
        try { return SetEvent(handle); }
        finally { CloseHandle(handle); }
    }
}
'@
$exe = 'D:\无线麦 SayAll\sayall-windows-app.exe'
if (!(Test-Path -LiteralPath $exe)) { throw 'Latest executable missing' }
$old = @(Get-Process -Name 'sayall-windows-app' -ErrorAction SilentlyContinue)
if ($old.Count -gt 0) {
    if (![SayAllLaunchSignals]::Signal('Local\SayAll-GracefulExit')) { throw 'Graceful exit event unavailable; exit the old app from its tray menu first' }
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    do {
        Start-Sleep -Milliseconds 300
        $remaining = @($old | Where-Object { !$_.HasExited })
    } while ($remaining.Count -gt 0 -and [DateTime]::UtcNow -lt $deadline)
    if ($remaining.Count -gt 0) { throw 'Old app has not exited; stopped without force-killing or launching a second version' }
    'old_version graceful_exit=confirmed'
}
$process = Start-Process -FilePath $exe -WorkingDirectory (Split-Path $exe) -WindowStyle Hidden -PassThru
Start-Sleep -Seconds 6
$process.Refresh()
if ($process.HasExited) { throw "Latest executable exited with code $($process.ExitCode)" }
$show = [SayAllLaunchSignals]::Signal('Local\SayAll-ShowMainWindow')
Start-Sleep -Seconds 2
$process.Refresh()
[pscustomobject]@{
    PID=$process.Id
    Executable=$process.Path
    Running=(!$process.HasExited)
    ShowWindowRequested=$show
    MainWindowHandle=$process.MainWindowHandle.ToInt64()
    InstalledVersionReplaced=$false
} | ConvertTo-Json

