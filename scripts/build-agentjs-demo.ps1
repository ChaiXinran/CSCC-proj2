param(
    [string]$PackagingPython = "",
    [string]$OxideProjectRoot = ""
)

$ErrorActionPreference = "Stop"

$projectRoot = Split-Path -Parent $PSScriptRoot
Set-Location $projectRoot

if (-not $PackagingPython) {
    $isolatedPython = Join-Path $projectRoot ".cache\agentjs-packaging\Scripts\python.exe"
    $PackagingPython = if (Test-Path $isolatedPython) { $isolatedPython } else { "python" }
}
if (-not $OxideProjectRoot) {
    $OxideProjectRoot = if ($env:OXIDE_PROJECT_ROOT) {
        $env:OXIDE_PROJECT_ROOT
    } else {
        Join-Path (Split-Path -Parent $projectRoot) "project3136859-381686"
    }
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

Write-Host "Building release Boa comparison runtime..."
cargo build --release --locked --manifest-path boa/Cargo.toml -p boa_cli
if ($LASTEXITCODE -ne 0) { throw "Boa cargo build failed" }

Write-Host "Building release OxideJS comparison runtime..."
$oxideManifest = Join-Path $OxideProjectRoot "Cargo.toml"
if (-not (Test-Path $oxideManifest)) {
    throw "OxideJS project not found at $OxideProjectRoot; set OXIDE_PROJECT_ROOT"
}
cargo +1.94.0-x86_64-pc-windows-msvc build --release --locked -p oxide_cli `
    --manifest-path $oxideManifest --target-dir target/oxide-compare
if ($LASTEXITCODE -ne 0) { throw "OxideJS cargo build failed" }

Write-Host "Running orchestrator tests..."
& $PackagingPython -m unittest discover -s demo/agent/tests -v
if ($LASTEXITCODE -ne 0) { throw "orchestrator tests failed" }

Write-Host "Packaging standalone executable..."
& $PackagingPython -m PyInstaller --noconfirm --clean demo/agent/agentjs-demo.spec
if ($LASTEXITCODE -ne 0) { throw "PyInstaller failed" }

$output = Join-Path $projectRoot "dist\AgentJS-Demo.exe"
Write-Host "Created: $output"
