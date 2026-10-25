param(
    [string]$BinaryPath = "target/release/ghitadownload.exe",
    [string]$OutputRoot = "",
    [string]$DownloadUrl = "https://www.youtube.com/watch?v=jNQXAC9IVRw",
    [string]$SearchQuery = "me at the zoo",
    [switch]$SkipDownloads,
    [switch]$KeepOutput
)

$ErrorActionPreference = "Stop"

Write-Host "=== Ghita Downloader live smoke test ===" -ForegroundColor Cyan

if (!(Test-Path -LiteralPath $BinaryPath -PathType Leaf)) {
    Write-Host "Binary not found: $BinaryPath" -ForegroundColor Red
    Write-Host "Build first: cargo build --release" -ForegroundColor Yellow
    exit 2
}

$stamp = Get-Date -Format "yyyyMMdd_HHmmss"
$baseDir = if ($OutputRoot -ne "") { $OutputRoot } else { [System.IO.Path]::GetTempPath() }
$smokeDir = Join-Path $baseDir "ghita_smoke_$stamp"
New-Item -ItemType Directory -Path $smokeDir -Force | Out-Null
Write-Host "Smoke directory: $smokeDir"
Write-Host ""

$failed = @()

Write-Host "==> Step 1/4: --version" -ForegroundColor Cyan
& $BinaryPath --version
if ($LASTEXITCODE -ne 0) {
    Write-Host "    FAIL: --version exited with $LASTEXITCODE" -ForegroundColor Red
    $failed += "version"
} else {
    Write-Host "    PASS" -ForegroundColor Green
}

Write-Host "==> Step 2/4: --check-ytdlp (0 = up to date, 1 = newer available)" -ForegroundColor Cyan
& $BinaryPath --check-ytdlp
if ($LASTEXITCODE -notin 0, 1) {
    Write-Host "    FAIL: --check-ytdlp exited with $LASTEXITCODE" -ForegroundColor Red
    $failed += "check-ytdlp"
} else {
    Write-Host "    PASS" -ForegroundColor Green
}

if (!$SkipDownloads) {
    Write-Host "==> Step 3/4: download by URL ($DownloadUrl)" -ForegroundColor Cyan
    $urlDir = Join-Path $smokeDir "by_url"
    & $BinaryPath --link $DownloadUrl --output $urlDir --format mp3 --concurrency 1
    if ($LASTEXITCODE -ne 0) {
        Write-Host "    FAIL: download exited with $LASTEXITCODE" -ForegroundColor Red
        $failed += "download-by-url"
    }
    $mp3 = Get-ChildItem -LiteralPath $urlDir -Filter *.mp3 -File -ErrorAction SilentlyContinue |
        Where-Object { $_.Length -gt 10KB } |
        Select-Object -First 1
    if ($null -eq $mp3) {
        Write-Host "    FAIL: no mp3 larger than 10KB produced" -ForegroundColor Red
        $failed += "mp3-output"
    } else {
        Write-Host "    PASS: $($mp3.Name) ($([math]::Round($mp3.Length / 1KB)) KB)" -ForegroundColor Green
    }

    Write-Host "==> Step 4/4: download by search ('$SearchQuery')" -ForegroundColor Cyan
    $searchDir = Join-Path $smokeDir "by_search"
    & $BinaryPath --link $SearchQuery --output $searchDir --format mp3 --concurrency 1
    if ($LASTEXITCODE -ne 0) {
        Write-Host "    FAIL: search download exited with $LASTEXITCODE" -ForegroundColor Red
        $failed += "download-by-search"
    }
    $searched = Get-ChildItem -LiteralPath $searchDir -Filter *.mp3 -File -ErrorAction SilentlyContinue |
        Where-Object { $_.Length -gt 10KB } |
        Select-Object -First 1
    if ($null -eq $searched) {
        Write-Host "    FAIL: no mp3 larger than 10KB produced from search" -ForegroundColor Red
        $failed += "search-output"
    } else {
        Write-Host "    PASS: $($searched.Name) ($([math]::Round($searched.Length / 1KB)) KB)" -ForegroundColor Green
    }

    $manifests = @(Get-ChildItem -LiteralPath $smokeDir -Recurse -Filter "ghita_manifest_*.json" -File -ErrorAction SilentlyContinue)
    if ($manifests.Count -eq 0) {
        Write-Host "    FAIL: no ghita_manifest_*.json written" -ForegroundColor Red
        $failed += "manifest"
    } else {
        Write-Host "    PASS: manifest(s): $($manifests.Count)" -ForegroundColor Green
    }
}

Write-Host ""
if ($failed.Count -gt 0) {
    Write-Host "=== SMOKE FAILED ===" -ForegroundColor Red
    foreach ($item in $failed) {
        Write-Host "  - $item" -ForegroundColor Red
    }
    Write-Host "Artifacts kept at: $smokeDir" -ForegroundColor Yellow
    exit 1
}

Write-Host "=== SMOKE PASS ===" -ForegroundColor Green
if ($KeepOutput) {
    Write-Host "Artifacts kept at: $smokeDir" -ForegroundColor Yellow
} else {
    Remove-Item -LiteralPath $smokeDir -Recurse -Force
    Write-Host "Smoke directory cleaned." -ForegroundColor Yellow
}
exit 0
