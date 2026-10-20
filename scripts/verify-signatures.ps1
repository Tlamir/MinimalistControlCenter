param(
    [Parameter(Mandatory=$true, Position=0, ValueFromRemainingArguments=$true)]
    [string[]]$Paths
)
$ErrorActionPreference = 'Stop'
# Resolve the host's own module even when a parent PowerShell 7 process supplied PSModulePath.
Import-Module (Join-Path $PSHOME 'Modules\Microsoft.PowerShell.Security\Microsoft.PowerShell.Security.psd1') -ErrorAction Stop
$publisher = $null
foreach ($path in $Paths) {
    $signature = Get-AuthenticodeSignature -LiteralPath $path
    if ($signature.Status -ne 'Valid' -or -not $signature.SignerCertificate) {
        throw ('Untrusted or missing Authenticode signature: ' + $path + ' (' + $signature.Status + ')')
    }
    if (-not $signature.TimeStamperCertificate) {
        throw ('Release signature must be timestamped: ' + $path)
    }
    $thumbprint = $signature.SignerCertificate.Thumbprint
    if ($publisher -and $publisher -ne $thumbprint) {
        throw ('Release files must use the same publisher certificate: ' + $path)
    }
    $publisher = $thumbprint
    Write-Output ('Verified: ' + $path + ' - ' + $signature.SignerCertificate.Subject)
}
