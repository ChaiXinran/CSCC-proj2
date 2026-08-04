param(
    [int]$Iterations = 1,
    [switch]$VerifyDeterminism,
    [string]$JetStreamRoot = "benchmarks/JetStream2"
)

$ErrorActionPreference = "Stop"
$runners = [ordered]@{
    "ai-astar.js" = "ai-astar"
    "crypto.js" = "crypto"
    "gaussian-blur.js" = "gaussian-blur"
    "hash-map.js" = "hash-map"
    "jetstream2-cdjs.js" = "cdjs"
    "jetstream2-intl.js" = "intl"
    "jetstream2-jsdom-d3-startup.js" = "jsdom-d3-startup"
    "jetstream2-mobx.js" = "mobx"
    "jetstream2-threejs.js" = "threejs"
    "jetstream2-validatorjs.js" = "validatorjs"
    "jetstream2-web-ssr.js" = "web-ssr"
    "jetstream2-WSL.js" = "WSL"
    "navier-stokes.js" = "navier-stokes"
    "raytrace.js" = "raytrace"
    "regexp.js" = "regexp-octane"
    "richards.js" = "richards"
    "splay.js" = "splay"
    "stanford-crypto-sha256.js" = "stanford-crypto-sha256"
    "test-cdjs.js" = "cdjs"
}

$generatedDirectory = "benchmarks/generated"
$results = @()
foreach ($entry in $runners.GetEnumerator()) {
    $output = Join-Path $generatedDirectory $entry.Key
    $generatorOutput = node scripts/prepare-jetstream2.mjs `
        $JetStreamRoot $entry.Value $Iterations $output
    if ($LASTEXITCODE -ne 0) {
        throw "failed to generate $($entry.Value)"
    }
    $plan = $generatorOutput | ConvertFrom-Json
    $manifest = Get-Content $plan.manifest -Raw | ConvertFrom-Json
    $deterministic = $null
    if ($VerifyDeterminism) {
        $verificationDirectory = "target/jetstream2-determinism"
        New-Item -ItemType Directory -Force -Path $verificationDirectory | Out-Null
        $verificationOutput = Join-Path $verificationDirectory $entry.Key
        node scripts/prepare-jetstream2.mjs `
            $JetStreamRoot $entry.Value $Iterations $verificationOutput | Out-Null
        if ($LASTEXITCODE -ne 0) {
            throw "failed repeat generation for $($entry.Value)"
        }
        $verificationManifest = Get-Content `
            ($verificationOutput -replace '\.js$', '.manifest.json') -Raw | ConvertFrom-Json
        $deterministic = $manifest.runnerSha256 -eq $verificationManifest.runnerSha256
        if (-not $deterministic) {
            throw "non-deterministic runner generation for $($entry.Value)"
        }
    }
    $results += [pscustomobject]@{
        benchmark = $entry.Value
        runner = $entry.Key
        manifest = Split-Path -Leaf $plan.manifest
        entryFileCount = @($plan.files).Count
        preloadFileCount = @($plan.preloadFiles).Count
        runnerSha256 = $manifest.runnerSha256
        deterministic = $deterministic
    }
}

$results | Format-Table -AutoSize
Write-Output "generated=$($results.Count)"
