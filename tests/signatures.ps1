$ErrorActionPreference = 'Stop'
$verifier = Join-Path (Split-Path $PSScriptRoot -Parent) 'scripts\verify-signatures.ps1'
function Get-AuthenticodeSignature {
    param([string]$LiteralPath)
    $publisher = if ($LiteralPath -eq 'other-publisher') { 'B' } else { 'A' }
    $status = if ($LiteralPath -in @('NotSigned','HashMismatch','NotTrusted')) { $LiteralPath } else { 'Valid' }
    [pscustomobject]@{
        Status = $status
        SignerCertificate = [pscustomobject]@{ Thumbprint=$publisher; Subject='Test fixture' }
        TimeStamperCertificate = $(if ($LiteralPath -ne 'no-timestamp') { [pscustomobject]@{ Subject='Test timestamp' } })
    }
}
& $verifier -Paths @('first','second') | Out-Null
foreach ($case in @('NotSigned','HashMismatch','NotTrusted','no-timestamp','other-publisher')) {
    $rejected = $false
    try { & $verifier -Paths @('first',$case) | Out-Null } catch { $rejected = $true }
    if (-not $rejected) { throw ('Verifier accepted invalid signature fixture: ' + $case) }
}
Write-Output 'Signature trust, timestamp and publisher regression checks passed.'
