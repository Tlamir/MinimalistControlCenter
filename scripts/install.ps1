param([string]$Source = $PSScriptRoot, [switch]$NoLaunch, [switch]$KeepOemStartup)
$ErrorActionPreference = 'Stop'
$Source = [IO.Path]::GetFullPath((Resolve-Path -LiteralPath $Source).Path)
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = New-Object Security.Principal.WindowsPrincipal($identity)
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    # The one-time UAC prompt is installation consent. No passwords are stored.
    $arguments = '-NoLogo -NoProfile -ExecutionPolicy Bypass -File "' + $PSCommandPath + '" -Source "' + $Source + '"'
    if ($NoLaunch) { $arguments += ' -NoLaunch' }
    if ($KeepOemStartup) { $arguments += ' -KeepOemStartup' }
    try {
        $child = Start-Process -FilePath ([IO.Path]::Combine([Environment]::SystemDirectory,'WindowsPowerShell\v1.0\powershell.exe')) -ArgumentList $arguments -Verb RunAs -PassThru -Wait -WindowStyle Hidden
    } catch {
        $launchError = $_.Exception
        while ($launchError.InnerException) { $launchError = $launchError.InnerException }
        if ($launchError -is [ComponentModel.Win32Exception] -and $launchError.NativeErrorCode -eq 1223) {
            Write-Output 'Installation canceled. No Windows settings were changed.'
            exit 1
        }
        throw
    }
    if ($child.ExitCode -ne 0) { throw ('Installation failed. Exit code: ' + $child.ExitCode) }
    exit 0
}
$taskChanged = $false
$runRemoved = $false
$oemRemoved = $false
$finished = $false
$shortcutChanged = $false
$metadataChanged = $false
$installStage = 'preflight'
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
    # Inspect all inputs before altering startup or shortcuts.
    $version = (Get-Content -LiteralPath (Join-Path $Source 'VERSION') -Raw).Trim()
    if ($version -notmatch '^\d+\.\d+\.\d+$') { throw 'Invalid release version' }
    $files = @('mcc.exe','mcc-ui.exe','README.md','icon.svg','BENCHMARKS.md','FAN_SUPPORT.md','OWNER_CHECKS.md','SAFETY.md','PROTOCOL.md','BUILD.md','benchmarks.svg','BENCHMARKS.json','LICENSE','THIRD_PARTY_NOTICES.md','RUST-RUNTIME-NOTICES.html','Uninstall.ps1','VERSION')
    $manifest = Get-Content -LiteralPath (Join-Path $Source 'manifest.json') -Raw | ConvertFrom-Json
    foreach ($name in $files) {
        $file = Join-Path $Source $name
        $expected = $manifest.PSObject.Properties[$name].Value
        if ($expected -notmatch '^[a-fA-F0-9]{64}$' -or -not (Test-Path -LiteralPath $file -PathType Leaf) -or
            (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash -ne $expected) { throw ('Release verification failed: ' + $name) }
    }
    $sid = $identity.User.Value
    $programFiles = [Environment]::GetFolderPath('ProgramFiles')
    $root = Join-Path $programFiles 'MinimalistControlCenter'
    $userRoot = Join-Path $root $sid
    $destination = Join-Path $userRoot $version
    function Assert-PhysicalPath([string]$path) {
        $itemPath = [IO.Path]::GetFullPath($path)
        if (-not $itemPath.StartsWith($root + '\', [StringComparison]::OrdinalIgnoreCase) -and $itemPath -ne $root) { throw 'Unexpected installation path' }
        for ($cursor = $itemPath; $cursor; $cursor = [IO.Path]::GetDirectoryName($cursor)) {
            if (Test-Path -LiteralPath $cursor) {
                $item = Get-Item -LiteralPath $cursor -Force
                if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw ('Refusing redirected installation path: ' + $cursor) }
            }
        }
    }
    Assert-PhysicalPath $destination
    foreach ($process in Get-Process -Name 'mcc-ui','mcc' -ErrorAction SilentlyContinue) {
        $runningPath = $process.Path
        if ($runningPath -and ($runningPath.StartsWith($userRoot + '\', [StringComparison]::OrdinalIgnoreCase) -or
                $runningPath.StartsWith((Join-Path $env:USERPROFILE 'Applications\MinimalistControlCenter') + '\', [StringComparison]::OrdinalIgnoreCase))) {
            throw 'Click Exit in the Minimalist header or its tray menu, then run Install again.'
        }
    }
    $scheduler = New-Object -ComObject Schedule.Service
    $scheduler.Connect()
    $taskFolder = $scheduler.GetFolder('\')
    $taskName = 'MinimalistControlCenter-' + $sid
    $oldTask = $null
    try { $oldTask = $taskFolder.GetTask($taskName) } catch { if ($_.Exception.HResult -ne -2147024894) { throw } }
    if ($oldTask) {
        $oldAction = $oldTask.Definition.Actions
        if ($oldAction.Count -ne 1 -or $oldAction.Item(1).Type -ne 0 -or
            -not $oldAction.Item(1).Path.StartsWith($userRoot + '\', [StringComparison]::OrdinalIgnoreCase) -or
            $oldAction.Item(1).Arguments -ne '--startup-controls' -or -not (Test-TaskUser $oldTask.Definition.Principal.UserId $sid)) {
            throw 'The startup task is not a recognized Minimalist installation. It was not replaced.'
        }
        $oldTaskXml = $oldTask.Xml
        $oldTaskSecurity = $oldTask.GetSecurityDescriptor(7)
    }
    $registry = [wmiclass]'\\.\root\default:StdRegProv'
    $runKey = 'Software\Microsoft\Windows\CurrentVersion\Run'
    $oldRun = Read-RegString ([uint32]2147483649) $runKey 'MinimalistControlCenter'
    if ($oldRun.ReturnValue -notin @(0,2)) { throw 'Could not inspect existing Minimalist startup' }
    $oem = Read-RegString ([uint32]2147483650) $runKey 'Monster Kontrol Merkezi'
    if (-not $KeepOemStartup) {
        if ($oem.ReturnValue -notin @(0,2)) { throw 'Could not inspect original-controller launcher startup' }
        if ($oem.ReturnValue -eq 0) {
            $oemPath = Join-Path $programFiles 'OEM\Monster Kontrol Merkezi\LaunchCtrlGM.exe'
            if ($oem.sValue -notin @(($oemPath + ' -R'),('"' + $oemPath + '" -R'))) { throw 'Unrecognized original-controller startup command was preserved. Disable it in Windows Startup Apps if needed.' }
            $backup = Join-Path $userRoot 'oem-startup.json'
            Assert-PhysicalPath $backup
            if (Test-Path -LiteralPath $backup) {
                if ((Get-Item -LiteralPath $backup).Length -gt 4096 -or
                    (Get-Content -LiteralPath $backup -Raw | ConvertFrom-Json).command -ne $oem.sValue) { throw 'Original-controller startup backup does not match. It was preserved.' }
            }
        }
    }
    $desktop = [Environment]::GetFolderPath('DesktopDirectory')
    $shortcutPath = Join-Path $desktop 'Minimalist Control Center.lnk'
    $shell = New-Object -ComObject WScript.Shell
    $shortcut = $shell.CreateShortcut($shortcutPath)
    $oldShortcutExists = Test-Path -LiteralPath $shortcutPath
    $oldShortcut = @{Target=$shortcut.TargetPath; Directory=$shortcut.WorkingDirectory; Arguments=$shortcut.Arguments; Description=$shortcut.Description; Icon=$shortcut.IconLocation}
    if ((Test-Path -LiteralPath $shortcutPath) -and $shortcut.TargetPath -and
        -not $shortcut.TargetPath.StartsWith($root + '\', [StringComparison]::OrdinalIgnoreCase) -and
        -not $shortcut.TargetPath.StartsWith((Join-Path $env:USERPROFILE 'Applications\MinimalistControlCenter') + '\', [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Desktop shortcut belongs to another application. It was not replaced.'
    }
    $uninstallKey = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\MinimalistControlCenter-' + $sid
    $oldMetadata = @{}
    foreach ($name in @('DisplayName','DisplayVersion','Publisher','InstallLocation','UninstallString','DisplayIcon')) {
        $oldValue = Read-RegString ([uint32]2147483650) $uninstallKey $name
        if ($oldValue.ReturnValue -notin @(0,2)) { throw 'Could not inspect existing uninstall registration' }
        $oldMetadata[$name] = $oldValue
    }
    foreach ($name in $files) {
        $existing = Join-Path $destination $name
        Assert-PhysicalPath $existing
        if ((Test-Path -LiteralPath $existing) -and (Get-FileHash -LiteralPath $existing -Algorithm SHA256).Hash -ne $manifest.PSObject.Properties[$name].Value) {
            throw ('A different build of version ' + $version + ' is already installed. Use a new version.')
        }
    }
    # No elevated task ever points at a user-writable executable or working directory.
    $acl = New-Object Security.AccessControl.DirectorySecurity
    $acl.SetSecurityDescriptorSddlForm('O:BAG:BAD:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;GRGX;;;BU)')
    foreach ($directory in @($root,$userRoot,$destination)) {
        New-Item -ItemType Directory -Path $directory -Force | Out-Null
        Assert-PhysicalPath $directory
        Set-Acl -LiteralPath $directory -AclObject $acl
    }
    foreach ($name in $files) {
        $target = Join-Path $destination $name
        Copy-Item -LiteralPath (Join-Path $Source $name) -Destination $target -Force
        $fileAcl = New-Object Security.AccessControl.FileSecurity
        $fileAcl.SetSecurityDescriptorSddlForm('O:BAG:BAD:P(A;;FA;;;SY)(A;;FA;;;BA)(A;;GRGX;;;BU)')
        Set-Acl -LiteralPath $target -AclObject $fileAcl
        if ((Get-FileHash -LiteralPath $target -Algorithm SHA256).Hash -ne $manifest.PSObject.Properties[$name].Value) { throw ('Installed file verification failed: ' + $name) }
    }
    $executable = Join-Path $destination 'mcc-ui.exe'
    $preferences = Join-Path $destination 'profiles.json'
    Assert-PhysicalPath $preferences
    if (-not (Test-Path -LiteralPath $preferences)) {
        $previous = @()
        foreach ($base in @($userRoot,(Join-Path $env:USERPROFILE 'Applications\MinimalistControlCenter'))) {
            if (Test-Path -LiteralPath $base -PathType Container) {
                $previous += Get-ChildItem -LiteralPath $base -Directory | Where-Object {
                    $_.Name -match '^\d+\.\d+\.\d+$' -and [version]$_.Name -lt [version]$version -and
                    -not ($_.Attributes -band [IO.FileAttributes]::ReparsePoint)
                }
            }
        }
        foreach ($old in @($previous | Sort-Object { [version]$_.Name } -Descending)) {
            $oldPreferences = Join-Path $old.FullName 'profiles.json'
            if (Test-Path -LiteralPath $oldPreferences -PathType Leaf) {
                $item = Get-Item -LiteralPath $oldPreferences
                if ($item.Length -le 16384 -and -not ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
                    Copy-Item -LiteralPath $oldPreferences -Destination $preferences
                    break
                }
            }
        }
    }
    # Interactive token: current administrator user's desktop, never SYSTEM or a stored password.
    $installStage = 'prepare startup task'
    $definition = $scheduler.NewTask(0)
    $definition.RegistrationInfo.Description = 'Minimalist tray controls. At sign-in: verified connection and light-bar off. No saved fan or keyboard preset is applied.'
    $definition.Principal.UserId = $sid
    $definition.Principal.LogonType = 3
    $definition.Principal.RunLevel = 1
    $definition.Settings.Enabled = $false
    $definition.Settings.StartWhenAvailable = $true
    $definition.Settings.DisallowStartIfOnBatteries = $false
    $definition.Settings.StopIfGoingOnBatteries = $false
    $definition.Settings.ExecutionTimeLimit = 'PT0S'
    $definition.Settings.MultipleInstances = 2
    $trigger = $definition.Triggers.Create(9)
    $trigger.UserId = $sid
    $trigger.Delay = 'PT10S'
    $trigger.Enabled = $true
    $action = $definition.Actions.Create(0)
    $action.Path = $executable
    $action.Arguments = '--startup-controls'
    $action.WorkingDirectory = $destination
    $security = 'O:BAG:BAD:P(A;;FA;;;SY)(A;;FA;;;BA)(A;;GRGX;;;' + $sid + ')'
    # CREATE_OR_UPDATE | DONT_ADD_PRINCIPAL_ACE: ordinary user can read/run, not rewrite the action.
    $installStage = 'register startup task'
    $task = $taskFolder.RegisterTaskDefinition($taskName, $definition, 22, $sid, $null, 3, $security)
    $taskChanged = $true
    $installStage = 'verify startup task'
    $readback = $taskFolder.GetTask($taskName)
    if ($readback.Enabled -or $readback.Definition.Actions.Item(1).Path -ne $executable -or
        $readback.Definition.Principal.RunLevel -ne 1 -or -not (Test-TaskUser $readback.Definition.Triggers.Item(1).UserId $sid)) { throw 'Startup task verification failed' }
    $installStage = 'update startup registrations'
    $removed = if ($oldRun.ReturnValue -eq 2) { @{ReturnValue=2} } else { $registry.DeleteValue([uint32]2147483649,$runKey,'MinimalistControlCenter') }
    if ($removed.ReturnValue -notin @(0,2)) { throw 'Installed task, but old tray startup could not be removed' }
    $runRemoved = $true
    if (-not $KeepOemStartup -and $oem.ReturnValue -eq 0) {
        # Prevent the reviewed original launcher from reopening a competing GUI at every logon.
        # Touch only this exact known HKLM value, with a protected reversible backup.
            if (-not (Test-Path -LiteralPath $backup)) {
                @{command=$oem.sValue} | ConvertTo-Json | Set-Content -LiteralPath $backup -Encoding utf8
            }
            $backupAcl = New-Object Security.AccessControl.FileSecurity
            $backupAcl.SetSecurityDescriptorSddlForm('O:BAG:BAD:P(A;;FA;;;SY)(A;;FA;;;BA)(A;;GR;;;BU)')
            Set-Acl -LiteralPath $backup -AclObject $backupAcl
            $removed = $registry.DeleteValue([uint32]2147483650,$runKey,'Monster Kontrol Merkezi')
            if ($removed.ReturnValue -ne 0) { throw 'Could not disable original-controller launcher startup' }
            $oemRemoved = $true
            Write-Output 'Original controller launcher startup paused; its entry is restored on uninstall. Touchpad/OSD entries were preserved.'
    }
    $installStage = 'update desktop shortcut'
    $shortcut.TargetPath = $executable
    $shortcut.WorkingDirectory = $destination
    $shortcut.Arguments = '--open-installed'
    $shortcut.Description = 'Minimalist Control Center'
    $shortcut.IconLocation = $executable + ',0'
    $shortcut.Save()
    $shortcutChanged = $true
    $installStage = 'register uninstall'
    $created = $registry.CreateKey([uint32]2147483650,$uninstallKey)
    if ($created.ReturnValue -ne 0) { throw 'Could not register uninstall entry' }
    $metadataChanged = $true
    $uninstallCommand = '"' + $env:SystemRoot + '\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -ExecutionPolicy Bypass -File "' + (Join-Path $destination 'Uninstall.ps1') + '"'
    foreach ($entry in @(@('DisplayName','Minimalist Control Center'),@('DisplayVersion',$version),@('Publisher','Minimalist Control Center contributors'),@('InstallLocation',$destination),@('UninstallString',$uninstallCommand),@('DisplayIcon',$executable))) {
        $result = $registry.SetStringValue([uint32]2147483650,$uninstallKey,$entry[0],$entry[1])
        if ($result.ReturnValue -ne 0) { throw ('Uninstall metadata failed: ' + $entry[0]) }
    }
    $installStage = 'enable startup task'
    $task.Enabled = $true
    if (-not $taskFolder.GetTask($taskName).Enabled) { throw 'Could not enable verified startup task' }
    $finished = $true
    Write-Output ('Installed: ' + $destination)
    Write-Output 'Startup enabled on AC and battery. One installation approval; no repeated UAC at sign-in. Light-bar off is requested after sign-in.'
    Write-Output 'Exit any already-running Monster GUI before using controls. Its installed files and service startup type were preserved.'
    if (-not $NoLaunch) {
        try { $null = $task.Run($null) } catch { Write-Warning ('Installed successfully, but immediate launch failed: ' + $_.Exception.Message) }
    }
} catch {
    $failure = $_
    $errorText = $failure.Exception.Message
    if (-not $finished) {
        $rollbackErrors = New-Object 'Collections.Generic.List[string]'
        function Invoke-Rollback([scriptblock]$step) {
            try { & $step } catch { $rollbackErrors.Add($_.Exception.Message) }
        }
        Invoke-Rollback {
            if ($shortcutChanged) {
                if ($oldShortcutExists) {
                    $shortcut.TargetPath = $oldShortcut.Target
                    $shortcut.WorkingDirectory = $oldShortcut.Directory
                    $shortcut.Arguments = $oldShortcut.Arguments
                    $shortcut.Description = $oldShortcut.Description
                    $shortcut.IconLocation = $oldShortcut.Icon
                    $shortcut.Save()
                } else { Remove-Item -LiteralPath $shortcutPath }
            }
        }
        Invoke-Rollback {
            if ($metadataChanged) {
                foreach ($name in $oldMetadata.Keys) {
                    if ($oldMetadata[$name].ReturnValue -eq 0) { $result = $registry.SetStringValue([uint32]2147483650,$uninstallKey,$name,$oldMetadata[$name].sValue) }
                    else { $result = $registry.DeleteValue([uint32]2147483650,$uninstallKey,$name) }
                    if ($result.ReturnValue -notin @(0,2)) { throw 'Uninstall registration rollback failed' }
                }
                if ($oldMetadata['InstallLocation'].ReturnValue -eq 2) { $null = $registry.DeleteKey([uint32]2147483650,$uninstallKey) }
            }
        }
        Invoke-Rollback {
            if ($oemRemoved) {
                $result = $registry.SetStringValue([uint32]2147483650,$runKey,'Monster Kontrol Merkezi',$oem.sValue)
                if ($result.ReturnValue -ne 0) { throw 'Original launcher startup restore failed' }
            }
        }
        Invoke-Rollback {
            if ($runRemoved -and $oldRun.ReturnValue -eq 0) {
                $result = $registry.SetStringValue([uint32]2147483649,$runKey,'MinimalistControlCenter',$oldRun.sValue)
                if ($result.ReturnValue -ne 0) { throw 'Previous tray startup restore failed' }
            }
        }
        Invoke-Rollback {
            if ($taskChanged) {
                if ($oldTaskXml) { $null = $taskFolder.RegisterTask($taskName,$oldTaskXml,22,$sid,$null,3,$oldTaskSecurity) }
                else { $taskFolder.DeleteTask($taskName,0) }
            }
        }
        if ($rollbackErrors.Count) { $errorText += '. Rollback errors: ' + ($rollbackErrors -join '; ') }
    }
    # Keep full diagnostics in the protected installation root, never in a release archive.
    try {
        if ($userRoot -and (Test-Path -LiteralPath $userRoot -PathType Container)) {
            $logPath = Join-Path $userRoot 'install-error.log'
            Assert-PhysicalPath $logPath
            @('Stage: ' + $installStage, $errorText, $failure.Exception.ToString(), $failure.ScriptStackTrace, $failure.InvocationInfo.PositionMessage) | Set-Content -LiteralPath $logPath -Encoding utf8
        }
    } catch { Write-Warning 'The installation diagnostic log could not be saved.' }
    Write-Error $errorText -ErrorAction Continue
    Add-Type -AssemblyName System.Windows.Forms
    [Windows.Forms.MessageBox]::Show($errorText,'Minimalist installation failed') | Out-Null
    exit 1
}
