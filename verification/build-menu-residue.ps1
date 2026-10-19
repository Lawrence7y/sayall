$ErrorActionPreference='Stop'
[Console]::OutputEncoding=[System.Text.Encoding]::UTF8
$root='D:\Project\sayall'
$repo=Join-Path $root 'remote-mic-app-windows'
Set-Location $repo
$env:PATH='C:\Users\Administrator\.cargo\bin;'+$env:PATH
$locked=@(Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -like "$repo\target\release\*" })
if($locked.Count -gt 0){throw 'A release build executable is still running; do not replace it'}
$ErrorActionPreference='Continue'
& powershell -NoProfile -ExecutionPolicy Bypass -File "$repo\scripts\build-local-test-package.ps1" *> "$root\verification\menu-residue-package.log"
$buildCode=$LASTEXITCODE
$ErrorActionPreference='Stop'
if($buildCode -ne 0){throw 'local package build failed'}
$folder=Join-Path $root 'builds\menu-residue-fixed'
New-Item -ItemType Directory $folder -Force | Out-Null
$setup=Get-ChildItem "$repo\target\release\bundle\nsis" -Filter '*.exe' | Sort-Object LastWriteTime -Descending | Select-Object -First 1
$destination=Join-Path $folder 'SayAll_0.8.6_menu-residue-fixed_x64-setup.exe'
Copy-Item $setup.FullName $destination -Force
Get-FileHash $destination -Algorithm SHA256 | Select-Object Hash | ConvertTo-Json -Compress | Set-Content "$root\verification\menu-residue-package-sha.json" -Encoding UTF8
$ErrorActionPreference='Continue'
& cargo build --release -p sayall-windows-app --example quick_menu_overlay_probe --example menu_lifecycle_probe *> "$root\verification\menu-residue-release-probes-build.log"
$buildCode=$LASTEXITCODE
$ErrorActionPreference='Stop'
if($buildCode -ne 0){throw 'release probe build failed'}
'build_completed=true' | Set-Content "$root\verification\menu-residue-build-result.txt" -Encoding UTF8
