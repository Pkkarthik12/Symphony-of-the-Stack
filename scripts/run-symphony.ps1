# Run Symphony of the Stack locally
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root
$env:SYMPHONY_WEB_DIR = Join-Path $Root "web"
Write-Host "Symphony UI: http://localhost:8765"
cargo run -p symphony-bridge --release
