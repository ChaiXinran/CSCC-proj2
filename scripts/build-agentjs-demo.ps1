param(
    [string]$PackagingPython = ""
)

$ErrorActionPreference = "Stop"

$projectRoot = Split-Path -Parent $PSScriptRoot
Set-Location $projectRoot

if (-not $PackagingPython) {
    $isolatedPython = Join-Path $projectRoot ".cache\agentjs-packaging\Scripts\python.exe"
    $PackagingPython = if (Test-Path $isolatedPython) { $isolatedPython } else { "python" }
}

& $PackagingPython -m PyInstaller --version *> $null
if ($LASTEXITCODE -ne 0) {
    throw "PyInstaller is not installed for $PackagingPython. Run: $PackagingPython -m pip install pyinstaller"
}
& $PackagingPython -c "import webview" *> $null
if ($LASTEXITCODE -ne 0) {
    throw "pywebview is not installed for $PackagingPython. Run: $PackagingPython -m pip install pywebview"
}

Write-Host "Building release AgentJS runtime..."
cargo build --release --locked
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

Write-Host "Running orchestrator tests..."
& $PackagingPython -m unittest discover -s demo/agent/tests -v
if ($LASTEXITCODE -ne 0) { throw "orchestrator tests failed" }

Write-Host "Packaging standalone executable..."
& $PackagingPython -m PyInstaller --noconfirm --clean demo/agent/agentjs-demo.spec
if ($LASTEXITCODE -ne 0) { throw "PyInstaller failed" }

$output = Join-Path $projectRoot "dist\AgentJS-Demo.exe"
Write-Host "Created: $output"
