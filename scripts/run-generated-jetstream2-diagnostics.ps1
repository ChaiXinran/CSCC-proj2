param(
    [string]$GeneratedDirectory = "benchmarks/generated",
    [string]$JetStreamRoot = "benchmarks/JetStream2",
    [int]$TimeoutSeconds = 150,
    [int]$MaxWorkingSetMB = 1536,
    [string]$OutputDirectory = "reports/jetstream2-generated-2026-08-04"
)

$ErrorActionPreference = "Stop"
$binary = (Resolve-Path "target/release/agentjs.exe").Path
$resourceRoot = (Resolve-Path $JetStreamRoot).Path
$generatedRoot = (Resolve-Path $GeneratedDirectory).Path
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$summaryPath = Join-Path $OutputDirectory "summary.json"
$results = @()

$manifests = Get-ChildItem -LiteralPath $generatedRoot -Filter "*.manifest.json" |
    Sort-Object Name
foreach ($manifestFile in $manifests) {
    $runnerName = $manifestFile.Name -replace '\.manifest\.json$', '.js'
    $runner = Join-Path $generatedRoot $runnerName
    if (-not (Test-Path -LiteralPath $runner)) { throw "missing runner $runner" }
    $manifest = Get-Content -LiteralPath $manifestFile.FullName -Raw | ConvertFrom-Json
    if ($manifest.schemaVersion -ne 2) { throw "unsupported manifest schema for $runnerName" }

    $processInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $processInfo.FileName = $binary
    $processInfo.Arguments = 'jetstream "' + $runner + '" --resource-root "' +
        $resourceRoot + '" --diagnostics'
    $processInfo.WorkingDirectory = (Get-Location).Path
    $processInfo.UseShellExecute = $false
    $processInfo.RedirectStandardOutput = $true
    $processInfo.RedirectStandardError = $true
    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $processInfo
    $timer = [System.Diagnostics.Stopwatch]::StartNew()
    [void]$process.Start()
    $stdoutTask = $process.StandardOutput.ReadToEndAsync()
    $stderrTask = $process.StandardError.ReadToEndAsync()
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    [long]$peak = 0
    $memoryLimited = $false
    while (-not $process.HasExited -and [DateTime]::UtcNow -lt $deadline) {
        $process.Refresh()
        $peak = [math]::Max($peak, $process.WorkingSet64)
        if ($peak -gt $MaxWorkingSetMB * 1MB) {
            $memoryLimited = $true
            break
        }
        Start-Sleep -Milliseconds 50
    }
    $timedOut = -not $process.HasExited -and -not $memoryLimited
    if (-not $process.HasExited) {
        Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
        [void]$process.WaitForExit(10000)
    }
    $timer.Stop()
    $stdout = $stdoutTask.Result
    $stderr = $stderrTask.Result
    $combined = $stdout + "`n" + $stderr
    $exitCode = if ($process.HasExited) { $process.ExitCode } else { $null }

    $status = if ($memoryLimited) { "MEMORY_LIMIT" }
        elseif ($timedOut) { "TIMEOUT" }
        elseif ($combined -match 'JETSTREAM_RUN_COMPLETE' -and $exitCode -eq 0) { "PASS" }
        elseif ($combined -match 'Invalid totalLength|Wrong checksum') { "RESULT_MISMATCH" }
        elseif ($combined -match 'Assertion failure|Assertion failed') { "ASSERTION_FAILURE" }
        elseif ($combined -match 'undefined is not callable|not callable') { "CALL_ERROR" }
        else { "ENGINE_FAILURE" }
    $phases = @([regex]::Matches($combined, 'JETSTREAM_PHASE:\d+:([^\r\n]+)') |
        ForEach-Object { $_.Groups[1].Value })
    $lastPhase = if ($phases.Count) { $phases[-1] } else { $null }
    $detailMatch = [regex]::Match(
        $combined,
        'JetStream2 failed:\s*([^\r\n]+)|agentjs:\s*([^\r\n]+)'
    )
    $detail = if ($detailMatch.Success) {
        ($detailMatch.Groups[1].Value + $detailMatch.Groups[2].Value).Trim()
    } else { $null }
    $nameResolution = @([regex]::Matches(
        $combined,
        'name_resolution:load_local_count=(\d+) store_local_count=(\d+) load_name_count=(\d+) store_name_count=(\d+) environment_hops=(\d+)'
    ))
    [long]$loadLocalCount = 0
    [long]$storeLocalCount = 0
    [long]$loadNameCount = 0
    [long]$storeNameCount = 0
    [long]$environmentHops = 0
    foreach ($sample in $nameResolution) {
        $loadLocalCount += [long]$sample.Groups[1].Value
        $storeLocalCount += [long]$sample.Groups[2].Value
        $loadNameCount += [long]$sample.Groups[3].Value
        $storeNameCount += [long]$sample.Groups[4].Value
        $environmentHops += [long]$sample.Groups[5].Value
    }
    $localAccesses = $loadLocalCount + $storeLocalCount
    $namedAccesses = $loadNameCount + $storeNameCount
    $localFastPathPercent = if ($localAccesses + $namedAccesses) {
        [math]::Round(100 * $localAccesses / ($localAccesses + $namedAccesses), 2)
    } else { 0 }
    $logPath = Join-Path $OutputDirectory ($runnerName -replace '\.js$', '.txt')
    Set-Content -LiteralPath $logPath -Encoding utf8 -Value $combined
    $result = [pscustomobject]@{
        runner = $runnerName
        benchmark = $manifest.benchmark
        runnerSha256 = $manifest.runnerSha256
        requestedIterations = $manifest.requestedIterations
        status = $status
        wallSeconds = [math]::Round($timer.Elapsed.TotalSeconds, 3)
        peakWorkingSetMiB = [math]::Round($peak / 1MB, 1)
        exitCode = $exitCode
        timedOut = $timedOut
        memoryLimited = $memoryLimited
        resourceCount = @($manifest.resourceHashes.PSObject.Properties).Count
        lastPhase = $lastPhase
        completed = $combined -match 'JETSTREAM_RUN_COMPLETE'
        detail = $detail
        nameResolutionSamples = $nameResolution.Count
        loadLocalCount = $loadLocalCount
        storeLocalCount = $storeLocalCount
        loadNameCount = $loadNameCount
        storeNameCount = $storeNameCount
        environmentHops = $environmentHops
        localFastPathPercent = $localFastPathPercent
        log = (Split-Path -Leaf $logPath)
    }
    $results += $result
    $results | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $summaryPath -Encoding utf8
    Write-Output ("{0,-36} {1,-18} {2,8:N3}s {3,8:N1} MiB" -f `
        $runnerName, $status, $result.wallSeconds, $result.peakWorkingSetMiB)
}

Write-Output "summary=$summaryPath"
