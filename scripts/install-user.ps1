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
# Preserve the last installed preferences without applying them or overwriting
# preferences already belonging to this version. Runtime validation still applies.
$preferences = Join-Path $folder 'profiles.json'
if (-not (Test-Path -LiteralPath $preferences)) {
    $previous = Get-ChildItem -LiteralPath $parent -Directory |
        Where-Object { $_.Name -match '^\d+\.\d+\.\d+$' -and
            -not ($_.Attributes -band [IO.FileAttributes]::ReparsePoint) -and
            [version]$_.Name -lt [version](Split-Path $folder -Leaf) } |
        Sort-Object { [version]$_.Name } -Descending | Select-Object -First 1
    if ($previous) {
        $oldPreferences = Join-Path $previous.FullName 'profiles.json'
        if (Test-Path -LiteralPath $oldPreferences -PathType Leaf) {
            $item = Get-Item -LiteralPath $oldPreferences
            if ($item.Length -le 16384 -and -not ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
                Copy-Item -LiteralPath $oldPreferences -Destination $preferences
            }
        }
    }
}
# Use Windows' registry provider so a packaged terminal cannot silently register
# only in its private hive. The scope remains this one current-user value.
$startupArgs = @{
    hDefKey = [uint32]2147483649
    sSubKeyName = 'Software\Microsoft\Windows\CurrentVersion\Run'
    sValueName = 'MinimalistControlCenter'
    sValue = ('"' + (Join-Path $folder 'mcc-ui.exe') + '" --tray')
}
$reply = Invoke-CimMethod -Namespace root/default -ClassName StdRegProv -MethodName SetStringValue -Arguments $startupArgs
if ($reply.ReturnValue -ne 0) { throw ('Files installed, but Windows startup registration failed: ' + $reply.ReturnValue) }
$expectedCommand = $startupArgs.sValue
$startupArgs.Remove('sValue')
$readback = Invoke-CimMethod -Namespace root/default -ClassName StdRegProv -MethodName GetStringValue -Arguments $startupArgs
if ($readback.ReturnValue -ne 0 -or $readback.sValue -ne $expectedCommand) { throw 'Windows startup readback mismatch' }
Write-Output "Installed: $folder"
$desktop = [Environment]::GetFolderPath('DesktopDirectory')
$shortcutPath = Join-Path $desktop 'Minimalist Control Center.lnk'
$shellLink = New-Object -ComObject WScript.Shell
$shortcut = $shellLink.CreateShortcut($shortcutPath)
if ((Test-Path -LiteralPath $shortcutPath) -and $shortcut.TargetPath -and
    -not $shortcut.TargetPath.StartsWith($parent + '\', [StringComparison]::OrdinalIgnoreCase)) {
    throw 'Desktop shortcut already points outside this installation. It was not replaced.'
}
$shortcut.TargetPath = Join-Path $folder 'mcc-ui.exe'
$shortcut.WorkingDirectory = $folder
$shortcut.Arguments = ''
$shortcut.Description = 'Minimalist Control Center'
$shortcut.Save()
Write-Output "Desktop shortcut: $shortcutPath"
Write-Output 'Starts in the tray at sign-in. Enable controls still needs administrator approval. No hardware settings were changed.'
