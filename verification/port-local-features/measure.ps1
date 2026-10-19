$ErrorActionPreference = 'Stop'
$nf = 'D:\Project\sayall\nfc-transfer'
$rm = 'D:\Project\sayall\remote-mic-app-windows'
$work = 'D:\Project\sayall\verification\port-local-features'
New-Item -ItemType Directory -Force -Path $work | Out-Null

function Merge-Count([string]$baseFile, [string]$oursFile, [string]$theirsFile, [string]$outFile) {
    $p = Start-Process -FilePath 'git' -ArgumentList @('merge-file', '-p', '--diff3', $oursFile, $baseFile, $theirsFile) -NoNewWindow -Wait -PassThru -RedirectStandardOutput $outFile -RedirectStandardError ($outFile + '.err')
    $content = ''
    if (Test-Path $outFile) { $content = Get-Content $outFile -Raw -ErrorAction SilentlyContinue }
    if ($null -eq $content) { $content = '' }
    return ([regex]::Matches($content, '(?m)^<<<<<<< ')).Count
}

# ---------- Direction X: new feature (05c37cf..HEAD) onto old working tree ----------
$featureFiles = @(git -C $nf diff --name-only '05c37cf..HEAD' | Where-Object { $_ -match '\.(rs|ts|vue|md|toml|html|js|json|ps1|py)$' })
$totalX = 0
$confX = @()
foreach ($rel in $featureFiles) {
    $d = Join-Path $work 'x'
    New-Item -ItemType Directory -Force -Path $d | Out-Null
    $base = Join-Path $d 'base'; $theirs = Join-Path $d 'theirs'; $ours = Join-Path $d 'ours'; $out = Join-Path $d 'out'
    git -C $nf show "05c37cf:$rel" 2>$null | Set-Content -Path $base -Encoding UTF8
    git -C $nf show "HEAD:$rel" 2>$null | Set-Content -Path $theirs -Encoding UTF8
    $remoteFile = Join-Path $rm $rel
    if (Test-Path $remoteFile) { Copy-Item $remoteFile $ours -Force } else { Set-Content -Path $ours -Value '' -Encoding UTF8 }
    $c = Merge-Count $base $ours $theirs $out
    $totalX += $c
    if ($c -gt 0) { $confX += [pscustomobject]@{ file = $rel; conflicts = $c } }
}
Write-Output ("=== Direction X (new feature -> old tree): files=" + $featureFiles.Count + " conflict_hunks=" + $totalX)
$confX | Sort-Object conflicts -Descending | Select-Object -First 25 | Format-Table -AutoSize | Out-String | Write-Output

# ---------- Direction Y: old working-tree features onto new tree ----------
$tracked = @(git -C $rm diff HEAD --name-only | Where-Object { $_ -match '\.(rs|ts|vue|md|toml|html|js|json|ps1|py)$' })
$totalY = 0
$confY = @()
foreach ($rel in $tracked) {
    $d = Join-Path $work 'y'
    New-Item -ItemType Directory -Force -Path $d | Out-Null
    $base = Join-Path $d 'base'; $theirs = Join-Path $d 'theirs'; $ours = Join-Path $d 'ours'; $out = Join-Path $d 'out'
    git -C $rm show "HEAD:$rel" 2>$null | Set-Content -Path $base -Encoding UTF8
    Copy-Item (Join-Path $rm $rel) $theirs -Force
    $ourFile = Join-Path $nf $rel
    if (Test-Path $ourFile) { Copy-Item $ourFile $ours -Force } else { Set-Content -Path $ours -Value '' -Encoding UTF8 }
    $c = Merge-Count $base $ours $theirs $out
    $totalY += $c
    if ($c -gt 0) { $confY += [pscustomobject]@{ file = $rel; conflicts = $c } }
}
Write-Output ("=== Direction Y (old features -> new tree): files=" + $tracked.Count + " conflict_hunks=" + $totalY)
$confY | Sort-Object conflicts -Descending | Select-Object -First 25 | Format-Table -AutoSize | Out-String | Write-Output
