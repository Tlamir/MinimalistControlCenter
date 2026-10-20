param(
    [Parameter(Mandatory=$true)]
    [ValidatePattern('^[a-fA-F0-9]{40}$')]
    [string]$CertificateThumbprint,
    [Parameter(Mandatory=$true)]
    [string]$SignTool,
    [string]$BinDir = (Join-Path (Split-Path $PSScriptRoot -Parent) 'target\release'),
    [string]$OutputDir = (Join-Path (Split-Path $PSScriptRoot -Parent) 'target\signed'),
    [string]$TimestampUrl = 'http://timestamp.digicert.com',
    [switch]$LocalMachine
)
$ErrorActionPreference = 'Stop'
# Select an existing publisher identity; never create or trust a self-signed certificate.
$store = if ($LocalMachine) { 'Cert:\LocalMachine\My\' } else { 'Cert:\CurrentUser\My\' }
$certificate = Get-Item -LiteralPath ($store + $CertificateThumbprint)
if (-not $certificate.HasPrivateKey -or $certificate.NotAfter -le (Get-Date) -or $certificate.NotBefore -gt (Get-Date)) {
    throw 'Signing requires a currently valid certificate with an accessible private key.'
}
if (@($certificate.EnhancedKeyUsageList | Where-Object { $_.ObjectId -eq '1.3.6.1.5.5.7.3.3' }).Count -eq 0) {
    throw 'Certificate must permit code signing.'
}
$tool = (Get-Item -LiteralPath $SignTool).FullName
if (Test-Path -LiteralPath $OutputDir) { throw 'Choose a new output directory for each signing attempt.' }
$inputs = @{
    'mcc.exe' = Join-Path $BinDir 'mcc.exe'
    'mcc-ui.exe' = Join-Path $BinDir 'mcc-ui.exe'
    'Install.ps1' = Join-Path $PSScriptRoot 'install.ps1'
    'Uninstall.ps1' = Join-Path $PSScriptRoot 'uninstall.ps1'
}
foreach ($source in $inputs.Values) {
    if (-not (Test-Path -LiteralPath $source -PathType Leaf)) { throw ('Missing signing input: ' + $source) }
}
New-Item -ItemType Directory -Path $OutputDir | Out-Null
$paths = @()
foreach ($name in @('mcc.exe','mcc-ui.exe','Install.ps1','Uninstall.ps1')) {
    $target = Join-Path $OutputDir $name
    Copy-Item -LiteralPath $inputs[$name] -Destination $target
    $arguments = @('sign','/sha1',$CertificateThumbprint,'/s','My','/fd','SHA256','/tr',$TimestampUrl,'/td','SHA256')
    if ($LocalMachine) { $arguments += '/sm' }
    & $tool @arguments $target
    if ($LASTEXITCODE -ne 0) { throw ('SignTool signing failed: ' + $name) }
    & $tool verify /pa /all /v $target
    if ($LASTEXITCODE -ne 0) { throw ('SignTool verification failed: ' + $name) }
    $paths += $target
}
& (Join-Path $PSScriptRoot 'verify-signatures.ps1') -Paths $paths
Write-Output ('Signed inputs ready for packaging: ' + $OutputDir)
