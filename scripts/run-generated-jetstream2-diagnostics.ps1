param(
    [string]$GeneratedDirectory = "benchmarks/generated",
    [string]$JetStreamRoot = "benchmarks/JetStream2",
    [int]$TimeoutSeconds = 150,
    [int]$MaxWorkingSetMB = 1536,
    [ValidateRange(4, 256)]
    [int]$ThreadStackMiB = 32,
    [long]$GcThreshold = 1000000,
    [string[]]$Tests = @(),
    [string]$OutputDirectory = "reports/jetstream2-generated-2026-08-04"
)

$ErrorActionPreference = "Stop"
$binary = (Resolve-Path "target/release/agentjs.exe").Path
$resourceRoot = (Resolve-Path $JetStreamRoot).Path
$generatedRoot = (Resolve-Path $GeneratedDirectory).Path
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$summaryPath = Join-Path $OutputDirectory "summary.json"
$results = @()
$selectedTests = @($Tests | ForEach-Object { $_ -split ',' } | Where-Object { $_ })

$manifests = Get-ChildItem -LiteralPath $generatedRoot -Filter "*.manifest.json" |
    Sort-Object Name
foreach ($manifestFile in $manifests) {
    $runnerName = $manifestFile.Name -replace '\.manifest\.json$', '.js'
    $testName = $runnerName -replace '\.js$', ''
    $workloadName = $testName -replace '^jetstream2-', ''
    if ($selectedTests.Count -gt 0 -and
        $selectedTests -notcontains $testName -and
        $selectedTests -notcontains $workloadName) { continue }
    $runner = Join-Path $generatedRoot $runnerName
    if (-not (Test-Path -LiteralPath $runner)) { throw "missing runner $runner" }
    $manifest = Get-Content -LiteralPath $manifestFile.FullName -Raw | ConvertFrom-Json
    if ($manifest.schemaVersion -ne 2) { throw "unsupported manifest schema for $runnerName" }

    $processInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $processInfo.FileName = $binary
    $processInfo.Arguments = 'jetstream "' + $runner + '" --resource-root "' +
        $resourceRoot + '" --thread-stack-mib ' + $ThreadStackMiB +
        ' --gc-threshold ' + $GcThreshold + ' --diagnostics'
    $processInfo.WorkingDirectory = (Get-Location).Path
    $processInfo.UseShellExecute = $false
    $processInfo.RedirectStandardOutput = $true
    $processInfo.RedirectStandardError = $true
    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $processInfo
    $timer = [System.Diagnostics.Stopwatch]::StartNew()
    [void]$process.Start()
    $stdoutTask = $process.StandardOutput.ReadToEndAsync()
    $stderrLines = [System.Collections.Generic.List[string]]::new()
    $stderrReadTask = $process.StandardError.ReadLineAsync()
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    [long]$peak = 0
    $peakPhase = "process_start"
    $currentPhase = "process_start"
    $rssSamples = [System.Collections.Generic.List[object]]::new()
    $memoryLimited = $false
    while (-not $process.HasExited -and [DateTime]::UtcNow -lt $deadline) {
        while ($stderrReadTask.IsCompleted) {
            $line = $stderrReadTask.Result
            if ($null -eq $line) { break }
            $stderrLines.Add($line)
            if ($line -match '^phase_diagnostics:phase=([^ ]+)') {
                $currentPhase = $Matches[1]
            } elseif ($line -match '^(runner_read_start|runner_read_end|resource_read_start|resource_read_end|job_drain_start|job_drain_end|run_end)') {
                $currentPhase = $Matches[1]
            }
            $stderrReadTask = $process.StandardError.ReadLineAsync()
        }
        $process.Refresh()
        $workingSet = $process.WorkingSet64
        if ($workingSet -gt $peak) {
            $peak = $workingSet
            $peakPhase = $currentPhase
        }
        $rssSamples.Add([pscustomobject]@{
            elapsedMs = [math]::Round($timer.Elapsed.TotalMilliseconds)
            workingSetMiB = [math]::Round($workingSet / 1MB, 1)
            phase = $currentPhase
        })
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
    while ($true) {
        $line = $stderrReadTask.Result
        if ($null -eq $line) { break }
        $stderrLines.Add($line)
        $stderrReadTask = $process.StandardError.ReadLineAsync()
    }
    $stderr = $stderrLines -join "`n"
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
    $propertyCaches = @([regex]::Matches(
        $combined,
        'property_cache:get_hits=(\d+) get_misses=(\d+) set_hits=(\d+) set_misses=(\d+) shape_transitions=(\d+) dictionary_objects=(\d+) invalidations=(\d+)'
    ))
    $phaseDiagnostics = @([regex]::Matches(
        $combined,
        'phase_diagnostics:phase=([^ ]+) elapsed_ms=(\d+) source_bytes=(\d+) token_count=([^ ]+) instruction_count=([^ ]+) constant_count=([^ ]+) function_count=([^ ]+) heap_estimated_bytes=(\d+) heap_live_objects=(\d+) heap_live_environments=(\d+) heap_live_functions=(\d+) gc_count=(\d+) gc_total_pause_ns=(\d+) gc_max_pause_ns=(\d+)'
    ) | ForEach-Object {
        [pscustomobject]@{
            phase = $_.Groups[1].Value
            elapsedMs = [long]$_.Groups[2].Value
            sourceBytes = [long]$_.Groups[3].Value
            tokenCount = $_.Groups[4].Value
            instructionCount = $_.Groups[5].Value
            constantCount = $_.Groups[6].Value
            functionCount = $_.Groups[7].Value
            heapEstimatedBytes = [long]$_.Groups[8].Value
            heapLiveObjects = [long]$_.Groups[9].Value
            heapLiveEnvironments = [long]$_.Groups[10].Value
            heapLiveFunctions = [long]$_.Groups[11].Value
            gcCount = [long]$_.Groups[12].Value
            gcTotalPauseNs = [long]$_.Groups[13].Value
            gcMaxPauseNs = [long]$_.Groups[14].Value
        }
    })
    [long]$loadLocalCount = 0
    [long]$storeLocalCount = 0
    [long]$loadNameCount = 0
    [long]$storeNameCount = 0
    [long]$environmentHops = 0
    [long]$propertyGetHits = 0
    [long]$propertyGetMisses = 0
    [long]$propertySetHits = 0
    [long]$propertySetMisses = 0
    [long]$shapeTransitions = 0
    [long]$dictionaryObjects = 0
    [long]$propertyCacheInvalidations = 0
    foreach ($sample in $nameResolution) {
        $loadLocalCount += [long]$sample.Groups[1].Value
        $storeLocalCount += [long]$sample.Groups[2].Value
        $loadNameCount += [long]$sample.Groups[3].Value
        $storeNameCount += [long]$sample.Groups[4].Value
        $environmentHops += [long]$sample.Groups[5].Value
    }
    foreach ($sample in $propertyCaches) {
        $propertyGetHits += [long]$sample.Groups[1].Value
        $propertyGetMisses += [long]$sample.Groups[2].Value
        $propertySetHits += [long]$sample.Groups[3].Value
        $propertySetMisses += [long]$sample.Groups[4].Value
        $shapeTransitions += [long]$sample.Groups[5].Value
        $dictionaryObjects += [long]$sample.Groups[6].Value
        $propertyCacheInvalidations += [long]$sample.Groups[7].Value
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
        peakPhase = $peakPhase
        threadStackMiB = $ThreadStackMiB
        gcThreshold = $GcThreshold
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
        propertyCacheSamples = $propertyCaches.Count
        propertyGetHits = $propertyGetHits
        propertyGetMisses = $propertyGetMisses
        propertySetHits = $propertySetHits
        propertySetMisses = $propertySetMisses
        shapeTransitions = $shapeTransitions
        dictionaryObjects = $dictionaryObjects
        propertyCacheInvalidations = $propertyCacheInvalidations
        phaseDiagnostics = $phaseDiagnostics
        rssSamples = $rssSamples
        log = (Split-Path -Leaf $logPath)
    }
    $results += $result
    $results | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $summaryPath -Encoding utf8
    Write-Output ("{0,-36} {1,-18} {2,8:N3}s {3,8:N1} MiB" -f `
        $runnerName, $status, $result.wallSeconds, $result.peakWorkingSetMiB)
}

Write-Output "summary=$summaryPath"
