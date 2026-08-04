param(
    [Parameter(Mandatory = $true)]
    [string[]]$Tests,
    [int]$Iterations = 0,
    [int]$TimeoutSeconds = 150,
    [string]$JetStreamRoot = "benchmarks/JetStream2"
)

$ErrorActionPreference = "Stop"
$binary = Resolve-Path "target/release/agentjs.exe"
$reportDirectory = "reports/jetstream2"
New-Item -ItemType Directory -Force -Path $reportDirectory | Out-Null

$results = @()
foreach ($test in $Tests) {
    $runner = Join-Path $reportDirectory "$test-runner.js"
    node scripts/prepare-jetstream2.mjs `
        $JetStreamRoot `
        $test `
        $Iterations `
        $runner | Set-Content (Join-Path $reportDirectory "$test-plan.json")

    $verificationJson = node scripts/verify-generated-runner.mjs `
        $runner $binary ($TimeoutSeconds * 1000)
    if ($LASTEXITCODE -ne 0) {
        throw "generated runner verification failed for '$test'"
    }
    $verification = $verificationJson | ConvertFrom-Json
    $verificationJson | Set-Content -Encoding utf8 `
        (Join-Path $reportDirectory "$test.json")
    $results += $verification
}

$summary = Join-Path $reportDirectory "summary.json"
$results | ConvertTo-Json -Depth 5 | Set-Content -Encoding utf8 $summary
$results | Select-Object benchmark, workloadStatus, exitCode, timedOut |
    Format-Table -AutoSize
Write-Output "summary=$summary"

if (@($results | Where-Object workloadStatus -ne "PASS").Count) {
    exit 1
}
