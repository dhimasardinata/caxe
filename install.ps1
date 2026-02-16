$ErrorActionPreference = "Stop"

$Repo = "dhimasardinata/caxe"
$InstallDir = "$env:USERPROFILE\.cx\bin"
$BinName = "cx.exe"

Write-Output "Installing caxe (cx)..."

# 1. Create Install Directory
if (!(Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
}

# 2. Get Latest Release Info
Write-Output "Fetching latest release..."
try {
    $ReleaseUrl = "https://api.github.com/repos/$Repo/releases/latest"
    $Latest = Invoke-RestMethod -Uri $ReleaseUrl
    $Tag = $Latest.tag_name
    Write-Output "Latest version: $Tag"
} catch {
    Write-Error "Failed to fetch release info. Check your internet connection."
}

# 3. Determine Download URL (Windows)
$Asset = $Latest.assets | Where-Object { $_.name -like "*.exe" } | Select-Object -First 1

if (!$Asset) {
    Write-Error "No Windows binary found in the latest release."
}

$DownloadUrl = $Asset.browser_download_url
$DestPath = "$InstallDir\$BinName"

# 4. Download
Write-Output "Downloading from $DownloadUrl..."
Invoke-WebRequest -Uri $DownloadUrl -OutFile $DestPath

# 5. Add to PATH
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($UserPath -notlike "*$InstallDir*") {
    Write-Output "Adding $InstallDir to User PATH..."
    [Environment]::SetEnvironmentVariable("Path", "$UserPath;$InstallDir", "User")
    Write-Output "Path updated. You may need to restart your terminal."
} else {
    Write-Output "$InstallDir is already in PATH."
}

Write-Output "Success! Run 'cx --version' to get started."
