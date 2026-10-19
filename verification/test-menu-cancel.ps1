$ErrorActionPreference='Stop'
$root='D:\Project\sayall'
$probe=Join-Path $root 'verification\menu-cancel-probe.exe'
Copy-Item "$root\remote-mic-app-windows\target\release\examples\quick_menu_overlay_probe.exe" $probe -Force
& 'C:\Program Files (x86)\Windows Kits\10\bin\10.0.19041.0\x64\mt.exe' -nologo -manifest "$root\verification\twinkle-poc\quick-menu-probe.exe.manifest" "-outputresource:$probe;#1"
if($LASTEXITCODE -ne 0){throw 'manifest failed'}
Add-Type -AssemblyName UIAutomationClient
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class MenuMouseProbe {
 public struct POINT { public int X; public int Y; }
 [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd,out uint pid);
 [DllImport("user32.dll")] public static extern bool GetCursorPos(out POINT point);
 [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
 [DllImport("user32.dll")] public static extern void mouse_event(uint flags,uint dx,uint dy,uint data,UIntPtr extra);
}
'@
$clicked=$false
$foregroundWasProbe=$false
$clickStable=$false
$env:SAYALL_GATT_LOG="$root\verification\menu-cancel-native-diagnostic.log"
$p=Start-Process $probe -ArgumentList --exercise -PassThru -RedirectStandardOutput "$root\verification\menu-cancel-native.log" -RedirectStandardError "$root\verification\menu-cancel-native-error.log"
$null=$p.Handle
$seen=@{}
$until=[DateTime]::UtcNow.AddSeconds(30)
while(!$p.HasExited -and [DateTime]::UtcNow -lt $until){
    try {
        $condition=New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ProcessIdProperty,[int]$p.Id)
        $windows=[System.Windows.Automation.AutomationElement]::RootElement.FindAll([System.Windows.Automation.TreeScope]::Children,$condition)
        foreach($window in $windows){
            if($window.Current.IsOffscreen){continue}
            $fgPid=[uint32]0
            [void][MenuMouseProbe]::GetWindowThreadProcessId([MenuMouseProbe]::GetForegroundWindow(),[ref]$fgPid)
            if($fgPid -eq $p.Id){$foregroundWasProbe=$true}
            $documentCondition=New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty,[System.Windows.Automation.ControlType]::Document)
            $documents=$window.FindAll([System.Windows.Automation.TreeScope]::Descendants,$documentCondition)
            foreach($doc in $documents){
                try {
                    $text=$doc.GetCurrentPattern([System.Windows.Automation.TextPattern]::Pattern).DocumentRange.GetText(-1)
                    foreach($generation in 900001..900003){if($text.Contains("$generation")){$seen[$generation]=$true}}
                    $readyThree=@(Get-Content "$root\verification\menu-cancel-native.log" -ErrorAction SilentlyContinue | Where-Object {$_ -match 'generation=900003 ready=true visible=true'}).Count -gt 0
                    if($text.Contains('900003') -and $readyThree -and !$clicked){
                        $rect=$window.Current.BoundingRectangle
                        $original=New-Object 'MenuMouseProbe+POINT'
                        [void][MenuMouseProbe]::GetCursorPos([ref]$original)
                        try {
                            $beforeClick=[MenuMouseProbe]::GetForegroundWindow()
                            [void][MenuMouseProbe]::SetCursorPos([int]($rect.X+$rect.Width/2),[int]($rect.Y+$rect.Height/2))
                            [MenuMouseProbe]::mouse_event(2,0,0,0,[UIntPtr]::Zero)
                            [MenuMouseProbe]::mouse_event(4,0,0,0,[UIntPtr]::Zero)
                            $clicked=$true
                            Start-Sleep -Milliseconds 300
                            $afterClick=[MenuMouseProbe]::GetForegroundWindow()
                            $clickStable=$beforeClick -eq $afterClick
                            $afterPid=[uint32]0
                            [void][MenuMouseProbe]::GetWindowThreadProcessId($afterClick,[ref]$afterPid)
                            if($afterPid -eq $p.Id){$foregroundWasProbe=$true}
                        } finally { [void][MenuMouseProbe]::SetCursorPos($original.X,$original.Y) }
                    }
                }catch{}
            }
        }
    }catch{}
    Start-Sleep -Milliseconds 50
    $p.Refresh()
}
if(!$p.WaitForExit(5000)){throw 'probe did not exit'}
$lines=Get-Content "$root\verification\menu-cancel-native.log"
$lines
"native_labels_confirmed=$($seen.Count)"
"menu_became_foreground=$foregroundWasProbe click_foreground_stable=$clickStable"
if($foregroundWasProbe -or !$clickStable){throw 'Menu click changed the foreground window'}
# Judge the observed outcomes, not only the process exit code.
if(($lines -match 'ready=true visible=true.*reused=true').Count -ne 3){throw 'menu readiness/reuse failed'}
if(($lines -match 'hidden=true').Count -ne 4){throw 'menu cancellation/hide failed'}
if(!$seen.ContainsKey(900003)){throw 'final menu labels were not observed'}
if(($lines -match 'mouse_cancel_received=true').Count -ne 1){throw 'native mouse cancel failed'}
