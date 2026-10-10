param([string]$Source = $PSScriptRoot)
# Per-user portable installation. No elevation, driver, service or firmware changes.
$ErrorActionPreference = 'Stop'
$Source = (Resolve-Path -LiteralPath $Source).Path
foreach ($name in @('mcc.exe', 'mcc-ui.exe')) {
    if (-not (Test-Path -LiteralPath (Join-Path $Source $name) -PathType Leaf)) {
        throw "Missing $name. Extract the complete release archive first."
    }
}
$version = (& (Join-Path $Source 'mcc.exe') --version).Trim()
if ($LASTEXITCODE -ne 0 -or $version -notmatch '^mcc ([0-9]+\.[0-9]+\.[0-9]+)$') { throw 'Cannot identify release version' }
# AppData writes made by packaged terminals can be transparently redirected to
# their package cache, invisible to the UAC broker at the apparent path. Use a
# normal user-owned location outside Windows' package-virtualized known folders.
$folder = Join-Path $env:USERPROFILE ('Applications\MinimalistControlCenter\' + $Matches[1])
$folder = [IO.Path]::GetFullPath($folder)
$parent = [IO.Path]::GetFullPath((Join-Path $env:USERPROFILE 'Applications\MinimalistControlCenter'))
if (-not $folder.StartsWith($parent + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Unexpected installation path' }
foreach ($name in @('mcc.exe','mcc-ui.exe')) {
    $existing = Join-Path $folder $name
    if ((Test-Path -LiteralPath $existing -PathType Leaf) -and
        (Get-FileHash -LiteralPath $existing).Hash -ne (Get-FileHash -LiteralPath (Join-Path $Source $name)).Hash) {
        throw "A different build of this version already exists at $folder. Keep it intact and use a new release version."
    }
}
foreach ($p in Get-Process -Name 'mcc','mcc-ui' -ErrorAction SilentlyContinue) {
    try { $runningPath = $p.Path } catch { $runningPath = $null }
    if ($runningPath -and ([IO.Path]::GetDirectoryName($runningPath) -eq $folder)) {
        throw 'Exit the installed Minimalist app from Settings or its tray menu before reinstalling.'
    }
}
New-Item -ItemType Directory -Path $folder -Force | Out-Null
foreach ($name in @('mcc.exe','mcc-ui.exe','README.md','LICENSE','THIRD_PARTY_NOTICES.md','RUST-RUNTIME-NOTICES.html')) {
    $file = Join-Path $Source $name
    if (Test-Path -LiteralPath $file -PathType Leaf) { Copy-Item -LiteralPath $file -Destination (Join-Path $folder $name) -Force }
}
foreach ($name in @('mcc.exe','mcc-ui.exe')) {
    if ((Get-FileHash -LiteralPath (Join-Path $Source $name)).Hash -ne (Get-FileHash -LiteralPath (Join-Path $folder $name)).Hash) { throw "Copy verification failed: $name" }
}
$reply = & (Join-Path $folder 'mcc.exe') startup on | ConvertFrom-Json
if (-not $reply.ok -or -not $reply.enabled) { throw ('Files installed, but startup registration failed: '+$reply.error) }
Write-Output "Installed: $folder"
Write-Output 'Starts in the tray at sign-in. Enable controls still needs administrator approval. No hardware settings were changed.'
