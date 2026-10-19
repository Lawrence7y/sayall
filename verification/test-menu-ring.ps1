$ErrorActionPreference='Stop'
$root='D:\Project\sayall'
$probe=Join-Path $root 'verification\menu-ring-probe.exe'
Copy-Item "$root\remote-mic-app-windows\target\release\examples\quick_menu_overlay_probe.exe" $probe -Force
& 'C:\Program Files (x86)\Windows Kits\10\bin\10.0.19041.0\x64\mt.exe' -nologo -manifest "$root\verification\twinkle-poc\quick-menu-probe.exe.manifest" "-outputresource:$probe;#1"
if($LASTEXITCODE -ne 0){throw 'manifest failed'}
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class RingMouseProbe {
 public struct POINT { public int X; public int Y; }
 public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
 public delegate bool EnumProc(IntPtr hwnd, IntPtr data);
 [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc proc,IntPtr data);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd,out RECT rect);
 public static bool FindVisibleRect(uint wanted,out RECT rect) {
   RECT result=new RECT(); bool found=false;
   EnumProc proc=(hwnd,data)=> { uint pid; RECT candidate;
     GetWindowThreadProcessId(hwnd,out pid);
     if(pid==wanted && IsWindowVisible(hwnd) && GetWindowRect(hwnd,out candidate) && candidate.Right-candidate.Left>=200 && candidate.Bottom-candidate.Top>=200) { result=candidate;found=true;return false; }
     return true;
   };
   EnumWindows(proc,IntPtr.Zero);rect=result;return found;
 }
 [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd,out uint pid);
 [DllImport("user32.dll")] public static extern bool GetCursorPos(out POINT point);
 [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
 [DllImport("user32.dll")] public static extern void mouse_event(uint flags,uint dx,uint dy,uint data,UIntPtr extra);
}
'@
function Capture-Ring($rect,$path){
    $image=New-Object Drawing.Bitmap([int]$rect.Width,[int]$rect.Height)
    $graphics=[Drawing.Graphics]::FromImage($image)
    try{$graphics.CopyFromScreen([int]$rect.X,[int]$rect.Y,0,0,$image.Size);$image.Save($path,[Drawing.Imaging.ImageFormat]::Png)}finally{$graphics.Dispose();$image.Dispose()}
}
$env:SAYALL_GATT_LOG="$root\verification\menu-ring-native-diagnostic.log"
$stdout="$root\verification\menu-ring-native.log"
$p=Start-Process $probe -ArgumentList --exercise -PassThru -RedirectStandardOutput $stdout -RedirectStandardError "$root\verification\menu-ring-native-error.log"
$null=$p.Handle
$seen=@{}
$early=$false
$late=$false
$clicked=$false
$foregroundWasProbe=$false
$clickStable=$false
$until=[DateTime]::UtcNow.AddSeconds(35)
while(!$p.HasExited -and [DateTime]::UtcNow -lt $until){
    try{
        $frameLines=@(Get-Content $stdout -ErrorAction SilentlyContinue)
        $nativeRect=New-Object 'RingMouseProbe+RECT'
        if([RingMouseProbe]::FindVisibleRect([uint32]$p.Id,[ref]$nativeRect)){
            $frameRect=[pscustomobject]@{X=$nativeRect.Left;Y=$nativeRect.Top;Width=($nativeRect.Right-$nativeRect.Left);Height=($nativeRect.Bottom-$nativeRect.Top)}
            if(!$early -and ($frameLines -match 'ring_frame=early').Count -gt 0 -and ($frameLines -match 'ring_frame=late').Count -eq 0){
                Start-Sleep -Milliseconds 450
                Capture-Ring $frameRect "$root\verification\menu-ring-early.png"
                $early=$true
            }
            if(!$late -and ($frameLines -match 'ring_frame=late').Count -gt 0 -and ($frameLines -match 'generation=900002 ready').Count -eq 0){
                Start-Sleep -Milliseconds 80
                Capture-Ring $frameRect "$root\verification\menu-ring-final.png"
                $late=$true
            }
        }
        $condition=New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ProcessIdProperty,[int]$p.Id)
        $windows=[System.Windows.Automation.AutomationElement]::RootElement.FindAll([System.Windows.Automation.TreeScope]::Children,$condition)
        foreach($window in $windows){
            if($window.Current.IsOffscreen){continue}
            $fgPid=[uint32]0
            [void][RingMouseProbe]::GetWindowThreadProcessId([RingMouseProbe]::GetForegroundWindow(),[ref]$fgPid)
            if($fgPid -eq $p.Id){$foregroundWasProbe=$true}
            $lines=@(Get-Content $stdout -ErrorAction SilentlyContinue)
            $rect=$window.Current.BoundingRectangle
            if($rect.Width -lt 200 -or $rect.Height -lt 200){continue}
            $documentCondition=New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty,[System.Windows.Automation.ControlType]::Document)
            foreach($doc in $window.FindAll([System.Windows.Automation.TreeScope]::Descendants,$documentCondition)){
                try{
                    $text=$doc.GetCurrentPattern([System.Windows.Automation.TextPattern]::Pattern).DocumentRange.GetText(-1)
                    foreach($generation in 900001..900003){if($text.Contains("$generation")){$seen[$generation]=$true}}
                    if(!$clicked -and $text.Contains('900003') -and ($lines -match 'generation=900003 ready=true visible=true').Count -gt 0){
                        # Wait until the 480 ms entry animation has painted; a
                        # fully transparent first frame has no native hit target.
                        Start-Sleep -Milliseconds 650
                        $clickRect=New-Object 'RingMouseProbe+RECT'
                        if(![RingMouseProbe]::FindVisibleRect([uint32]$p.Id,[ref]$clickRect)){continue}
                        $original=New-Object 'RingMouseProbe+POINT'
                        [void][RingMouseProbe]::GetCursorPos([ref]$original)
                        $before=[RingMouseProbe]::GetForegroundWindow()
                        try{
                            [void][RingMouseProbe]::SetCursorPos([int](($clickRect.Left+$clickRect.Right)/2),[int](($clickRect.Top+$clickRect.Bottom)/2))
                            [RingMouseProbe]::mouse_event(2,0,0,0,[UIntPtr]::Zero)
                            [RingMouseProbe]::mouse_event(4,0,0,0,[UIntPtr]::Zero)
                            $clicked=$true
                            Start-Sleep -Milliseconds 300
                            $clickStable=$before -eq [RingMouseProbe]::GetForegroundWindow()
                        }finally{[void][RingMouseProbe]::SetCursorPos($original.X,$original.Y)}
                    }
                }catch{}
            }
        }
    }catch{}
    Start-Sleep -Milliseconds 50
    $p.Refresh()
}
if(!$p.WaitForExit(5000)){throw 'probe did not exit'}
$lines=Get-Content $stdout
$lines
"native_labels_confirmed=$($seen.Count) early_frame=$early late_frame=$late menu_became_foreground=$foregroundWasProbe click_foreground_stable=$clickStable"
if(($lines -match 'ready=true visible=true.*reused=true anchored_above_clock=true').Count -ne 3){throw 'ring window anchoring/reuse failed'}
if(($lines -match 'mouse_cancel_received=true').Count -ne 1){throw 'mouse cancel failed'}
if(($lines -match 'hidden=true').Count -ne 4){throw 'menu hide/cancel failed'}
if(!$early -or !$late -or $seen.Count -ne 3){throw 'ring render frames or labels not observed'}
if($foregroundWasProbe -or !$clickStable){throw 'ring stole foreground focus'}
$first=Get-FileHash "$root\verification\menu-ring-early.png" -Algorithm SHA256
$last=Get-FileHash "$root\verification\menu-ring-final.png" -Algorithm SHA256
if($first.Hash -eq $last.Hash){throw 'animation frames are identical'}
"animation_frames_differ=true"
