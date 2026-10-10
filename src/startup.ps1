# Embedded, fixed-scope script. Inputs travel through environment values, never evaluated code.
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = New-Object Text.UTF8Encoding($false)
function Test-TaskUser([string]$user,[string]$expectedSid) {
    if ($user -eq $expectedSid) { return $true }
    try {
        # Task Scheduler may return DOMAIN\user even when registered with a SID.
        return ([Security.Principal.NTAccount]::new($user).Translate([Security.Principal.SecurityIdentifier]).Value -eq $expectedSid)
    } catch { return $false }
}
function Read-StartupValue {
    $values = $registry.EnumValues([uint32]2147483649,$key)
    if ($values.ReturnValue -eq 2) { return @{ReturnValue=2; sValue=$null} }
    if ($values.ReturnValue -ne 0) { throw ('Cannot inspect Windows startup: ' + $values.ReturnValue) }
    if ($values.sNames -notcontains $value) { return @{ReturnValue=2; sValue=$null} }
    $result = $registry.GetStringValue([uint32]2147483649,$key,$value)
    if ($result.ReturnValue -ne 0 -or $null -eq $result.sValue) { throw ('Cannot read Windows startup: ' + $result.ReturnValue) }
    return $result
}
try {
    $action = $env:MCC_STARTUP_ACTION
    if ($action -notin @('status','on','off','launch')) { throw 'Invalid startup action' }
    $executable = $env:MCC_STARTUP_EXECUTABLE
    $expected = $env:MCC_STARTUP_COMMAND
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $sid = $identity.User.Value
    $name = 'MinimalistControlCenter-' + $sid
    $scheduler = New-Object -ComObject Schedule.Service
    $scheduler.Connect()
    $folder = $scheduler.GetFolder('\')
    $task = $null
    try { $task = $folder.GetTask($name) } catch {
        if ($_.Exception.HResult -ne -2147024894) { throw }
    }
    # A task belonging to another installation must not be edited or launched.
    $managed = $false
    if ($task) {
        $definition = $task.Definition
        $principal = $definition.Principal
        $actions = $definition.Actions
        $triggers = $definition.Triggers
        $managed = $actions.Count -eq 1 -and $actions.Item(1).Type -eq 0 -and
            $actions.Item(1).Path -eq $executable -and
            $actions.Item(1).Arguments -eq '--startup-controls' -and
            $actions.Item(1).WorkingDirectory -eq [IO.Path]::GetDirectoryName($executable) -and
            (Test-TaskUser $principal.UserId $sid) -and $principal.LogonType -eq 3 -and $principal.RunLevel -eq 1 -and
            $triggers.Count -eq 1 -and $triggers.Item(1).Type -eq 9 -and
            (Test-TaskUser $triggers.Item(1).UserId $sid)
        if (-not $managed) { throw 'Startup belongs to a different installation. Use its installer to update it.' }
    }
    # This out-of-process provider reaches the real HKCU even from packaged launchers.
    $registry = [wmiclass]'\\.\root\default:StdRegProv'
    $key = 'Software\Microsoft\Windows\CurrentVersion\Run'
    $value = 'MinimalistControlCenter'
    if ($action -eq 'launch') {
        if (-not $managed -or -not $task.Enabled) { throw 'The installed launch task is unavailable. Run the installer to repair it.' }
        $null = $task.Run($null)
    } elseif ($action -in @('on','off')) {
        if ($managed) {
            # Keep demand launch available when automatic startup is off. The installed UI is
            # elevated; its ordinary desktop shortcut has only read/run permission on this task.
            $definition.Triggers.Item(1).Enabled = $action -eq 'on'
            $security = $task.GetSecurityDescriptor(7)
            $null = $folder.RegisterTaskDefinition($name,$definition,22,$sid,$null,3,$security)
            $existing = Read-StartupValue
            $result = if ($existing.ReturnValue -eq 2) { @{ReturnValue=2} } else { $registry.DeleteValue([uint32]2147483649, $key, $value) }
        } elseif ($action -eq 'on') {
            $result = $registry.SetStringValue([uint32]2147483649, $key, $value, $expected)
        } else {
            $existing = Read-StartupValue
            $result = if ($existing.ReturnValue -eq 2) { @{ReturnValue=2} } else { $registry.DeleteValue([uint32]2147483649, $key, $value) }
        }
        if ($result.ReturnValue -ne 0 -and -not ($result.ReturnValue -eq 2 -and ($managed -or $action -eq 'off'))) {
            throw ('Windows registry error ' + $result.ReturnValue)
        }
    }
    if ($managed) {
        $readback = $folder.GetTask($name)
        $enabled = [bool]$readback.Enabled -and [bool]$readback.Definition.Triggers.Item(1).Enabled
    } else {
        $result = Read-StartupValue
        $enabled = $result.ReturnValue -eq 0 -and $result.sValue -eq $expected
    }
    if (($action -eq 'on' -and -not $enabled) -or ($action -eq 'off' -and $enabled)) { throw 'Startup readback mismatch' }
    @{ok=$true; enabled=[bool]$enabled; managed=[bool]$managed; error=$null} | ConvertTo-Json -Compress
} catch {
    @{ok=$false; enabled=$false; managed=$false; error=$_.Exception.Message} | ConvertTo-Json -Compress
}
