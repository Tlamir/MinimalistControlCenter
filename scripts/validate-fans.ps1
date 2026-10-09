param([Parameter(Mandatory=$true)][string]$Binary,
      [Parameter(Mandatory=$true)][string]$Output,
      [switch]$Handover)
# Run as administrator. Only a short BOOST -> original AUTO transaction is permitted.
$ErrorActionPreference='Stop'
$Binary=(Resolve-Path -LiteralPath $Binary).Path
function Invoke-Typed([string]$Action) {
    $start=Get-Date
    $reply=& $Binary hardware $Action | ConvertFrom-Json
    if (-not $reply.ok) { throw $reply.error }
    @{reply=$reply; elapsed_ms=((Get-Date)-$start).TotalMilliseconds}
}
$result=@{test='firmware_boost_then_auto'; writes=@(); physical_response='Requires observation; mode readback alone is insufficient'}
$restore=$false
$restoreOem=$false
try {
    $before=Invoke-Typed 'status'
    $result.before=$before
    if (-not $before.reply.status.writes_allowed -and $Handover -and $before.reply.status.ownership_reason -like 'OEM service GamingCenter is running*') {
        $result.pause_oem=Invoke-Typed 'pause-oem'
        $restoreOem=$true
        $before=Invoke-Typed 'status'
        $result.after_handover=$before
    }
    if (-not $before.reply.status.writes_allowed) {throw $before.reply.status.ownership_reason}
    if ($before.reply.status.capabilities.fan_mode_raw -ne 0) {throw 'Original fan mode is not AUTO; test refuses to replace it'}
    if (-not $before.reply.status.capabilities.boost) {throw 'Boost capability is absent'}
    $restore=$true
    $result.boost=Invoke-Typed 'fan-boost'
    $result.writes+=64
    Start-Sleep -Seconds 5
    $result.during=Invoke-Typed 'status'
    if ($result.during.reply.status.capabilities.fan_mode_raw -ne 64) {throw 'Boost did not persist for the test interval'}
} catch {$result.error=$_.Exception.Message} finally {
    if ($restore) {
        try { $result.restore=Invoke-Typed 'fan-auto'; $result.writes+=0; $result.after=Invoke-Typed 'status'; $result.auto_confirmed=($result.after.reply.status.capabilities.fan_mode_raw -eq 0) }
        catch {$result.restore_error=$_.Exception.Message; $result.auto_confirmed=$false}
    }
    if ($restoreOem) {
        try {$result.resume_oem=Invoke-Typed 'resume-oem'}catch {$result.resume_error=$_.Exception.Message}
    }
    $result.success=($result.ContainsKey('boost') -and $result.ContainsKey('during') -and $result.auto_confirmed -and -not $result.ContainsKey('error'))
    $result | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $Output -Encoding UTF8
}
