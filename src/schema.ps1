$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
# Metadata queries only. Never instantiate vendor objects or invoke their methods.
$results = foreach ($name in @('AcpiTest_MULong', 'AcpiTest_EventULong')) {
    try {
        $class = Get-CimClass -Namespace 'root/WMI' -ClassName $name -OperationTimeoutSec 3
        $methods = @($class.CimClassMethods | ForEach-Object {
            [ordered]@{
                name = $_.Name
                return_type = [string]$_.ReturnType
                parameters = @($_.Parameters | ForEach-Object {
                    [ordered]@{ name = $_.Name; cim_type = [string]$_.CimType }
                })
            }
        })
        [ordered]@{ name = $name; available = $true; methods = $methods; error = $null }
    } catch {
        # Omit raw exception strings; they may contain machine names or user paths.
        [ordered]@{ name = $name; available = $false; methods = @(); error = 'unavailable_or_access_denied' }
    }
}
[ordered]@{ namespace = 'root/WMI'; classes = @($results) } | ConvertTo-Json -Depth 8 -Compress
