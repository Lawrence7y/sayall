$ErrorActionPreference='Stop'
[Console]::OutputEncoding=[System.Text.Encoding]::UTF8
$root='D:\Project\sayall'
$repo=Join-Path $root 'remote-mic-app-windows'
function Stage-Probe($name,$destination){
 Copy-Item "$repo\target\release\examples\$name.exe" $destination -Force
 & 'C:\Program Files (x86)\Windows Kits\10\bin\10.0.19041.0\x64\mt.exe' -nologo -manifest "$root\verification\twinkle-poc\quick-menu-probe.exe.manifest" "-outputresource:$destination;#1"
 if($LASTEXITCODE -ne 0){throw 'manifest failed'}
}
$probe="$root\verification\menu-residue-lifecycle-final.exe"
Stage-Probe 'menu_lifecycle_probe' $probe
$env:SAYALL_GATT_LOG="$root\verification\menu-residue-final-diagnostic.log"
$p=Start-Process $probe -ArgumentList --exercise -PassThru -RedirectStandardOutput "$root\verification\menu-residue-final-native.log" -RedirectStandardError "$root\verification\menu-residue-final-error.log"
$null=$p.Handle
if(!$p.WaitForExit(25000)){throw 'lifecycle probe timed out'}
$lines=Get-Content "$root\verification\menu-residue-final-native.log"
$lines
if(($lines -match '^startup menu_absent=true$').Count -ne 1){throw 'Startup menu window exists'}
if(($lines -match '^open .*ready=true native_state=Some\(\(true, true\)\) reused=true$').Count -ne 2){throw 'Native shown/tool/reuse check failed'}
if(($lines -match '^cancel .*native_hidden_tool=true native_state=Some\(\(false, true\)\)$').Count -ne 2){throw 'Native hide readback failed'}
if(($lines -match '^native_close .*requested=true$').Count -ne 1){throw 'Native close not exercised'}
if(($lines -match '^cancel_before_load native_hidden_tool=true$').Count -ne 1){throw 'Cancelled load resurrected menu'}
Add-Type @'
using System;
using System.Runtime.InteropServices;
public class ResidueMouse {
 public struct POINT { public int X,Y; }
 public struct RECT { public int L,T,R,B; }
 public delegate bool Proc(IntPtr h,IntPtr l);
 [DllImport("user32.dll")] public static extern bool EnumWindows(Proc p,IntPtr l);
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h,out RECT r);
 [DllImport("user32.dll")] public static extern bool GetPhysicalCursorPos(out POINT p);
 [DllImport("user32.dll")] public static extern bool SetPhysicalCursorPos(int x,int y);
 [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr c);
 [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(POINT p);
 [DllImport("user32.dll")] public static extern IntPtr GetAncestor(IntPtr h,uint f);
 public static bool PointBelongsTo(uint wanted, int x,int y){
  var previous=SetThreadDpiAwarenessContext(new IntPtr(-4));
  try { uint p;GetWindowThreadProcessId(GetAncestor(WindowFromPoint(new POINT { X=x,Y=y }),2),out p);return p==wanted; }
  finally { SetThreadDpiAwarenessContext(previous); }
 }
 [DllImport("user32.dll")] public static extern void mouse_event(uint f,uint x,uint y,uint d,UIntPtr e);
 [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
 public static bool Find(uint wanted,out RECT r) {
  RECT result=new RECT();bool found=false;
  var previous=SetThreadDpiAwarenessContext(new IntPtr(-4));
  try { EnumWindows((h,l)=> {uint p;RECT a;GetWindowThreadProcessId(h,out p);
   if(p==wanted && IsWindowVisible(h) && GetWindowRect(h,out a) && a.R-a.L>=200 && a.R-a.L<=380 && a.B-a.T>=200 && a.B-a.T<=400){result=a;found=true;return false;}return true;
  },IntPtr.Zero); } finally { SetThreadDpiAwarenessContext(previous); }
  r=result;return found;
 }
}
'@
$oldDpi=[ResidueMouse]::SetThreadDpiAwarenessContext([IntPtr](-4))
$probe="$root\verification\menu-residue-mouse-final.exe"
Stage-Probe 'quick_menu_overlay_probe' $probe
$env:SAYALL_GATT_LOG="$root\verification\menu-residue-mouse-diagnostic.log"
$stdout="$root\verification\menu-residue-mouse-native.log"
$p=Start-Process $probe -ArgumentList --exercise -PassThru -RedirectStandardOutput $stdout -RedirectStandardError "$root\verification\menu-residue-mouse-error.log"
$null=$p.Handle
$clicked=$false
$preserved=$false
$until=[DateTime]::UtcNow.AddSeconds(25)
while(!$p.HasExited -and [DateTime]::UtcNow -lt $until){
 $lines=@(Get-Content $stdout -ErrorAction SilentlyContinue)
 if(!$clicked -and ($lines -match 'generation=900003 ready=true visible=true').Count -gt 0){
  Start-Sleep -Milliseconds 650
  $r=New-Object 'ResidueMouse+RECT'
  if([ResidueMouse]::Find([uint32]$p.Id,[ref]$r)){
   $x=[int](($r.L+$r.R)/2);$y=[int](($r.T+$r.B)/2)
   $hit=[ResidueMouse]::PointBelongsTo([uint32]$p.Id,$x,$y)
   "center_hit_is_probe=$hit ring_hit_is_probe=$([ResidueMouse]::PointBelongsTo([uint32]$p.Id,$x,[int]($r.T+($r.B-$r.T)*0.15)))"
   if(!$hit){throw 'Center hit is not the probe; no mouse input injected'}
   $old=New-Object 'ResidueMouse+POINT';[void][ResidueMouse]::GetPhysicalCursorPos([ref]$old)
   $fg=[ResidueMouse]::GetForegroundWindow()
   try {
    [void][ResidueMouse]::SetPhysicalCursorPos($x,$y)
    $actual=New-Object 'ResidueMouse+POINT';[void][ResidueMouse]::GetPhysicalCursorPos([ref]$actual)
    $actualHit=[ResidueMouse]::PointBelongsTo([uint32]$p.Id,$actual.X,$actual.Y)
    "cursor_at_target=$($actual.X -eq $x -and $actual.Y -eq $y) cursor_hit_is_probe=$actualHit"
    if(!$actualHit){throw 'Actual cursor does not hit probe; no mouse input injected'}
    [ResidueMouse]::mouse_event(2,0,0,0,[UIntPtr]::Zero);[ResidueMouse]::mouse_event(4,0,0,0,[UIntPtr]::Zero)
    $clicked=$true;Start-Sleep -Milliseconds 250
    $preserved=$fg -eq [ResidueMouse]::GetForegroundWindow()
   } finally { [void][ResidueMouse]::SetPhysicalCursorPos($old.X,$old.Y) }
  }
 }
 Start-Sleep -Milliseconds 50
 $p.Refresh()
}
if(!$p.WaitForExit(3000)){throw 'mouse probe timed out'}
$lines=Get-Content $stdout
$lines
if(($lines -match 'ready=true visible=true.*reused=true anchored_above_clock=true').Count -ne 3){throw 'Menu readiness/reuse/anchor regression'}
if(($lines -match 'mouse_cancel_received=true click_focus_preserved=true').Count -ne 1){throw 'Mouse cancellation/focus regression'}
if(($lines -match 'hidden=true').Count -ne 4){throw 'Menu hide/cancel regression'}
if(!$clicked -or !$preserved){throw 'Mouse not exercised or focus changed'}
[void][ResidueMouse]::SetThreadDpiAwarenessContext($oldDpi)
'native_lifecycle_passed=true mouse_cancel_passed=true focus_preserved=true'
