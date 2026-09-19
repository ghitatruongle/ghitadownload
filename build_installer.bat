@echo off
chcp 65001 >nul
echo ==================================================================
echo   ĐANG BIÊN DỊCH GHITA DOWNLOADER VÀ TỆP CÀI ĐẶT SETUP.EXE
echo ==================================================================
echo [1/2] Biên dịch ứng dụng chính (ghitadownload.exe)...
cargo build --release --bin ghitadownload
if %ERRORLEVEL% neq 0 (
    echo [LỖI] Không thể biên dịch ghitadownload.exe!
    pause
    exit /b 1
)

echo [2/2] Đóng gói tệp cài đặt duy nhất (ghitadownload-setup.exe)...
cargo build --release --bin ghitadownload-setup
if %ERRORLEVEL% neq 0 (
    echo [LỖI] Không thể đóng gói tệp cài đặt!
    pause
    exit /b 1
)

for /f "tokens=3 delims= " %%v in ('findstr /r "^version = " Cargo.toml') do set VERSION=%%~v
set VERSION=%VERSION:"=%
if "%VERSION%"=="" set VERSION=0.0.0

if not exist Release mkdir Release
copy /y target\release\ghitadownload-setup.exe Release\ghitadownload_%VERSION%.exe >nul
echo.
echo ==================================================================
echo   🎉 HOÀN TẤT! TỆP CÀI ĐẶT ĐÃ SẴN SÀNG: Release\ghitadownload_%VERSION%.exe
echo ==================================================================
pause
