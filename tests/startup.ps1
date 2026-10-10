# Executes the embedded startup script against fake COM/registry dependencies; no OS mutations.
$ErrorActionPreference = 'Stop'
class FakeCollection {
    [object[]]$Items
    [int]$Count
    FakeCollection([object[]]$items) { $this.Items=$items; $this.Count=$items.Count }
    [object] Item([int]$number) { return $this.Items[$number-1] }
}
class FakeTask {
    [bool]$Enabled = $true
    [object]$Definition
    [int]$Runs = 0
    [object] Run([object]$ignored) { $this.Runs++; return $null }
    [string] GetSecurityDescriptor([int]$flags) { if ($flags -ne 7) { throw 'Wrong descriptor scope' }; return 'protected-owner-and-dacl' }
}
class FakeFolder {
    [object]$Task
    [object]$Readback
    [int]$Reads = 0
    [int]$Registrations = 0
    [object] RegisterTaskDefinition([string]$name,[object]$definition,[int]$flags,[string]$sid,[object]$password,[int]$logon,[string]$security) {
        if ($name -ne ('MinimalistControlCenter-' + $sid) -or $flags -ne 22 -or $password -or $logon -ne 3 -or $security -ne 'protected-owner-and-dacl') { throw 'Task registration scope changed' }
        $this.Registrations++
        $this.Task.Definition=$definition
        return $this.Task
    }
    [object] GetTask([string]$name) {
        $this.Reads++
        if ($this.Reads -gt 1 -and $this.Readback) { return $this.Readback }
        return $this.Task
    }
}
class FakeScheduler {
    [FakeFolder]$Folder = [FakeFolder]::new()
    [void] Connect() {}
    [object] GetFolder([string]$path) { if ($path -ne '\') { throw 'Wrong task scope' }; return $this.Folder }
}
class FakeRegistry {
    [string]$Value = ''
    [int]$Reads = 0
    [int]$Writes = 0
    [int]$Deletes = 0
    [uint32]$Error = 0
    [uint32]$EnumError = 0
    [int]$Enumerations = 0
    [void] Check([uint32]$hive,[string]$key,[string]$name) {
        if ($hive -ne 2147483649 -or $key -ne 'Software\Microsoft\Windows\CurrentVersion\Run' -or $name -ne 'MinimalistControlCenter') { throw 'Wrong registry scope' }
    }
    [object] GetStringValue([uint32]$hive,[string]$key,[string]$name) {
        $this.Check($hive,$key,$name); $this.Reads++
        if (-not $this.Value) { return @{ReturnValue=1; sValue=$null} }
        return @{ReturnValue=0; sValue=$this.Value}
    }
    [object] EnumValues([uint32]$hive,[string]$key) {
        $this.Check($hive,$key,'MinimalistControlCenter'); $this.Enumerations++
        return @{ReturnValue=$this.EnumError; sNames=$(if ($this.Value) { @('MinimalistControlCenter') } else { @() })}
    }
    [object] SetStringValue([uint32]$hive,[string]$key,[string]$name,[string]$command) {
        $this.Check($hive,$key,$name); $this.Writes++
        if (-not $this.Error) { $this.Value=$command }
        return @{ReturnValue=$this.Error}
    }
    [object] DeleteValue([uint32]$hive,[string]$key,[string]$name) {
        $this.Check($hive,$key,$name); $this.Deletes++
        if (-not $this.Error) { $this.Value='' }
        return @{ReturnValue=$this.Error}
    }
}
function New-Object {
    param([string]$TypeName,[object[]]$ArgumentList,[string]$ComObject)
    if ($ComObject -eq 'Schedule.Service') { return $script:TestScheduler }
    Microsoft.PowerShell.Utility\New-Object @PSBoundParameters
}
$source = Get-Content -LiteralPath (Join-Path $PSScriptRoot '../src/startup.ps1') -Raw
$line = '$registry = [wmiclass]''\\.\root\default:StdRegProv'''
if (-not $source.Contains($line)) { throw 'Registry dependency injection point changed' }
$scriptUnderTest = [scriptblock]::Create($source.Replace($line,'$registry = $TestRegistry'))
$env:MCC_STARTUP_EXECUTABLE = 'C:\Program Files\MinimalistControlCenter\0.6.0\mcc-ui.exe'
$env:MCC_STARTUP_COMMAND = '"' + $env:MCC_STARTUP_EXECUTABLE + '" --tray'
function Reset-Fakes {
    $script:TestScheduler=[FakeScheduler]::new()
    $script:TestRegistry=[FakeRegistry]::new()
}
function Request([string]$action) {
    $env:MCC_STARTUP_ACTION=$action
    return (& $scriptUnderTest | ConvertFrom-Json)
}
function Assert([bool]$condition,[string]$message) { if (-not $condition) { throw $message } }
function Set-ManagedTask {
    $sid = [Security.Principal.WindowsIdentity]::GetCurrent().User.Value
    $task=[FakeTask]::new()
    $task.Definition=@{
        Actions=[FakeCollection]::new(@(@{Type=0; Path=$env:MCC_STARTUP_EXECUTABLE; Arguments='--startup-controls'; WorkingDirectory=[IO.Path]::GetDirectoryName($env:MCC_STARTUP_EXECUTABLE)}))
        Triggers=[FakeCollection]::new(@(@{Type=9; UserId=$sid; Enabled=$true}))
        Principal=@{UserId=$sid; LogonType=3; RunLevel=1}
    }
    $script:TestScheduler.Folder.Task=$task
    return $task
}
Reset-Fakes
$reply=Request 'status'
Assert ($reply.ok -and -not $reply.enabled -and -not $reply.managed) 'Absent portable startup is off'
Assert ($TestRegistry.Writes -eq 0 -and $TestRegistry.Deletes -eq 0) 'Status must never write'
$reply=Request 'on'
Assert ($reply.ok -and $reply.enabled -and -not $reply.managed) 'Portable startup must verify real-provider readback'
Assert ($TestRegistry.Writes -eq 1 -and $TestRegistry.Value -eq $env:MCC_STARTUP_COMMAND) 'Portable writes fixed quoted command'
Reset-Fakes
$TestRegistry.Error=5
$reply=Request 'on'
Assert (-not $reply.ok -and $TestRegistry.Writes -eq 1) 'Denied writes must surface without fallback or retries'
Reset-Fakes
$task=Set-ManagedTask
$reply=Request 'status'
Assert ($reply.ok -and $reply.managed -and $reply.enabled) 'Installed startup is distinguished from portable startup'
Assert ($TestRegistry.Reads -eq 0 -and $TestRegistry.Writes -eq 0) 'Managed status must not consult the redirected Run entry'
$TestRegistry.Value=$env:MCC_STARTUP_COMMAND
$reply=Request 'off'
Assert ($reply.ok -and -not $reply.enabled -and $task.Enabled -and -not $task.Definition.Triggers.Item(1).Enabled) 'Installed toggle disables automatic logon but retains demand launch'
Assert ($TestRegistry.Writes -eq 0 -and $TestRegistry.Deletes -eq 1) 'Installed toggle must remove duplicate portable startup'
Assert ($TestScheduler.Folder.Registrations -eq 1) 'Toggle preserves protected task registration'
$reply=Request 'launch'
Assert ($reply.ok -and -not $reply.enabled -and $task.Runs -eq 1) 'Desktop launch must still work with automatic startup off'
$reply=Request 'on'
Assert ($reply.ok -and $reply.enabled -and $task.Definition.Triggers.Item(1).Enabled) 'Automatic logon can be enabled again'
Reset-Fakes
$task=Set-ManagedTask
$task.Definition.Actions.Item(1).Path='C:\unrelated.exe'
$reply=Request 'launch'
Assert (-not $reply.ok -and $task.Runs -eq 0 -and $TestRegistry.Deletes -eq 0) 'Foreign task must not be launched or changed'
Reset-Fakes
$task=Set-ManagedTask
$task.Enabled=$false
$stale=[FakeTask]::new()
$stale.Enabled=$false
$TestScheduler.Folder.Readback=$stale
$reply=Request 'on'
Assert (-not $reply.ok -and $reply.error -eq 'Startup readback mismatch') 'Task changes require independent readback'
Reset-Fakes
$task=Set-ManagedTask
$task.Definition.Principal.UserId=[Security.Principal.WindowsIdentity]::GetCurrent().Name
$task.Definition.Triggers.Item(1).UserId=$task.Definition.Principal.UserId
$reply=Request 'status'
Assert ($reply.ok -and $reply.managed -and $reply.enabled) 'Task Scheduler account-name readback must resolve to the same registered SID'
Assert ($TestScheduler.Folder.Registrations -eq 0 -and $TestRegistry.Writes -eq 0) 'Identity normalization never writes'
$task.Definition.Principal.UserId='BUILTIN\Users'
$reply=Request 'launch'
Assert (-not $reply.ok -and $task.Runs -eq 0) 'A resolvable foreign account must still be refused'
Reset-Fakes
$reply=Request 'off'
Assert ($reply.ok -and -not $reply.enabled -and $TestRegistry.Deletes -eq 0 -and $TestRegistry.Reads -eq 0) 'Absent startup is confirmed by enumeration without relying on missing-value error codes or deleting again'
$TestRegistry.EnumError=5
$reply=Request 'status'
Assert (-not $reply.ok -and $TestRegistry.Writes -eq 0) 'Registry enumeration errors must never be mistaken for absence'
foreach ($scriptName in @('install.ps1','uninstall.ps1')) {
    $tokens=$null; $parseErrors=$null
    $ast=[Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot ('../scripts/' + $scriptName)),[ref]$tokens,[ref]$parseErrors)
    $helper=$ast.Find({param($node) $node -is [Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -eq 'Read-RegString'},$true)
    Assert ($null -ne $helper -and -not $parseErrors.Count) 'Installer registry helper must parse'
    $read=[scriptblock]::Create($helper.Extent.Text + "`nRead-RegString 2147483649 'Software\Microsoft\Windows\CurrentVersion\Run' 'MinimalistControlCenter'")
    Reset-Fakes
    $registry=$TestRegistry
    $reply=& $read
    Assert ($reply.ReturnValue -eq 2 -and $TestRegistry.Reads -eq 0) ($scriptName + ' confirms absence without a GetStringValue error')
    $TestRegistry.Value='verified string'
    $reply=& $read
    Assert ($reply.ReturnValue -eq 0 -and $reply.sValue -eq 'verified string') ($scriptName + ' still reads present values')
    $TestRegistry.EnumError=5
    $refused=$false
    try { $null=& $read } catch { $refused=$true }
    Assert $refused ($scriptName + ' must surface enumeration failure')
}
Write-Output 'Startup fake-provider checks passed (15 scenarios); no OS settings changed.'
