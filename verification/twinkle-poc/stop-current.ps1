$ErrorActionPreference='Stop'
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class SayAllStopSignal {
 [DllImport("kernel32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr OpenEvent(uint access,bool inherit,string name);
 [DllImport("kernel32.dll")] public static extern bool SetEvent(IntPtr handle);
 [DllImport("kernel32.dll")] public static extern bool CloseHandle(IntPtr handle);
}
'@
$old=@(Get-Process -Name sayall-windows-app -ErrorAction SilentlyContinue)
if($old.Count -eq 0){'not_running';exit 0}
$h=[SayAllStopSignal]::OpenEvent(2,$false,'Local\SayAll-GracefulExit')
if($h -eq [IntPtr]::Zero){throw 'Graceful exit unavailable; close from tray menu'}
try{if(![SayAllStopSignal]::SetEvent($h)){throw 'Exit request not delivered'}}finally{[void][SayAllStopSignal]::CloseHandle($h)}
$deadline=[DateTime]::UtcNow.AddSeconds(30)
do{Start-Sleep -Milliseconds 200;$remaining=@($old|Where-Object{!$_.HasExited})}while($remaining.Count -gt 0 -and [DateTime]::UtcNow -lt $deadline)
if($remaining.Count -gt 0){throw 'Old app still running; no process was force-killed'}
'graceful_exit_confirmed=true'
