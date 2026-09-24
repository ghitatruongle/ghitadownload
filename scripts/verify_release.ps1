param(
    [string]$ArtifactPath = ""
)

$ErrorActionPreference = "Stop"
$versionLine = Select-String -Path "Cargo.toml" -Pattern '^version = "(.+)"$' | Select-Object -First 1
if ($null -eq $versionLine) {
    throw "Không đọc được version từ Cargo.toml"
}
$version = $versionLine.Matches[0].Groups[1].Value
if ($version -ne "0.0.3-beta") {
    throw "Unexpected version: $version"
}

if ([string]::IsNullOrWhiteSpace($ArtifactPath)) {
    $ArtifactPath = Join-Path "Release" "ghitadownload_$version.exe"
}
if (!(Test-Path -LiteralPath $ArtifactPath -PathType Leaf)) {
    throw "Missing release artifact: $ArtifactPath"
}

$bytes = [IO.File]::ReadAllBytes($ArtifactPath)
if ($bytes.Length -lt 1024 -or $bytes[0] -ne 0x4D -or $bytes[1] -ne 0x5A) {
    throw "Artifact is not a valid Windows PE executable"
}

$ascii = [Text.Encoding]::ASCII.GetString($bytes)
$versionBytes = [Text.Encoding]::ASCII.GetBytes($version)
$hasVersion = $false
for ($index = 0; $index -le $bytes.Length - $versionBytes.Length; $index++) {
    $matches = $true
    for ($offset = 0; $offset -lt $versionBytes.Length; $offset++) {
        if ($bytes[$index + $offset] -ne $versionBytes[$offset]) {
            $matches = $false
            break
        }
    }
    if ($matches) {
        $hasVersion = $true
        break
    }
}
if (!$hasVersion) {
    throw "Artifact does not embed the package version evidence: $version"
}
if ($ascii.Contains("NO_EMBED")) {
    throw "Artifact contains development placeholder payload evidence"
}
if (!$ascii.Contains("ghitadownload.exe") -or !$ascii.Contains("yt-dlp.exe")) {
    throw "Artifact lacks embedded payload name evidence"
}

$hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $ArtifactPath).Hash
Write-Host "Release version: $version"
Write-Host "Release artifact: $ArtifactPath"
Write-Host "Artifact size: $($bytes.Length) bytes"
Write-Host "Embedded version evidence: present"
Write-Host "Embedded payload names: present"
Write-Host "SHA-256: $hash"
