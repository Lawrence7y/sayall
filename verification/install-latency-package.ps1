$ErrorActionPreference='Stop'
$root='D:\Project\sayall'
$repo=Join-Path $root 'remote-mic-app-windows'
$setup=Join-Path $root 'builds\latency-optimized\SayAll_0.8.6_latency-optimized_x64-setup.exe'
$install=(Get-Content "$root\verification\latency-install-directory.txt" -Encoding UTF8 -Raw).Trim()
$backup=(Get-Content "$root\verification\latency-install-backup.txt" -Encoding UTF8 -Raw).Trim()
$config=Join-Path $env:APPDATA 'app.getsayall.remote-mic.windows'
if(Test-Path $config){Copy-Item $config (Join-Path $backup 'configuration') -Recurse}
$before=@{}
if(Test-Path $config){Get-ChildItem $config -File -Recurse -Filter '*.json' | ForEach-Object {$before[$_.FullName]=(Get-FileHash $_.FullName -Algorithm SHA256).Hash}}
if((Get-FileHash $setup -Algorithm SHA256).Hash -ne 'D6334DE5F70093B42FF67E996772333F52442F44DDDCC1B0FD5554560CF90415'){throw 'Package checksum mismatch'}
if(Get-Process sayall-windows-app -ErrorAction SilentlyContinue){throw 'Old app is still running'}
$p=Start-Process $setup -ArgumentList "/S /D=$install" -PassThru
$null=$p.Handle
if(!$p.WaitForExit(120000)){throw 'Installer did not complete in time'}
"installer_exit=$($p.ExitCode)"
Start-Sleep -Seconds 3
$exe=Join-Path $install 'sayall-windows-app.exe'
$diff=(& python "$repo\scripts\verify-binary-markers.py" diff $exe "$repo\target\release\sayall-windows-app.exe") | Out-String
"app_payload=$($diff.Trim())"
if($LASTEXITCODE -ne 0 -or $diff -notmatch 'differing_bytes=(0|3)(\s|$)'){throw 'Installed app does not match build'}
foreach($name in @('sayall-helper.exe','frida-gadget.dll')){
    $ok=(Get-FileHash (Join-Path $install $name) -Algorithm SHA256).Hash -eq (Get-FileHash (Join-Path "$repo\src-tauri" $name) -Algorithm SHA256).Hash
    "$name payload_match=$ok"
    if(!$ok){throw 'Installed helper/gadget does not match build'}
}
foreach($name in @('settings.json','button-mappings.json','app-settings.json')){
    $path=Join-Path $config $name
    if($before.ContainsKey($path)){
        $ok=(Test-Path $path) -and (Get-FileHash $path -Algorithm SHA256).Hash -eq $before[$path]
        "$name configuration_preserved=$ok"
        if(!$ok){throw 'User configuration changed during install'}
    }
}
$markers=@('sayall-diagnostic-writer','waiting_for_preload','reused=true elapsed_ms=')
foreach($marker in $markers){
    $count=[int](& python "$repo\scripts\verify-binary-markers.py" count $exe $marker)
    "optimization_marker_present=$($count -gt 0)"
    if($count -lt 1){throw 'Optimization marker missing'}
}
$start=[DateTime]::UtcNow
$app=Start-Process $exe -PassThru
"launched_pid=$($app.Id)"
Start-Sleep -Seconds 15
$running=@(Get-CimInstance Win32_Process | Where-Object {$_.Name -eq 'sayall-windows-app.exe' -and $_.ExecutablePath -eq $exe})
"installed_process_count=$($running.Count)"
if($running.Count -lt 1){throw 'Installed app failed to stay running'}
$diag=Join-Path $env:LOCALAPPDATA 'SayAll\Logs\sayall-diagnostic.log'
$lines=Get-Content $diag -Tail 350 -Encoding UTF8
$newLines=@($lines | Where-Object {$_ -match ('pid='+$app.Id+'\s')})
$setupOk=@($newLines | Where-Object {$_ -match 'tauri_setup.*completed'}).Count -gt 0
$vueOk=@($newLines | Where-Object {$_ -match 'vue_mount.*completed|Vue.*completed'}).Count -gt 0
"startup_setup_observed=$setupOk"
"startup_vue_observed=$vueOk"
if(!$setupOk){throw 'No setup completion evidence for launched app'}
$newLines | Where-Object {$_ -match 'tauri_setup|vue_mount|quick_menu_overlay|runtime_snapshot|ble_connect_stage.*phase=completed'} | Select-Object -Last 12
"install_verified=true"
