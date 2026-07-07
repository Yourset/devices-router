param(
    [string]$ServerHost,

    [int]$Port = 8765
)

$ErrorActionPreference = "Stop"

Push-Location $PSScriptRoot
try {
    if ($ServerHost) {
        .\.venv\Scripts\python -m flow_keyboard_bridge.client --host $ServerHost --port $Port
    } else {
        .\.venv\Scripts\python -m flow_keyboard_bridge.client --port $Port
    }
} finally {
    Pop-Location
}
