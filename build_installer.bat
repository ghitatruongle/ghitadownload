@echo off
setlocal
chcp 65001 >nul
echo ==================================================================
echo   ĐANG BIÊN DỊCH GHITA DOWNLOADER VÀ TỆP CÀI ĐẶT SETUP.EXE
echo ==================================================================
if not exist bin\yt-dlp.exe (
    echo [LỖI] Thiếu bin\yt-dlp.exe.
    exit /b 1
)
for %%F in (bin\yt-dlp.exe) do if %%~zF LSS 100 (
    echo [LỖI] bin\yt-dlp.exe không hợp lệ hoặc quá nhỏ.
    exit /b 1
)
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\verify_dependencies.ps1
if %ERRORLEVEL% neq 0 (
    echo [LỖI] Xác minh yt-dlp thất bại.
    exit /b 1
)

echo [1/5] Kiểm tra format và test bằng lockfile...
cargo fmt --all -- --check
if %ERRORLEVEL% neq 0 (
    echo [LỖI] Kiểm tra format thất bại.
    exit /b 1
)
cargo test --locked --all-targets
if %ERRORLEVEL% neq 0 (
    echo [LỖI] Test suite thất bại.
    exit /b 1
)

echo [2/5] Biên dịch ứng dụng chính bằng lockfile...
cargo build --locked --release --bin ghitadownload
if %ERRORLEVEL% neq 0 (
    echo [LỖI] Không thể biên dịch ghitadownload.exe!
    exit /b 1
)
if not exist target\release\ghitadownload.exe (
    echo [LỖI] Chưa có bản build ghitadownload.exe.
    exit /b 1
)
for %%F in (target\release\ghitadownload.exe) do if %%~zF LSS 100 (
    echo [LỖI] bản build ghitadownload.exe không hợp lệ hoặc quá nhỏ.
    exit /b 1
)

echo [3/5] Đóng gói tệp cài đặt tạm bằng lockfile...
set "GHITA_REQUIRE_EMBED=1"
cargo build --locked --release --bin ghitadownload-setup
set "BUILD_STATUS=%ERRORLEVEL%"
set "GHITA_REQUIRE_EMBED="
if not "%BUILD_STATUS%"=="0" (
    echo [LỖI] Không thể đóng gói tệp cài đặt!
    exit /b 1
)

set "VERSION="
for /f "tokens=3 delims= " %%v in ('findstr /r /c:"^version = " Cargo.toml') do set "VERSION=%%~v"
if "%VERSION%"=="" (
    echo [LỖI] Không đọc được version từ Cargo.toml.
    exit /b 1
)
if not exist Release mkdir Release
set "CANDIDATE=Release\ghitadownload_%VERSION%.candidate.exe"
copy /y target\release\ghitadownload-setup.exe "%CANDIDATE%" >nul
if %ERRORLEVEL% neq 0 (
    echo [LỖI] Không thể tạo artifact kiểm chứng tạm.
    exit /b 1
)

echo [4/5] Xác minh artifact tạm trước khi thay thế artifact công khai...
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\verify_release.ps1 -ArtifactPath "%CANDIDATE%"
if %ERRORLEVEL% neq 0 (
    del /q "%CANDIDATE%" 2>nul
    echo [LỖI] Xác minh bản phát hành thất bại; artifact cũ không bị thay thế.
    exit /b 1
)
copy /y "%CANDIDATE%" "Release\ghitadownload_%VERSION%.exe" >nul
if %ERRORLEVEL% neq 0 (
    del /q "%CANDIDATE%" 2>nul
    echo [LỖI] Không thể công bố artifact đã xác minh.
    exit /b 1
)
echo [5/5] Công bố artifact đã xác minh...
del /q "%CANDIDATE%" 2>nul

echo.
echo ==================================================================
echo   HOAN TAT! TEP CAI DAT DA SAN SANG: Release\ghitadownload_%VERSION%.exe
echo ==================================================================
pause
