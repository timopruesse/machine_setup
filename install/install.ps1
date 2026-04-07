# machine_setup installer for Windows
# Usage: irm https://raw.githubusercontent.com/timopruesse/machine_setup/main/install/install.ps1 | iex

$ErrorActionPreference = "Stop"

$Repo = "timopruesse/machine_setup"
$Binary = "machine_setup"
$Target = "x86_64-pc-windows-msvc"

Write-Host "Installing machine_setup..." -ForegroundColor Cyan
Write-Host ""

# Get latest version
Write-Host "Fetching latest release..."
$Release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest"
$Version = $Release.tag_name

if (-not $Version) {
    Write-Host "Error: Could not determine latest version." -ForegroundColor Red
    exit 1
}

Write-Host "Latest version: $Version"

# Download
$Artifact = "$Binary-$Target.zip"
$Url = "https://github.com/$Repo/releases/download/$Version/$Artifact"
$TmpDir = Join-Path $env:TEMP "machine_setup_install"

if (Test-Path $TmpDir) { Remove-Item -Recurse -Force $TmpDir }
New-Item -ItemType Directory -Path $TmpDir | Out-Null

Write-Host "Downloading $Artifact..."
Invoke-WebRequest -Uri $Url -OutFile (Join-Path $TmpDir $Artifact)

# Extract
Write-Host "Extracting..."
Expand-Archive -Path (Join-Path $TmpDir $Artifact) -DestinationPath $TmpDir -Force

# Install
$InstallDir = Join-Path $env:LOCALAPPDATA "machine_setup"
if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir | Out-Null
}

Copy-Item -Path (Join-Path $TmpDir "$Binary.exe") -Destination (Join-Path $InstallDir "$Binary.exe") -Force

# Cleanup
Remove-Item -Recurse -Force $TmpDir

# Add to PATH if needed
$UserPath = [Environment]::GetEnvironmentVariable("PATH", "User")
if ($UserPath -notlike "*$InstallDir*") {
    Write-Host "Adding $InstallDir to user PATH..."
    [Environment]::SetEnvironmentVariable("PATH", "$InstallDir;$UserPath", "User")
    $env:PATH = "$InstallDir;$env:PATH"
}

# Verify
$InstalledPath = Join-Path $InstallDir "$Binary.exe"
if (Test-Path $InstalledPath) {
    $VersionOutput = & $InstalledPath --version 2>&1
    Write-Host ""
    Write-Host "Installed $VersionOutput to $InstalledPath" -ForegroundColor Green
    Write-Host ""
    Write-Host "Restart your terminal, then run 'machine_setup --help' to get started."
} else {
    Write-Host ""
    Write-Host "Error: Installation failed." -ForegroundColor Red
    exit 1
}
