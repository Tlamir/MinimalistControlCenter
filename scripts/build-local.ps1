param([switch]$Release, [switch]$Test)
$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path $PSScriptRoot -Parent
Push-Location $taskRoot
try {
    $env:CARGO_HOME=Join-Path $taskRoot '.tools\cargo'
    $env:RUSTUP_HOME=Join-Path $taskRoot '.tools\rustup'
    $toolbin=Join-Path $taskRoot '.tools\rustup\toolchains\stable-x86_64-pc-windows-gnullvm\bin'
    $env:PATH="$toolbin;$env:PATH"
    $env:RUSTC=Join-Path $toolbin 'rustc.exe'
    $env:RUSTFLAGS='-C target-feature=+crt-static -L native='+((Join-Path $taskRoot '.tools/windows-imports').Replace('\','/'))
    $cargoArgs=@('build','--all-targets','--locked')
    if ($Test) { $cargoArgs[0]='test' }
    if ($Release) { $cargoArgs+='--release' }
    & (Join-Path $toolbin 'cargo.exe') @cargoArgs
    if ($LASTEXITCODE -ne 0) { throw "Cargo failed: $LASTEXITCODE" }
} finally { Pop-Location }
