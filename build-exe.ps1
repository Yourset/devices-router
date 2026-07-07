$ErrorActionPreference = "Stop"

Push-Location $PSScriptRoot
try {
    if (-not (Test-Path ".venv")) {
        python -m venv .venv
    }
    .\.venv\Scripts\python -m pip install -r requirements.txt

    Remove-Item -Recurse -Force -ErrorAction SilentlyContinue build
    Remove-Item -Recurse -Force -ErrorAction SilentlyContinue dist

    .\.venv\Scripts\python -m PyInstaller `
        --onefile `
        --console `
        --name flow-keyboard-server `
        --collect-submodules pynput `
        .\flow_keyboard_server.py

    .\.venv\Scripts\python -m PyInstaller `
        --onefile `
        --console `
        --name flow-keyboard-client `
        --collect-submodules pynput `
        .\flow_keyboard_client.py

    .\.venv\Scripts\python -m PyInstaller `
        --onefile `
        --windowed `
        --name FlowKeyboardHost `
        --collect-submodules pynput `
        .\flow_keyboard_host_app.py

    .\.venv\Scripts\python -m PyInstaller `
        --onefile `
        --windowed `
        --name FlowKeyboardRemote `
        --collect-submodules pynput `
        .\flow_keyboard_remote_app.py

    $updatesDir = Join-Path $PSScriptRoot "dist\updates"
    New-Item -ItemType Directory -Force -Path $updatesDir | Out-Null
    Copy-Item -Force -LiteralPath (Join-Path $PSScriptRoot "dist\FlowKeyboardHost.exe") -Destination (Join-Path $updatesDir "FlowKeyboardHost.exe")
    Copy-Item -Force -LiteralPath (Join-Path $PSScriptRoot "dist\FlowKeyboardRemote.exe") -Destination (Join-Path $updatesDir "FlowKeyboardRemote.exe")
    $version = (& .\.venv\Scripts\python -c "from flow_keyboard_bridge.app_info import APP_VERSION; print(APP_VERSION)").Trim()
    $manifest = [ordered]@{
        version = $version
        files = [ordered]@{
            host = [ordered]@{
                version = $version
                path = "FlowKeyboardHost.exe"
            }
            remote = [ordered]@{
                version = $version
                path = "FlowKeyboardRemote.exe"
            }
        }
    }
    $manifest | ConvertTo-Json -Depth 5 | Set-Content -Encoding UTF8 -LiteralPath (Join-Path $updatesDir "manifest.json")

    Write-Host "Built:"
    Write-Host "  $PSScriptRoot\dist\flow-keyboard-server.exe"
    Write-Host "  $PSScriptRoot\dist\flow-keyboard-client.exe"
    Write-Host "  $PSScriptRoot\dist\FlowKeyboardHost.exe"
    Write-Host "  $PSScriptRoot\dist\FlowKeyboardRemote.exe"
    Write-Host "  $PSScriptRoot\dist\updates\manifest.json"
} finally {
    Pop-Location
}
