$ErrorActionPreference = "Stop"

Push-Location $PSScriptRoot
try {
    if (-not (Test-Path ".venv")) {
        python -m venv .venv
    }
    .\.venv\Scripts\python -m pip install --upgrade pip
    .\.venv\Scripts\python -m pip install -r requirements.txt
    Write-Host "Installed. Run .\run-server.ps1 on the keyboard computer."
} finally {
    Pop-Location
}

