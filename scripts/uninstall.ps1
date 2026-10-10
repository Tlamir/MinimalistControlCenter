param()
$ErrorActionPreference = 'Stop'
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = New-Object Security.Principal.WindowsPrincipal($identity)
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    try {
        $child = Start-Process -FilePath ([IO.Path]::Combine([Environment]::SystemDirectory,'WindowsPowerShell\v1.0\powershell.exe')) -ArgumentList ('-NoProfile -ExecutionPolicy Bypass -File "' + $PSCommandPath + '"') -Verb RunAs -PassThru -Wait -WindowStyle Hidden
    } catch {
        $launchError = $_.Exception
        while ($launchError.InnerException) { $launchError = $launchError.InnerException }
        if ($launchError -is [ComponentModel.Win32Exception] -and $launchError.NativeErrorCode -eq 1223) {
            Write-Output 'Uninstall canceled. No Windows settings were changed.'
            exit 1
        }
        throw
    }
    exit $child.ExitCode
}
function Test-TaskUser([string]$user,[string]$expectedSid) {
    if ($user -eq $expectedSid) { return $true }
    try { return ([Security.Principal.NTAccount]::new($user).Translate([Security.Principal.SecurityIdentifier]).Value -eq $expectedSid) }
    catch { return $false }
}
function Read-RegString([uint32]$hive,[string]$key,[string]$name) {
    $values = $registry.EnumValues($hive,$key)
    if ($values.ReturnValue -eq 2) { return @{ReturnValue=2; sValue=$null} }
    if ($values.ReturnValue -ne 0) { throw ('Cannot inspect registry key: ' + $values.ReturnValue) }
    if ($values.sNames -notcontains $name) { return @{ReturnValue=2; sValue=$null} }
    $result = $registry.GetStringValue($hive,$key,$name)
    if ($result.ReturnValue -ne 0 -or $null -eq $result.sValue) { throw ('Cannot read registry value: ' + $result.ReturnValue) }
    return $result
}
try {
    $sid = $identity.User.Value
    $root = Join-Path ([Environment]::GetFolderPath('ProgramFiles')) ('MinimalistControlCenter\' + $sid)
    $root = [IO.Path]::GetFullPath($root)
    function Assert-UninstallPath([string]$path) {
        $resolved = [IO.Path]::GetFullPath($path)
        if (-not $resolved.StartsWith($root + '\', [StringComparison]::OrdinalIgnoreCase) -and $resolved -ne $root) { throw 'Uninstall path is outside this installation' }
        for ($cursor = $resolved; $cursor; $cursor = [IO.Path]::GetDirectoryName($cursor)) {
            if ((Test-Path -LiteralPath $cursor) -and ((Get-Item -LiteralPath $cursor -Force).Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Refusing a redirected uninstall path' }
        }
    }
    Assert-UninstallPath $PSCommandPath
    foreach ($process in Get-Process -Name 'mcc-ui','mcc' -ErrorAction SilentlyContinue) {
        if ($process.Path -and $process.Path.StartsWith($root + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Exit Minimalist using Exit in the header or tray menu before uninstalling.' }
    }
    $scheduler = New-Object -ComObject Schedule.Service
    $scheduler.Connect()
    $folder = $scheduler.GetFolder('\')
    $name = 'MinimalistControlCenter-' + $sid
    $task = $null
    try { $task = $folder.GetTask($name) } catch { if ($_.Exception.HResult -ne -2147024894) { throw } }
    if ($task) {
        $actions = $task.Definition.Actions
        if ($actions.Count -ne 1 -or $actions.Item(1).Type -ne 0 -or
            -not $actions.Item(1).Path.StartsWith($root + '\', [StringComparison]::OrdinalIgnoreCase) -or
            $actions.Item(1).Arguments -ne '--startup-controls' -or -not (Test-TaskUser $task.Definition.Principal.UserId $sid)) { throw 'Unrecognized startup task was not removed' }
        $folder.DeleteTask($name,0)
    }
    $registry = [wmiclass]'\\.\root\default:StdRegProv'
    $backup = Join-Path $root 'oem-startup.json'
    Assert-UninstallPath $backup
    if (Test-Path -LiteralPath $backup -PathType Leaf) {
        if ((Get-Item -LiteralPath $backup).Length -gt 4096) { throw 'OEM startup backup is too large' }
        $saved = Get-Content -LiteralPath $backup -Raw | ConvertFrom-Json
        $oemPath = Join-Path ([Environment]::GetFolderPath('ProgramFiles')) 'OEM\Monster Kontrol Merkezi\LaunchCtrlGM.exe'
        if ($saved.command -notin @(($oemPath + ' -R'),('"' + $oemPath + '" -R'))) { throw 'Unrecognized OEM startup backup was not restored' }
        $runKey = 'Software\Microsoft\Windows\CurrentVersion\Run'
        $current = Read-RegString ([uint32]2147483650) $runKey 'Monster Kontrol Merkezi'
        if ($current.ReturnValue -eq 2) {
            $result = $registry.SetStringValue([uint32]2147483650,$runKey,'Monster Kontrol Merkezi',$saved.command)
            if ($result.ReturnValue -ne 0) { throw 'Could not restore original controller startup' }
        } elseif ($current.ReturnValue -ne 0 -or $current.sValue -ne $saved.command) { throw 'Original controller startup changed; its current entry was preserved' }
        Remove-Item -LiteralPath $backup
    }
    $desktopShortcut = Join-Path ([Environment]::GetFolderPath('DesktopDirectory')) 'Minimalist Control Center.lnk'
    if (Test-Path -LiteralPath $desktopShortcut) {
        $shell = New-Object -ComObject WScript.Shell
        $link = $shell.CreateShortcut($desktopShortcut)
        if ($link.TargetPath.StartsWith($root + '\', [StringComparison]::OrdinalIgnoreCase)) { Remove-Item -LiteralPath $desktopShortcut }
    }
    $key = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\MinimalistControlCenter-' + $sid
    $result = $registry.DeleteKey([uint32]2147483650,$key)
    if ($result.ReturnValue -notin @(0,2)) { throw 'Could not remove uninstall registration' }
    # Delete known files only after checking their absolute paths. Preserve profiles and unknown files.
    $files = @('mcc.exe','mcc-ui.exe','README.md','icon.svg','BENCHMARKS.md','FAN_SUPPORT.md','OWNER_CHECKS.md','SAFETY.md','PROTOCOL.md','CONTINUATION.md','BUILD.md','benchmarks.svg','BENCHMARKS.json','LICENSE','THIRD_PARTY_NOTICES.md','RUST-RUNTIME-NOTICES.html','Uninstall.ps1','VERSION')
    if (Test-Path -LiteralPath $root) {
        foreach ($directory in Get-ChildItem -LiteralPath $root -Directory) {
            if ($directory.Name -notmatch '^\d+\.\d+\.\d+$') { continue }
            Assert-UninstallPath $directory.FullName
            foreach ($file in $files) {
                $path = Join-Path $directory.FullName $file
                Assert-UninstallPath $path
                if (Test-Path -LiteralPath $path -PathType Leaf) { Remove-Item -LiteralPath $path }
            }
            if (-not (Get-ChildItem -LiteralPath $directory.FullName -Force)) { Remove-Item -LiteralPath $directory.FullName }
        }
    }
    Write-Output 'Uninstalled. Saved profiles were preserved. Firmware cooling was not changed.'
} catch {
    Write-Error $_.Exception.Message -ErrorAction Continue
    Add-Type -AssemblyName System.Windows.Forms
    [Windows.Forms.MessageBox]::Show($_.Exception.Message,'Minimalist uninstall failed') | Out-Null
    exit 1
}
