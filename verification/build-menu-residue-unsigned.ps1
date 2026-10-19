$ErrorActionPreference='Stop'
[Console]::OutputEncoding=[System.Text.Encoding]::UTF8
$root='D:\Project\sayall'
$repo=Join-Path $root 'remote-mic-app-windows'
Set-Location $repo
$env:PATH='C:\Users\Administrator\.cargo\bin;'+$env:PATH
# Staging and dist-build already completed in the preceding build. Only disable
# updater artifacts for this unsigned LOCAL test package; do not change product
# config, public updater keys or install any signing credentials.
(Get-Item "$repo\src-tauri\src\lib.rs").LastWriteTime=Get-Date
$ErrorActionPreference='Continue'
& pnpm tauri build --bundles nsis --ci --config src-tauri/tauri.local-build.conf.json --config "$root\verification\menu-residue-unsigned.conf.json" *> "$root\verification\menu-residue-unsigned-package.log"
$code=$LASTEXITCODE
$ErrorActionPreference='Stop'
if($code -ne 0){throw 'Unsigned LOCAL package build failed'}
& powershell -NoProfile -ExecutionPolicy Bypass -File "$repo\scripts\verify-frontend-embedded.ps1" -Executable "$repo\target\release\sayall-windows-app.exe" -FrontendDirectory "$repo\dist-build" | Out-File -Encoding utf8 "$root\verification\menu-residue-embedded.log"
if($LASTEXITCODE -ne 0){throw 'Embedded assets verification failed'}
$folder=Join-Path $root 'builds\menu-residue-fixed'
New-Item -ItemType Directory $folder -Force | Out-Null
$setup=Get-ChildItem "$repo\target\release\bundle\nsis" -Filter '*.exe' | Sort-Object LastWriteTime -Descending | Select-Object -First 1
$destination=Join-Path $folder 'SayAll_0.8.6_menu-residue-fixed_x64-setup.exe'
Copy-Item $setup.FullName $destination -Force
Get-FileHash $destination -Algorithm SHA256 | Select-Object Hash | ConvertTo-Json -Compress | Set-Content "$root\verification\menu-residue-package-sha.json" -Encoding UTF8
$ErrorActionPreference='Continue'
& cargo build --release -p sayall-windows-app --features tauri/custom-protocol --example quick_menu_overlay_probe --example menu_lifecycle_probe *> "$root\verification\menu-residue-release-probes-build.log"
$code=$LASTEXITCODE
$ErrorActionPreference='Stop'
if($code -ne 0){throw 'release probe build failed'}
'build_completed=true unsigned_local_only=true' | Set-Content "$root\verification\menu-residue-build-result.txt" -Encoding UTF8
