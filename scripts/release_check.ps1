param(
    [string]$Version = ""
)

$ErrorActionPreference = "Stop"

Write-Host "=== Ghita Downloader release check ===" -ForegroundColor Cyan
Write-Host ""

Write-Host "==> Step 1/5: cargo fmt --all -- --check" -ForegroundColor Cyan
cargo fmt --all -- --check
if ($LASTEXITCODE -ne 0) {
    Write-Host "RELEASE CHECK FAIL: formatting" -ForegroundColor Red
    exit 1
}
Write-Host "    PASS" -ForegroundColor Green

Write-Host "==> Step 2/5: cargo clippy --locked --all-targets -D warnings" -ForegroundColor Cyan
cargo clippy --locked --all-targets -- -D warnings
if ($LASTEXITCODE -ne 0) {
    Write-Host "RELEASE CHECK FAIL: clippy" -ForegroundColor Red
    exit 1
}
Write-Host "    PASS" -ForegroundColor Green

Write-Host "==> Step 3/5: cargo test --locked --all-targets" -ForegroundColor Cyan
cargo test --locked --all-targets
if ($LASTEXITCODE -ne 0) {
    Write-Host "RELEASE CHECK FAIL: test suite" -ForegroundColor Red
    exit 1
}
Write-Host "    PASS" -ForegroundColor Green

Write-Host "==> Step 4/5: yt-dlp pin check" -ForegroundColor Cyan
& (Join-Path $PSScriptRoot "check_ytdlp_pin.ps1")
if ($LASTEXITCODE -ne 0) {
    Write-Host "RELEASE CHECK FAIL: yt-dlp pin check" -ForegroundColor Red
    exit 1
}
Write-Host "    PASS" -ForegroundColor Green

Write-Host "==> Step 5/5: release artifact verification" -ForegroundColor Cyan
if ([string]::IsNullOrWhiteSpace($Version)) {
    $versionLine = Select-String -Path "Cargo.toml" -Pattern '^version = "(.+)"$' | Select-Object -First 1
    if ($null -eq $versionLine) {
        throw "Cannot read version from Cargo.toml"
    }
    $Version = $versionLine.Matches[0].Groups[1].Value
}
$artifact = Join-Path "Release" "ghitadownload_$Version.exe"
if (Test-Path -LiteralPath $artifact -PathType Leaf) {
    & (Join-Path $PSScriptRoot "verify_release.ps1") -ArtifactPath $artifact
    if ($LASTEXITCODE -ne 0) {
        Write-Host "RELEASE CHECK FAIL: verify_release" -ForegroundColor Red
        exit 1
    }
    Write-Host "    PASS" -ForegroundColor Green
}
else {
    Write-Host "    SKIP: artifact not found: $artifact" -ForegroundColor Yellow
    Write-Host "    Build first with build_installer.bat, then re-run this script." -ForegroundColor Yellow
}

Write-Host ""
Write-Host "RELEASE CHECK PASS" -ForegroundColor Green
exit 0
