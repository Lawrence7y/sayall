$ErrorActionPreference='Stop'
[Console]::OutputEncoding=[System.Text.Encoding]::UTF8
$root='D:\Project\sayall'
$repo=Join-Path $root 'remote-mic-app-windows'
$setup="$root\builds\menu-residue-fixed\SayAll_0.8.6_menu-residue-fixed_x64-setup.exe"
$expected=(Get-Content "$root\verification\menu-residue-package-sha.json" -Encoding UTF8 -Raw | ConvertFrom-Json).Hash
if((Get-FileHash $setup -Algorithm SHA256).Hash -ne $expected){throw 'Package checksum mismatch'}
$install=(Get-Content "$root\verification\latency-install-directory.txt" -Encoding UTF8 -Raw).Trim()
$exe=Join-Path $install 'sayall-windows-app.exe'
if(!(Test-Path $exe)){throw 'Existing install target not found'}
$backup=Join-Path $root ('backups\pre-menu-residue-install-'+(Get-Date -Format yyyyMMdd-HHmmss))
New-Item -ItemType Directory $backup | Out-Null
Copy-Item $install (Join-Path $backup 'installed') -Recurse
$backup | Set-Content "$root\verification\menu-residue-install-backup.txt" -Encoding UTF8
# Request normal application shutdown; never force-kill BLE or the helper.
$running=@(Get-CimInstance Win32_Process | Where-Object {$_.ExecutablePath -eq $exe})
if($running.Count -gt 0){
 $event=[System.Threading.EventWaitHandle]::OpenExisting('Local\SayAll-GracefulExit')
 try{[void]$event.Set()}finally{$event.Dispose()}
 $until=[DateTime]::UtcNow.AddSeconds(25)
 do {
  Start-Sleep -Milliseconds 250
  $remaining=@(Get-CimInstance Win32_Process | Where-Object {$_.ExecutablePath -eq $exe})
 }while($remaining.Count -gt 0 -and [DateTime]::UtcNow -lt $until)
 if($remaining.Count -gt 0){throw 'Normal shutdown did not finish; install aborted without force kill'}
}
'old_app_shutdown=graceful'
# The helper is intentionally resident across main-app exits. Use its public
# stop-marker contract before invoking NSIS, so no active task is force-ended.
$helperPath=Join-Path $install 'sayall-helper.exe'
$helpers=@(Get-CimInstance Win32_Process | Where-Object {$_.ExecutablePath -eq $helperPath})
if($helpers.Count -gt 0){
 $stop=Join-Path $env:LOCALAPPDATA 'SayAll\rc003-capture-stop'
 [System.IO.File]::WriteAllText($stop,'stop')
 $until=[DateTime]::UtcNow.AddSeconds(20)
 do { Start-Sleep -Milliseconds 250; $helpers=@(Get-CimInstance Win32_Process | Where-Object {$_.ExecutablePath -eq $helperPath}) }
 while($helpers.Count -gt 0 -and [DateTime]::UtcNow -lt $until)
 if($helpers.Count -gt 0){throw 'Helper stop marker was not consumed; install aborted without force kill'}
}
'old_helper_shutdown=stop_marker'
$config=Join-Path $env:APPDATA 'app.getsayall.remote-mic.windows'
$before=@{}
if(Test-Path $config){
 Copy-Item $config (Join-Path $backup 'configuration') -Recurse
 Get-ChildItem $config -File -Recurse -Filter '*.json' | ForEach-Object {$before[$_.FullName]=(Get-FileHash $_.FullName -Algorithm SHA256).Hash}
}
$p=Start-Process $setup -ArgumentList "/S /D=$install" -PassThru
$null=$p.Handle
if(!$p.WaitForExit(120000)){throw 'Installer did not complete in time'}
"installer_exit=$($p.ExitCode)"
if($p.ExitCode -ne 0){throw 'Installer rejected upgrade'}
Start-Sleep -Seconds 3
$diff=(& python "$repo\scripts\verify-binary-markers.py" diff $exe "$repo\target\release\sayall-windows-app.exe") | Out-String
"app_payload=$($diff.Trim())"
if($LASTEXITCODE -ne 0 -or $diff -notmatch 'differing_bytes=(0|3)(\s|$)'){throw 'Installed app does not match build'}
foreach($name in @('sayall-helper.exe','frida-gadget.dll')){
 $ok=(Get-FileHash (Join-Path $install $name) -Algorithm SHA256).Hash -eq (Get-FileHash "$repo\src-tauri\$name" -Algorithm SHA256).Hash
 "$name payload_match=$ok"
 if(!$ok){throw 'Installed helper/gadget does not match build'}
}
foreach($path in $before.Keys){
 if(!(Test-Path $path) -or (Get-FileHash $path -Algorithm SHA256).Hash -ne $before[$path]){throw 'Configuration changed during upgrade'}
}
"configuration_preserved=true files=$($before.Count)"
foreach($marker in @('phase=created_on_demand native_hidden=true','native_verified=','orphan_mouse_cancel','scope=global_hook_window')){
 $count=[int](& python "$repo\scripts\verify-binary-markers.py" count $exe $marker)
 if($count -lt 1){throw 'Required fix marker missing'}
}
'fix_markers_present=true'
Add-Type @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public class MenuStartupReadback {
 public delegate bool Proc(IntPtr h,IntPtr l);
 [DllImport("user32.dll")] public static extern bool EnumWindows(Proc p,IntPtr l);
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h,StringBuilder b,int n);
 public static bool HasMenu(uint wanted){
  bool found=false;
  EnumWindows((h,l)=>{uint p;GetWindowThreadProcessId(h,out p);if(p!=wanted)return true;
   var b=new StringBuilder(200);GetWindowText(h,b,200);
   if(b.ToString()=="\u56db\u5411\u5feb\u6377\u83dc\u5355"){found=true;return false;}return true;
  },IntPtr.Zero);return found;
 }
}
'@
$app=Start-Process $exe -PassThru
"launched_pid=$($app.Id)"
Start-Sleep -Seconds 15
if(!(Get-Process -Id $app.Id -ErrorAction SilentlyContinue)){throw 'Installed app exited unexpectedly'}
$menuAbsent=![MenuStartupReadback]::HasMenu([uint32]$app.Id)
"startup_native_menu_absent=$menuAbsent"
$diag=Join-Path $env:LOCALAPPDATA 'SayAll\Logs\sayall-diagnostic.log'
$lines=Get-Content $diag -Tail 600 -Encoding UTF8
$newLines=@($lines | Where-Object {$_ -match ('pid='+$app.Id+'\s')})
$setupOk=@($newLines | Where-Object {$_ -match 'tauri_setup.*completed'}).Count -gt 0
$vueOk=@($newLines | Where-Object {$_ -match 'vue_mount.*completed'}).Count -gt 0
"startup_setup_observed=$setupOk startup_vue_observed=$vueOk"
if(!$menuAbsent -or !$setupOk -or !$vueOk){throw 'Complete startup regression failed'}
$newLines | Where-Object {$_ -match 'tauri_setup|vue_mount|quick_menu_overlay'} | Select-Object -Last 8
'install_verified=true'
