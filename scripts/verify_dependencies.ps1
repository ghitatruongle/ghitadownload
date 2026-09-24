param(
    [string]$YtDlpPath = "bin\yt-dlp.exe",
    [string]$ManifestPath = "release\yt-dlp.json"
)

$ErrorActionPreference = "Stop"
if (!(Test-Path -LiteralPath $YtDlpPath -PathType Leaf)) {
    throw "Missing yt-dlp payload: $YtDlpPath"
}
if (!(Test-Path -LiteralPath $ManifestPath -PathType Leaf)) {
    throw "Missing yt-dlp manifest: $ManifestPath"
}
$manifest = Get-Content -LiteralPath $ManifestPath -Raw | ConvertFrom-Json
$bytes = [IO.File]::ReadAllBytes($YtDlpPath)
if ($bytes.Length -lt 1024 -or $bytes[0] -ne 0x4D -or $bytes[1] -ne 0x5A) {
    throw "yt-dlp payload is not a valid Windows PE executable"
}
$hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $YtDlpPath).Hash.ToLowerInvariant()
if ($hash -ne $manifest.sha256.ToLowerInvariant()) {
    throw "yt-dlp SHA-256 mismatch: expected $($manifest.sha256), got $hash"
}
$version = (& $YtDlpPath --version).Trim()
if ($LASTEXITCODE -ne 0 -or $version -ne $manifest.version) {
    throw "yt-dlp version mismatch: expected $($manifest.version), got $version"
}
Write-Host "yt-dlp version: $version"
Write-Host "yt-dlp SHA-256: $hash"
