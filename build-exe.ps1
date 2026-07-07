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

    Write-Host "Built:"
    Write-Host "  $PSScriptRoot\dist\flow-keyboard-server.exe"
    Write-Host "  $PSScriptRoot\dist\flow-keyboard-client.exe"
    Write-Host "  $PSScriptRoot\dist\FlowKeyboardHost.exe"
    Write-Host "  $PSScriptRoot\dist\FlowKeyboardRemote.exe"
} finally {
    Pop-Location
}
