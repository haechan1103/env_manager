param(
  [Parameter(Mandatory = $true, Position = 0)]
  [string]$FilePath
)

$ErrorActionPreference = "Stop"

$required = @(
  "AZURE_CLIENT_ID",
  "AZURE_CLIENT_SECRET",
  "AZURE_TENANT_ID",
  "AZURE_ARTIFACT_SIGNING_ENDPOINT",
  "AZURE_ARTIFACT_SIGNING_ACCOUNT",
  "AZURE_ARTIFACT_SIGNING_CERTIFICATE_PROFILE"
)

$missing = $required | Where-Object {
  [string]::IsNullOrWhiteSpace([Environment]::GetEnvironmentVariable($_))
}
if ($missing.Count -gt 0) {
  throw "Windows signing is not configured. Missing: $($missing -join ', ')"
}

$resolvedFile = (Resolve-Path -LiteralPath $FilePath).Path
$endpoint = [Uri]$env:AZURE_ARTIFACT_SIGNING_ENDPOINT
if ($endpoint.Scheme -ne "https") {
  throw "AZURE_ARTIFACT_SIGNING_ENDPOINT must use HTTPS."
}

& artifact-signing-cli `
  -e $endpoint.AbsoluteUri `
  -a $env:AZURE_ARTIFACT_SIGNING_ACCOUNT `
  -c $env:AZURE_ARTIFACT_SIGNING_CERTIFICATE_PROFILE `
  -d "Kavranta" `
  $resolvedFile

if ($LASTEXITCODE -ne 0) {
  throw "Artifact Signing failed for $([IO.Path]::GetFileName($resolvedFile))."
}

$signature = Get-AuthenticodeSignature -FilePath $resolvedFile
if ($signature.Status -ne "Valid") {
  throw "Artifact Signing produced an invalid signature: $($signature.Status)"
}
if (-not $signature.TimeStamperCertificate) {
  throw "Artifact Signing did not add an RFC 3161 timestamp."
}
