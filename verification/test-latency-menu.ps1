$ErrorActionPreference='Stop'
$root='D:\Project\sayall'
$probe=Join-Path $root 'verification\latency-menu-probe.exe'
Copy-Item "$root\remote-mic-app-windows\target\release\examples\quick_menu_overlay_probe.exe" $probe -Force
& 'C:\Program Files (x86)\Windows Kits\10\bin\10.0.19041.0\x64\mt.exe' -nologo -manifest "$root\verification\twinkle-poc\quick-menu-probe.exe.manifest" "-outputresource:$probe;#1"
if($LASTEXITCODE -ne 0){throw 'manifest failed'}
Add-Type -AssemblyName UIAutomationClient
$env:SAYALL_GATT_LOG="$root\verification\latency-menu-diagnostic-final.log"
$p=Start-Process $probe -ArgumentList --exercise -PassThru -RedirectStandardOutput "$root\verification\latency-menu-native-final.log" -RedirectStandardError "$root\verification\latency-menu-native-error-final.log"
$null=$p.Handle
$seen=@{}
$until=[DateTime]::UtcNow.AddSeconds(30)
while(!$p.HasExited -and [DateTime]::UtcNow -lt $until){
    try {
        $condition=New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ProcessIdProperty,[int]$p.Id)
        $windows=[System.Windows.Automation.AutomationElement]::RootElement.FindAll([System.Windows.Automation.TreeScope]::Children,$condition)
        foreach($window in $windows){
            if($window.Current.IsOffscreen){continue}
            $documentCondition=New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty,[System.Windows.Automation.ControlType]::Document)
            $documents=$window.FindAll([System.Windows.Automation.TreeScope]::Descendants,$documentCondition)
            foreach($doc in $documents){
                try {
                    $text=$doc.GetCurrentPattern([System.Windows.Automation.TextPattern]::Pattern).DocumentRange.GetText(-1)
                    foreach($generation in 900001..900003){if($text.Contains("$generation")){$seen[$generation]=$true}}
                }catch{}
            }
        }
    }catch{}
    Start-Sleep -Milliseconds 50
    $p.Refresh()
}
if(!$p.WaitForExit(5000)){throw 'probe did not exit'}
$lines=Get-Content "$root\verification\latency-menu-native-final.log"
$lines
"native_labels_confirmed=$($seen.Count)"
# Judge the observed outcomes, not only the process exit code.
if(($lines -match 'ready=true visible=true focus_preserved=true reused=true').Count -ne 3){throw 'menu readiness/focus/reuse failed'}
if(($lines -match 'hidden=true').Count -ne 4){throw 'menu cancellation/hide failed'}
if($seen.Count -ne 3){throw 'menu label generations were not all observed'}
