$ErrorActionPreference = "Stop"

Push-Location $PSScriptRoot
try {
    .\.venv\Scripts\python -m flow_keyboard_bridge.server
} finally {
    Pop-Location
}

