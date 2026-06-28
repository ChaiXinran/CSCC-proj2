param(
    [Parameter(Mandatory = $true)]
    [string[]]$Tests,
    [int]$Iterations = 0,
    [string]$JetStreamRoot = "benchmarks/JetStream2"
)

$ErrorActionPreference = "Stop"
$binary = Resolve-Path "target/release/agentjs.exe"
$generated = "benchmarks/generated"
$reportDirectory = "reports/jetstream2"
New-Item -ItemType Directory -Force -Path $generated, $reportDirectory | Out-Null

foreach ($test in $Tests) {
    $runner = Join-Path $generated "$test.js"
    node scripts/prepare-jetstream2.mjs `
        $JetStreamRoot `
        $test `
        $Iterations `
        $runner | Set-Content (Join-Path $reportDirectory "$test-plan.json")

    $started = Get-Date
    $previousErrorActionPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    $output = & $binary jetstream $runner 2>&1
    $exitCode = $LASTEXITCODE
    $ErrorActionPreference = $previousErrorActionPreference
    $output | Tee-Object -FilePath (Join-Path $reportDirectory "$test.txt")
    $elapsed = (Get-Date) - $started
    "wall_time_ms=$([math]::Round($elapsed.TotalMilliseconds))" |
        Add-Content (Join-Path $reportDirectory "$test.txt")
    if ($exitCode -ne 0) {
        throw "JetStream workload '$test' failed with exit code $exitCode"
    }
}
