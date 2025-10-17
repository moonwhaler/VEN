# Uninstallation script for ffmpeg-encoder on Windows

$ErrorActionPreference = "Stop"

# Set installation directories
$BinDir = Join-Path $env:USERPROFILE ".local\bin"
$ConfigDir = Join-Path $env:APPDATA "ffmpeg-encoder"
$BinaryName = "ffmpeg-encoder.exe"
$BinaryPath = Join-Path $BinDir $BinaryName

Write-Host "Uninstalling ffmpeg-encoder..." -ForegroundColor Cyan
Write-Host ""

# Remove binary
if (Test-Path $BinaryPath) {
    Write-Host "Removing binary from $BinDir..." -ForegroundColor Green
    Remove-Item -Path $BinaryPath -Force
    Write-Host "Binary removed." -ForegroundColor Green
} else {
    Write-Host "Binary not found at $BinaryPath" -ForegroundColor Yellow
}

# Ask about config directory
if (Test-Path $ConfigDir) {
    Write-Host ""
    Write-Host "Config directory found at $ConfigDir" -ForegroundColor Yellow
    $Response = Read-Host "Do you want to remove the config directory and all settings? (y/N)"

    if ($Response -match "^[Yy]$") {
        Remove-Item -Path $ConfigDir -Recurse -Force
        Write-Host "Config directory removed." -ForegroundColor Green
    } else {
        Write-Host "Config directory kept." -ForegroundColor Green
    }
} else {
    Write-Host "Config directory not found at $ConfigDir" -ForegroundColor Yellow
}

# Ask about PATH removal
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

if ($InPath) {
    Write-Host ""
    Write-Host "$BinDir is in your PATH." -ForegroundColor Yellow
    $Response = Read-Host "Do you want to remove it from PATH? (y/N)"

    if ($Response -match "^[Yy]$") {
        try {
            $NewPathArray = $PathsArray | Where-Object { $_.TrimEnd('\') -ne $NormalizedBinDir }
            $NewPath = $NewPathArray -join ";"
            [Environment]::SetEnvironmentVariable("Path", $NewPath, "User")

            Write-Host "Successfully removed from PATH." -ForegroundColor Green
            Write-Host "Note: You may need to restart your terminal for the PATH changes to take effect." -ForegroundColor Yellow
        } catch {
            Write-Host "Warning: Could not automatically remove from PATH." -ForegroundColor Yellow
            Write-Host "Please manually remove it from your environment variables." -ForegroundColor Yellow
        }
    } else {
        Write-Host "PATH entry kept." -ForegroundColor Green
    }
}

Write-Host ""
Write-Host "Uninstallation completed!" -ForegroundColor Green
