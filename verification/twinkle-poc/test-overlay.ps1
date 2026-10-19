$ErrorActionPreference='Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName System.Drawing
$repo='D:\Project\sayall\remote-mic-app-windows'
$probe=Join-Path $PSScriptRoot 'brightness_overlay_probe.exe'
Copy-Item "$repo\target\release\examples\brightness_overlay_probe.exe" $probe -Force
& 'C:\Program Files (x86)\Windows Kits\10\bin\10.0.19041.0\x64\mt.exe' -nologo -manifest "$probe.manifest" "-outputresource:$probe;#1"
if($LASTEXITCODE -ne 0){throw 'Probe manifest embedding failed'}
$p=Start-Process $probe -ArgumentList --exercise -PassThru -RedirectStandardOutput (Join-Path $PSScriptRoot 'overlay-native.log') -RedirectStandardError (Join-Path $PSScriptRoot 'overlay-native-error.log')
$null=$p.Handle
$found=$false
$hostFound=$false
$captured=$false
$started=[DateTime]::UtcNow
$until=[DateTime]::UtcNow.AddSeconds(8)
while(!$p.HasExited -and [DateTime]::UtcNow -lt $until) {
    try {
        $condition=New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ProcessIdProperty,[int]$p.Id)
        $window=[System.Windows.Automation.AutomationElement]::RootElement.FindFirst([System.Windows.Automation.TreeScope]::Children,$condition)
        if($window) {
            $hostFound=$true
            if(!$captured -and $found -and !$window.Current.IsOffscreen -and ([DateTime]::UtcNow-$started).TotalMilliseconds -gt 1100) {
                $rect=$window.Current.BoundingRectangle
                if($rect.Width -gt 0 -and $rect.Height -gt 0){
                    $image=New-Object Drawing.Bitmap([int]$rect.Width,[int]$rect.Height)
                    $graphics=[Drawing.Graphics]::FromImage($image)
                    try { $graphics.CopyFromScreen([int]$rect.X,[int]$rect.Y,0,0,$image.Size);$image.Save((Join-Path $PSScriptRoot 'target-overlay.png'),[Drawing.Imaging.ImageFormat]::Png);$captured=$true } finally {$graphics.Dispose();$image.Dispose()}
                }
            }
            # This machine's known controllable display. Only the boolean result is recorded.
            $target=New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty,'S82')
            if($window.FindFirst([System.Windows.Automation.TreeScope]::Descendants,$target)){$found=$true}
            $documentCondition=New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty,[System.Windows.Automation.ControlType]::Document)
            $documents=$window.FindAll([System.Windows.Automation.TreeScope]::Descendants,$documentCondition)
            foreach($doc in $documents){try{$text=$doc.GetCurrentPattern([System.Windows.Automation.TextPattern]::Pattern).DocumentRange.GetText(-1);if($text.Contains('S82')){$found=$true}}catch{}}
        }
    } catch { }
    Start-Sleep -Milliseconds 100
    $p.Refresh()
}
if(!$p.WaitForExit(12000)){throw 'Probe did not exit normally'}
Get-Content (Join-Path $PSScriptRoot 'overlay-native.log')
Get-Content (Join-Path $PSScriptRoot 'overlay-native-error.log')
"overlay host_window_found=$hostFound target_text_confirmed=$found screenshot=$captured exit=$($p.ExitCode)"
if($p.ExitCode -ne 0){throw "Overlay probe failed with exit $($p.ExitCode)"}
if(!$found){throw 'Overlay window lifecycle passed but target text was not confirmed'}
