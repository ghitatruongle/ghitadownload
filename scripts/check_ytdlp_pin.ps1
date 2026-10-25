param(
    [string]$ManifestPath = "release/yt-dlp.json",
    [string]$ShaFilePath = "release/yt-dlp.sha256",
    [string]$EnvSourcePath = "src/utils/env.rs"
)

$ErrorActionPreference = "Stop"

if (!(Test-Path -LiteralPath $ManifestPath -PathType Leaf)) {
    throw "Missing yt-dlp manifest: $ManifestPath"
}
if (!(Test-Path -LiteralPath $ShaFilePath -PathType Leaf)) {
    throw "Missing yt-dlp sha file: $ShaFilePath"
}
if (!(Test-Path -LiteralPath $EnvSourcePath -PathType Leaf)) {
    throw "Missing env source: $EnvSourcePath"
}

$manifest = Get-Content -LiteralPath $ManifestPath -Raw | ConvertFrom-Json
$shaFile = (Get-Content -LiteralPath $ShaFilePath -Raw).Trim().ToLowerInvariant()
$envSource = Get-Content -LiteralPath $EnvSourcePath -Raw

if ($shaFile -ne $manifest.sha256.ToLowerInvariant()) {
    throw "Pin desync: release/yt-dlp.sha256 does not match release/yt-dlp.json sha256"
}
if (-not $envSource.Contains($manifest.version)) {
    throw "Pin desync: src/utils/env.rs does not contain pinned version $($manifest.version)"
}
if (-not $envSource.Contains($manifest.sha256)) {
    throw "Pin desync: src/utils/env.rs does not contain pinned sha256"
}
Write-Host "Pinned yt-dlp: $($manifest.version)"
Write-Host "Pinned sha256: $($manifest.sha256)"

$headers = @{
    "User-Agent" = "ghitadownload-pin-check"
    "Accept"     = "application/vnd.github+json"
}
$latest = (Invoke-RestMethod -Uri "https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest" -Headers $headers).tag_name
Write-Host "Latest upstream yt-dlp: $latest"

function ConvertTo-VersionParts {
    param([string]$Version)
    return @($Version.Trim().TrimStart('v').Split('.') | ForEach-Object { [long]$_ })
}

$pinnedParts = ConvertTo-VersionParts $manifest.version
$latestParts = ConvertTo-VersionParts $latest
$updateAvailable = $false
for ($index = 0; $index -lt [Math]::Max($pinnedParts.Count, $latestParts.Count); $index++) {
    $pinnedPart = if ($index -lt $pinnedParts.Count) { $pinnedParts[$index] } else { 0 }
    $latestPart = if ($index -lt $latestParts.Count) { $latestParts[$index] } else { 0 }
    if ($latestPart -gt $pinnedPart) {
        $updateAvailable = $true
        break
    }
    if ($latestPart -lt $pinnedPart) {
        break
    }
}

if (!$updateAvailable) {
    Write-Host "yt-dlp pin is up to date."
    exit 0
}

$title = "yt-dlp pin can cap nhat len $latest"
$existing = gh issue list --state open --search "$latest in:title yt-dlp pin" --json number --jq "length" 2>$null
if ($existing -and [int]$existing -gt 0) {
    Write-Host "Reminder issue for yt-dlp $latest already exists."
    exit 0
}

$body = @'
Upstream yt-dlp latest release: **{0}**
Pinned in this repository: **{1}**

Cần cập nhật đồng bộ các chỗ pin yt-dlp:

- `src/utils/env.rs`: `YTDLP_PINNED_VERSION`, `YTDLP_FALLBACK_URL`, `YTDLP_FALLBACK_SHA256`
- `release/yt-dlp.json`: `version`, `url`, `sha256`
- `release/yt-dlp.sha256`
- `bin/yt-dlp.exe`: payload chính thức từ bản {0}, xác minh bằng `scripts/verify_dependencies.ps1`

Các bước:

1. Tải yt-dlp.exe chính thức của bản {0}, thay payload `bin/yt-dlp.exe`.
2. Cập nhật version/url/sha256 ở 3 chỗ pin trên với SHA-256 chính thức của payload.
3. Chạy `scripts/verify_dependencies.ps1` để xác minh payload khớp manifest.
4. Chạy `cargo test --locked` rồi rebuild installer (`build_installer.bat`), xác minh bằng `scripts/verify_release.ps1`.
'@ -f $latest, $manifest.version

gh issue create --title $title --body $body
Write-Host "Created reminder issue: $title"
