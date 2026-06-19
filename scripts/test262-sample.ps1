param(
    [int]$Limit = 1000,
    [int]$Jobs = [Environment]::ProcessorCount,
    [string]$Suite = "test/language"
)

$ErrorActionPreference = "Stop"
New-Item -ItemType Directory -Force -Path "reports" | Out-Null

cargo run --release -- test262 `
    --root test262 `
    --suite $Suite `
    --limit $Limit `
    --jobs $Jobs `
    --json reports/test262-sample.json `
    --verbose

