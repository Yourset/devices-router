$ErrorActionPreference = "Stop"

$root = Resolve-Path (Join-Path $PSScriptRoot "..")
$tauriRoot = Join-Path $root "src-tauri"
$configPath = Join-Path $tauriRoot "tauri.conf.json"
$config = Get-Content -Raw -Path $configPath | ConvertFrom-Json
$version = [string]$config.version
$bundleDir = Join-Path $tauriRoot "target\release\bundle\nsis"
$bundleInstallerName = "Devices Router_${version}_x64-setup.exe"
$installerName = "DevicesRouter_${version}_x64_setup.exe"
$installer = Join-Path $bundleDir $bundleInstallerName

if (-not (Test-Path -LiteralPath $installer)) {
  throw "Installer not found: $installer"
}

$updatesDir = Join-Path $tauriRoot "target\release\updates"
New-Item -ItemType Directory -Force -Path $updatesDir | Out-Null
$targetInstaller = Join-Path $updatesDir $installerName
Copy-Item -Force -LiteralPath $installer -Destination $targetInstaller

$file = Get-Item -LiteralPath $targetInstaller
$hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $targetInstaller).Hash.ToLowerInvariant()
$manifest = [ordered]@{
  version = $version
  files = [ordered]@{
    desktop = [ordered]@{
      version = $version
      path = $installerName
      size = $file.Length
      sha256 = $hash
      kind = "installer"
    }
  }
}

$manifestPath = Join-Path $updatesDir "manifest.json"
$manifest | ConvertTo-Json -Depth 8 | Set-Content -Encoding UTF8 -Path $manifestPath

$installedDir = Join-Path $env:LOCALAPPDATA "Devices Router"
if (Test-Path -LiteralPath $installedDir) {
  $installedUpdatesDir = Join-Path $installedDir "updates"
  New-Item -ItemType Directory -Force -Path $installedUpdatesDir | Out-Null
  Copy-Item -Force -LiteralPath $targetInstaller -Destination (Join-Path $installedUpdatesDir $installerName)
  Copy-Item -Force -LiteralPath $manifestPath -Destination (Join-Path $installedUpdatesDir "manifest.json")
}

Write-Host "LAN update package ready:"
Write-Host "  $targetInstaller"
Write-Host "  $manifestPath"
if (Test-Path -LiteralPath $installedDir) {
  Write-Host "  $installedDir\updates"
}
