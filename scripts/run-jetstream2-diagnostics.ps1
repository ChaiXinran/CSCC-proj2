param(
    [string[]]$Tests = @("threejs", "WSL"),
    [int[]]$Iterations = @(1, 2, 5, 10),
    [int]$TimeoutSeconds = 180,
    [int]$MaxWorkingSetMB = 1024,
    [string]$JetStreamRoot = "benchmarks/JetStream2"
)

$ErrorActionPreference = "Stop"
$binary = (Resolve-Path "target/release/agentjs.exe").Path
$outputDirectory = "target/jetstream2-diagnostics"
New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null
$results = @()
$summary = Join-Path $outputDirectory "summary.json"

foreach ($test in $Tests) {
    foreach ($iteration in $Iterations) {
        $runner = Join-Path $outputDirectory "$test-i$iteration.js"
        node scripts/prepare-jetstream2.mjs `
            $JetStreamRoot $test $iteration $runner --phase-markers | Out-Null
        if ($LASTEXITCODE -ne 0) {
            throw "failed to generate $test with $iteration iteration(s)"
        }

        $processInfo = [System.Diagnostics.ProcessStartInfo]::new()
        $processInfo.FileName = $binary
        $processInfo.Arguments = 'jetstream "' + (Resolve-Path $runner).Path +
            '" --resource-root "' + (Resolve-Path $JetStreamRoot).Path +
            '" --diagnostics'
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
        [long]$peakWorkingSetBytes = 0
        $memoryLimited = $false
        while (-not $process.HasExited -and [DateTime]::UtcNow -lt $deadline) {
            $process.Refresh()
            if ($process.WorkingSet64 -gt $peakWorkingSetBytes) {
                $peakWorkingSetBytes = $process.WorkingSet64
            }
            if ($MaxWorkingSetMB -gt 0 -and
                $process.WorkingSet64 -gt ($MaxWorkingSetMB * 1MB)) {
                $memoryLimited = $true
                break
            }
            Start-Sleep -Milliseconds 100
        }
        $timedOut = -not $memoryLimited -and -not $process.HasExited
        if ($timedOut -or $memoryLimited) {
            Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
            [void]$process.WaitForExit(10000)
        }
        if ($process.HasExited) { $process.WaitForExit() }
        $timer.Stop()

        $combined = "$($stdoutTask.Result)`n$($stderrTask.Result)"
        $phaseMatches = [regex]::Matches(
            $combined,
            "JETSTREAM_PHASE:(\d+):([^\r\n]+)"
        )
        $phases = @($phaseMatches | ForEach-Object { $_.Groups[2].Value })
        $outputTimeMatches = [regex]::Matches($combined, "JETSTREAM_OUTPUT_AT:(\d+)")
        $lastOutputTimestamp = if ($outputTimeMatches.Count) {
            $lastOutputMatch = $outputTimeMatches.Item($outputTimeMatches.Count - 1)
            [DateTimeOffset]::FromUnixTimeMilliseconds(
                [long]$lastOutputMatch.Groups[1].Value
            ).UtcDateTime
        } else { $null }
        $status = if ($memoryLimited) {
            "MEMORY_LIMIT"
        } elseif ($timedOut) {
            "TIMEOUT"
        } elseif ($process.ExitCode -eq 0) {
            "PASS"
        } else {
            "FAIL"
        }
        $flatOutput = ($combined -replace "[\r\n]+", " ").Trim()
        $outputTail = if ($flatOutput.Length -gt 500) {
            $flatOutput.Substring($flatOutput.Length - 500)
        } else {
            $flatOutput
        }
        $results += [pscustomobject]@{
            benchmark = $test
            iterations = $iteration
            status = $status
            exitCode = if ($timedOut -or $memoryLimited) { $null } else { $process.ExitCode }
            wallTimeMs = [math]::Round($timer.Elapsed.TotalMilliseconds)
            cpuTimeMs = [math]::Round($process.TotalProcessorTime.TotalMilliseconds)
            peakWorkingSetBytes = $peakWorkingSetBytes
            workloadStarted = @($phases | Where-Object { $_ -eq 'init:start' }).Count -gt 0
            initializationCompleted = @($phases | Where-Object { $_ -eq 'init:end' }).Count -gt 0
            enteredIteration = @($phases | Where-Object { $_ -match '^iteration:\d+:start$' }).Count -gt 0
            completedIterations = @($phases | Where-Object { $_ -match '^iteration:\d+:end$' }).Count
            validationStarted = @($phases | Where-Object { $_ -eq 'validate:start' }).Count -gt 0
            lastPhase = if ($phases.Count) { $phases[-1] } else { $null }
            lastOutputUtc = if ($lastOutputTimestamp) { $lastOutputTimestamp.ToString('o') } else { $null }
            millisecondsSinceLastOutput = if ($lastOutputTimestamp) {
                [math]::Round(([DateTime]::UtcNow - $lastOutputTimestamp).TotalMilliseconds)
            } else { $null }
            outputTail = $outputTail
        }
        $results | ConvertTo-Json -Depth 4 | Set-Content -Encoding utf8 $summary
    }
}

$results | Format-Table -AutoSize
Write-Output "summary=$summary"
