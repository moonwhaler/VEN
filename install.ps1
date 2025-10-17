# Installation script for ffmpeg-encoder on Windows
# This script installs the binary to %USERPROFILE%\.local\bin and config to %APPDATA%\ffmpeg-encoder

# Requires PowerShell 5.1 or later

param(
    [switch]$Force = $false
)

$ErrorActionPreference = "Stop"

# Set installation directories
$BinDir = Join-Path $env:USERPROFILE ".local\bin"
$ConfigDir = Join-Path $env:APPDATA "ffmpeg-encoder"
$BinaryName = "ffmpeg-encoder.exe"

Write-Host "Installing ffmpeg-encoder for Windows..." -ForegroundColor Cyan
Write-Host ""

# Check if binary exists
$BinarySource = ".\target\release\$BinaryName"
if (-not (Test-Path $BinarySource)) {
    Write-Host "Error: Binary not found at $BinarySource" -ForegroundColor Red
    Write-Host "Please run 'cargo build --release' first" -ForegroundColor Yellow
    exit 1
}

# Create directories if they don't exist
Write-Host "Creating installation directories..." -ForegroundColor Green
New-Item -ItemType Directory -Force -Path $BinDir | Out-Null
New-Item -ItemType Directory -Force -Path $ConfigDir | Out-Null

# Install binary
Write-Host "Installing binary to $BinDir..." -ForegroundColor Green
Copy-Item -Path $BinarySource -Destination (Join-Path $BinDir $BinaryName) -Force

# Install default config if user doesn't have one
$UserConfig = Join-Path $ConfigDir "config.yaml"
if (-not (Test-Path $UserConfig)) {
    $DefaultConfig = ".\config\config.default.yaml"
    if (Test-Path $DefaultConfig) {
        Write-Host "Installing default config to $ConfigDir..." -ForegroundColor Green
        Copy-Item -Path $DefaultConfig -Destination $UserConfig -Force
        Write-Host ""
        Write-Host "Note: A default config has been created at $UserConfig" -ForegroundColor Yellow
        Write-Host "Please edit this file to configure your encoding settings." -ForegroundColor Yellow
    } else {
        Write-Host ""
        Write-Host "Warning: No default config found. You'll need to create $UserConfig manually." -ForegroundColor Yellow
    }
} else {
    Write-Host "Existing config found at $UserConfig - keeping it." -ForegroundColor Green
}

# Check if $BinDir is in PATH
$CurrentPath = [Environment]::GetEnvironmentVariable("Path", "User")
$PathsArray = $CurrentPath -split ";"
$NormalizedBinDir = $BinDir.TrimEnd('\')

$InPath = $false
foreach ($p in $PathsArray) {
    if ($p.TrimEnd('\') -eq $NormalizedBinDir) {
        $InPath = $true
        break
    }
}

if (-not $InPath) {
    Write-Host ""
    Write-Host "Adding $BinDir to your PATH..." -ForegroundColor Yellow

    try {
        $NewPath = "$CurrentPath;$BinDir"
        [Environment]::SetEnvironmentVariable("Path", $NewPath, "User")

        # Also update current session
        $env:Path = "$env:Path;$BinDir"

        Write-Host "Successfully added to PATH." -ForegroundColor Green
        Write-Host "Note: You may need to restart your terminal for the PATH changes to take effect." -ForegroundColor Yellow
    } catch {
        Write-Host "Warning: Could not automatically add to PATH." -ForegroundColor Yellow
        Write-Host "Please manually add the following directory to your PATH:" -ForegroundColor Yellow
        Write-Host "  $BinDir" -ForegroundColor White
        Write-Host ""
        Write-Host "To add it manually:" -ForegroundColor Yellow
        Write-Host "  1. Open System Properties > Environment Variables" -ForegroundColor White
        Write-Host "  2. Under User variables, select 'Path' and click 'Edit'" -ForegroundColor White
        Write-Host "  3. Click 'New' and add: $BinDir" -ForegroundColor White
    }
} else {
    Write-Host "$BinDir is already in your PATH." -ForegroundColor Green
}

Write-Host ""
Write-Host "Installation completed successfully!" -ForegroundColor Green
Write-Host ""
Write-Host "Binary installed to: $(Join-Path $BinDir $BinaryName)" -ForegroundColor Cyan
Write-Host "Config directory: $ConfigDir" -ForegroundColor Cyan
Write-Host ""
Write-Host "You can now run: ffmpeg-encoder --help" -ForegroundColor White
Write-Host ""
Write-Host "To uninstall, run: .\uninstall.ps1" -ForegroundColor White
