@echo off
REM FFmpeg Autoencoder - Windows Build Script
REM Usage: build.bat [production|dev]

setlocal enabledelayedexpansion

REM Display banner
echo ========================================
echo    FFmpeg Autoencoder Build Script
echo ========================================
echo.

REM Check if Rust is installed
where cargo >nul 2>nul
if %ERRORLEVEL% neq 0 (
    echo [ERROR] Rust/Cargo is not installed.
    echo Please install Rust from: https://rustup.rs/
    exit /b 1
)

REM Determine build mode
set BUILD_MODE=%1
if "%BUILD_MODE%"=="" set BUILD_MODE=production

if /i "%BUILD_MODE%"=="production" goto :build_production
if /i "%BUILD_MODE%"=="prod" goto :build_production
if /i "%BUILD_MODE%"=="dev" goto :build_dev
if /i "%BUILD_MODE%"=="development" goto :build_dev

echo [ERROR] Invalid build mode '%BUILD_MODE%'
echo Usage: %0 [production^|dev]
echo.
echo Build modes:
echo   production, prod  - Optimized release build (slower compile, faster runtime)
echo   dev, development  - Debug build (faster compile, includes debug symbols)
exit /b 1

:build_production
echo [INFO] Building in PRODUCTION mode...
echo Optimizations: Maximum (opt-level=3, LTO enabled)
echo.

cargo build --release
if %ERRORLEVEL% neq 0 (
    echo.
    echo [ERROR] Production build failed!
    exit /b 1
)

set BINARY_PATH=.\target\release\ffmpeg-encoder.exe
echo.
echo [SUCCESS] Production build completed successfully!
goto :display_info

:build_dev
echo [INFO] Building in DEVELOPMENT mode...
echo Optimizations: Minimal (debug info included)
echo.

cargo build
if %ERRORLEVEL% neq 0 (
    echo.
    echo [ERROR] Development build failed!
    exit /b 1
)

set BINARY_PATH=.\target\debug\ffmpeg-encoder.exe
echo.
echo [SUCCESS] Development build completed successfully!
goto :display_info

:display_info
REM Display binary information
if exist "%BINARY_PATH%" (
    echo.
    echo Binary location: %BINARY_PATH%

    REM Get file size
    for %%A in ("%BINARY_PATH%") do (
        set SIZE=%%~zA
        set /a SIZE_MB=!SIZE! / 1048576
        echo Binary size: !SIZE_MB! MB
    )

    echo.
    echo You can now run the encoder with:
    echo   %BINARY_PATH% --help
    echo.
    echo To add to PATH (optional):
    echo   1. Copy %BINARY_PATH% to a permanent location
    echo   2. Add that location to your system PATH
)

echo.
echo [SUCCESS] Build process complete!
exit /b 0
