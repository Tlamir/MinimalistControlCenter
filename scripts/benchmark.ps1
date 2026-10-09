param(
    [string]$Executable = (Join-Path $PSScriptRoot '..\target\release\mcc.exe'),
    [ValidateRange(1,3600)][int]$Seconds = 60,
    [string]$OutputDirectory = (Join-Path $PSScriptRoot '..\reports\local')
)
$ErrorActionPreference = 'Stop'
$Executable = (Resolve-Path -LiteralPath $Executable).Path
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
# These commands only query documented OS counters or run an offline calculation.
$policy = & $Executable bench --iterations 1000000
if ($LASTEXITCODE -ne 0) { throw 'Policy benchmark failed' }
$policy | Set-Content -LiteralPath (Join-Path $OutputDirectory 'policy.json') -Encoding utf8
$warmups = 3
$runs = 30
$durations = @()
for ($i=0; $i -lt ($runs+$warmups); $i++) {
    $timer = [Diagnostics.Stopwatch]::StartNew()
    $status = & $Executable status --json
    if ($LASTEXITCODE -ne 0) { throw 'Status command failed' }
    $timer.Stop()
    $parsed = ($status -join "`n") | ConvertFrom-Json
    if ($parsed.hardware_control_available -ne $false) { throw 'Unexpected control capability' }
    if ($i -ge $warmups) { $durations += $timer.Elapsed.TotalMilliseconds }
}
$sorted = @($durations | Sort-Object)
[ordered]@{
    benchmark = 'warm_cli_status_process_wall_time'
    runs = $runs
    warmups_discarded = $warmups
    median_ms = ($sorted[14] + $sorted[15]) / 2
    p95_ms = $sorted[28]
    min_ms = $sorted[0]
    max_ms = $sorted[-1]
    executable_bytes = (Get-Item -LiteralPath $Executable).Length
    sha256 = (Get-FileHash -LiteralPath $Executable -Algorithm SHA256).Hash.ToLowerInvariant()
    note = 'Includes PowerShell launch/capture overhead. Warm CLI latency, not GUI or cold-boot latency. No OEM baseline.'
} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $OutputDirectory 'startup.json') -Encoding utf8
# Redirect to a file rather than rendering 600 console lines. Last line is summary.
& $Executable watch --seconds $Seconds --interval-ms 1000 --json |
    Set-Content -LiteralPath (Join-Path $OutputDirectory 'watch.jsonl') -Encoding utf8
if ($LASTEXITCODE -ne 0) { throw 'Watch benchmark failed' }
Get-Content -LiteralPath (Join-Path $OutputDirectory 'watch.jsonl') -Tail 1 |
    Set-Content -LiteralPath (Join-Path $OutputDirectory 'watch-summary.json') -Encoding utf8
Write-Output "Benchmark reports saved under $OutputDirectory"
