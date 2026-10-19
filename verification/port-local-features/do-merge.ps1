$ErrorActionPreference = 'Stop'
$nf = 'D:\Project\sayall\nfc-transfer'
$rm = 'D:\Project\sayall\remote-mic-app-windows'
$work = 'D:\Project\sayall\verification\port-local-features'
New-Item -ItemType Directory -Force -Path $work | Out-Null
Set-Location $nf

$branch = git branch --show-current
if ($branch -ne 'feat/nfc-phone-transfer') { throw "unexpected branch: $branch" }

$utf8 = [System.Text.UTF8Encoding]::new($false)
function Read-Normalized([string]$path) {
    if (-not (Test-Path $path)) { return '' }
    $text = [System.IO.File]::ReadAllText($path, [System.Text.Encoding]::UTF8)
    $text = $text.Replace("`r`n", "`n").Replace("`r", "`n")
    if ($text.Length -gt 0 -and -not $text.EndsWith("`n")) { $text += "`n" }
    return $text
}
function Write-Text([string]$path, [string]$text) {
    [System.IO.File]::WriteAllText($path, $text, $utf8)
}

$report = @()
$conflicts = @()

$changed = @(git -C $rm diff HEAD --name-only)
$added = @(git -C $rm diff HEAD --name-status | Where-Object { $_ -match '^A' } | ForEach-Object { ($_ -split "`t")[1] })
$tracked = @($changed | Where-Object { $_ -match '\.(rs|ts|vue|md|toml|html|js|json|ps1|py)$' } | Select-Object -Unique)

foreach ($rel in $tracked) {
    $d = Join-Path $work 'merge'
    New-Item -ItemType Directory -Force -Path $d | Out-Null
    $base = Join-Path $d 'base'
    $theirs = Join-Path $d 'theirs'
    $ours = Join-Path $d 'ours'
    $outLog = Join-Path $d 'out.log'

    if ($added -contains $rel) {
        $target = Join-Path $nf $rel
        New-Item -ItemType Directory -Force -Path (Split-Path -Parent $target) | Out-Null
        Copy-Item (Join-Path $rm $rel) $target -Force
        $report += [pscustomobject]@{ file = $rel; action = 'added'; conflicts = 0 }
        continue
    }

    $baseText = ''
    try { $baseText = (& git -C $rm show ("HEAD:" + $rel) 2>$null | Out-String) } catch { $baseText = '' }
    $baseText = $baseText.Replace("`r`n", "`n").Replace("`r", "`n")
    Write-Text $base $baseText
    Write-Text $theirs (Read-Normalized (Join-Path $rm $rel))
    $ourFile = Join-Path $nf $rel
    Write-Text $ours (Read-Normalized $ourFile)

    if (Test-Path $outLog) { Remove-Item $outLog -Force }
    if (Test-Path ($outLog + '.err')) { Remove-Item ($outLog + '.err') -Force }
    $proc = Start-Process -FilePath 'git' -ArgumentList @('merge-file', '-p', '--diff3', $ours, $base, $theirs) -NoNewWindow -Wait -PassThru -RedirectStandardOutput $outLog -RedirectStandardError ($outLog + '.err')
    $merged = ''
    if (Test-Path $outLog) { $merged = [System.IO.File]::ReadAllText($outLog, [System.Text.Encoding]::UTF8) }
    if ($null -eq $merged) { $merged = '' }
    $c = ([regex]::Matches($merged, '(?m)^<<<<<<< ')).Count
    if ($proc.ExitCode -eq 255 -or $merged -eq '') {
        $report += [pscustomobject]@{ file = $rel; action = 'merge_error'; conflicts = -1 }
        continue
    }
    Write-Text $ourFile $merged
    $report += [pscustomobject]@{ file = $rel; action = 'merged'; conflicts = $c }
    if ($c -gt 0) { $conflicts += [pscustomobject]@{ file = $rel; conflicts = $c } }
}

$untracked = @(git -C $rm status --porcelain | Where-Object { $_ -match '^\?\?' } | ForEach-Object { $_.Substring(3) })
foreach ($rel in $untracked) {
    $target = Join-Path $nf $rel
    if (Test-Path $target) {
        $report += [pscustomobject]@{ file = $rel; action = 'untracked_exists_skipped'; conflicts = 0 }
        continue
    }
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $target) | Out-Null
    Copy-Item (Join-Path $rm $rel) $target -Force
    $report += [pscustomobject]@{ file = $rel; action = 'untracked_copied'; conflicts = 0 }
}

$report | Export-Csv -Path (Join-Path $work 'merge-report.csv') -NoTypeInformation -Encoding UTF8
$conflicts | Export-Csv -Path (Join-Path $work 'conflicts.csv') -NoTypeInformation -Encoding UTF8

Write-Output ("=== files processed: " + $report.Count + "; conflict files: " + $conflicts.Count + " ===")
if ($conflicts.Count -gt 0) { $conflicts | Format-Table -AutoSize | Out-String -Width 160 | Write-Output }
Write-Output "=== merge errors ==="
$report | Where-Object { $_.action -eq 'merge_error' } | Format-Table -AutoSize | Out-String -Width 160 | Write-Output
Write-Output ("REPORT=" + (Join-Path $work 'merge-report.csv'))
