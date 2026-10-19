# Resolve ATTRIBUTION.md / TODO.md conflict blocks by splicing (UTF-8 no BOM, LF preserved).
$ErrorActionPreference = 'Stop'
$enc = [System.Text.UTF8Encoding]::new($false)

function Resolve-Doc([string]$path, [string]$introLine) {
    $text = [System.IO.File]::ReadAllText($path, [System.Text.Encoding]::UTF8)
    if ($text.Contains("`r")) { throw ("CR line endings in " + $path) }
    if ($text.Contains([char]0xFEFF)) { throw ("BOM in " + $path) }
    $lines = [System.Collections.Generic.List[string]]::new()
    $lines.AddRange([string[]]($text -split "`n"))
    $start = -1; $base = -1; $sep = -1; $end = -1
    for ($i = 0; $i -lt $lines.Count; $i++) {
        $l = $lines[$i]
        if ($l.StartsWith('<<<<<<< ')) { $start = $i }
        elseif ($l.StartsWith('||||||| ') -and $start -ge 0) { $base = $i }
        elseif (($l -ceq '=======') -and $base -ge 0 -and $sep -lt 0) { $sep = $i }
        elseif ($l.StartsWith('>>>>>>> ') -and $sep -ge 0) { $end = $i }
    }
    if ($start -lt 0 -or $base -lt 0 -or $sep -lt 0 -or $end -lt 0) { throw ("markers not found in " + $path) }
    $head = $lines.GetRange(0, $start)
    $theirs = $lines.GetRange($sep + 1, $end - $sep - 1)
    $tail = $lines.GetRange($end + 1, $lines.Count - $end - 1)
    while ($theirs.Count -gt 0 -and $theirs[$theirs.Count - 1] -eq '') { $theirs.RemoveAt($theirs.Count - 1) }
    $tailStart = 0
    while ($tailStart -lt $tail.Count -and $tail[$tailStart] -eq '') { $tailStart++ }
    $tail = $tail.GetRange($tailStart, $tail.Count - $tailStart)
    $out = [System.Collections.Generic.List[string]]::new()
    $out.AddRange($head)
    if ($introLine) { $out.Add($introLine); $out.Add('') }
    $out.AddRange($theirs)
    $out.Add('')
    $out.AddRange($tail)
    $outText = ($out -join "`n")
    if (-not $outText.EndsWith("`n")) { $outText += "`n" }
    [System.IO.File]::WriteAllText($path, $outText, $enc)
    return @{ head = $head.Count; theirs = $theirs.Count; tail = $tail.Count; total = $out.Count }
}

$r1 = Resolve-Doc 'D:\Project\sayall\nfc-transfer\ATTRIBUTION.md' '本仓库是面向 Windows 的 Rust/Tauri 工程。'
Write-Output ("ATTRIBUTION.md: head={0} theirs={1} tail={2} totalLines={3}" -f $r1.head, $r1.theirs, $r1.tail, $r1.total)
$r2 = Resolve-Doc 'D:\Project\sayall\nfc-transfer\TODO.md' $null
Write-Output ("TODO.md: head={0} theirs={1} tail={2} totalLines={3}" -f $r2.head, $r2.theirs, $r2.tail, $r2.total)
