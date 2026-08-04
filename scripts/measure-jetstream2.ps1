param(
    [Parameter(Mandatory = $true)]
    [string[]]$Tests,
    [int]$Iterations = 1,
    [int]$Repeats = 5,
    [int]$TimeoutSeconds = 180,
    [string]$JetStreamRoot = "benchmarks/JetStream2",
    [string]$Output = "reports/jetstream2/performance-summary.json"
)

$ErrorActionPreference = "Stop"
if ($Repeats -lt 1) { throw "Repeats must be at least 1" }
if ($Iterations -lt 1) { throw "Iterations must be at least 1" }
$binary = (Resolve-Path "target/release/agentjs.exe").Path
$runDirectory = "target/jetstream2-measure"
New-Item -ItemType Directory -Force -Path $runDirectory | Out-Null

function Get-Percentile([double[]]$Values, [double]$Percentile) {
    $sorted = @($Values | Sort-Object)
    if ($sorted.Count -eq 1) { return $sorted[0] }
    $position = ($sorted.Count - 1) * $Percentile
    $lower = [math]::Floor($position)
    $upper = [math]::Ceiling($position)
    if ($lower -eq $upper) { return $sorted[$lower] }
    return $sorted[$lower] + ($sorted[$upper] - $sorted[$lower]) * ($position - $lower)
}

$agentRevision = (& git rev-parse HEAD).Trim()
$agentDirty = [bool](& git status --porcelain)
$benchmarkResults = @()
foreach ($test in $Tests) {
    $runner = Join-Path $runDirectory "$test.js"
    node scripts/prepare-jetstream2.mjs $JetStreamRoot $test $Iterations $runner | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "failed to generate $test" }
    $manifest = Get-Content ($runner -replace '\.js$', '.manifest.json') -Raw |
        ConvertFrom-Json
    $samples = @()
    foreach ($repeat in 1..$Repeats) {
        $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
        $startInfo.FileName = $binary
        $startInfo.Arguments = 'jetstream "' + (Resolve-Path $runner).Path +
            '" --resource-root "' + (Resolve-Path $JetStreamRoot).Path + '"'
        $startInfo.WorkingDirectory = (Get-Location).Path
        $startInfo.UseShellExecute = $false
        $startInfo.RedirectStandardOutput = $true
        $startInfo.RedirectStandardError = $true
        $process = [System.Diagnostics.Process]::new()
        $process.StartInfo = $startInfo
        $timer = [System.Diagnostics.Stopwatch]::StartNew()
        [void]$process.Start()
        $stdoutTask = $process.StandardOutput.ReadToEndAsync()
        $stderrTask = $process.StandardError.ReadToEndAsync()
        $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
        [long]$peak = 0
        while (-not $process.HasExited -and [DateTime]::UtcNow -lt $deadline) {
            $process.Refresh()
            $peak = [math]::Max($peak, $process.WorkingSet64)
            Start-Sleep -Milliseconds 50
        }
        $timedOut = -not $process.HasExited
        if ($timedOut) {
            Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
            [void]$process.WaitForExit(10000)
        }
        $timer.Stop()
        $combined = $stdoutTask.Result + $stderrTask.Result
        $completed = $combined.Contains("JETSTREAM_RUN_COMPLETE")
        $status = if ($timedOut) { "TIMEOUT" } elseif ($process.ExitCode -eq 0 -and $completed) { "PASS" } else { "FAIL" }
        $samples += [pscustomobject]@{
            repeat = $repeat
            status = $status
            wallTimeMs = [math]::Round($timer.Elapsed.TotalMilliseconds, 3)
            cpuTimeMs = [math]::Round($process.TotalProcessorTime.TotalMilliseconds, 3)
            peakWorkingSetBytes = $peak
            exitCode = if ($timedOut) { $null } else { $process.ExitCode }
        }
    }
    $passing = @($samples | Where-Object status -eq "PASS")
    $walls = [double[]]@($passing | ForEach-Object wallTimeMs)
    $summary = $null
    if ($walls.Count) {
        $median = Get-Percentile $walls 0.5
        $deviations = [double[]]@($walls | ForEach-Object { [math]::Abs($_ - $median) })
        $mean = ($walls | Measure-Object -Average).Average
        $sumSquares = ($walls | ForEach-Object { ($_ - $mean) * ($_ - $mean) } | Measure-Object -Sum).Sum
        $summary = [pscustomobject]@{
            medianWallTimeMs = [math]::Round($median, 3)
            minWallTimeMs = [math]::Round(($walls | Measure-Object -Minimum).Minimum, 3)
            maxWallTimeMs = [math]::Round(($walls | Measure-Object -Maximum).Maximum, 3)
            p90WallTimeMs = [math]::Round((Get-Percentile $walls 0.9), 3)
            madWallTimeMs = [math]::Round((Get-Percentile $deviations 0.5), 3)
            standardDeviationMs = [math]::Round([math]::Sqrt($sumSquares / $walls.Count), 3)
            peakWorkingSetBytes = ($passing | Measure-Object peakWorkingSetBytes -Maximum).Maximum
        }
    }
    $benchmarkResults += [pscustomobject]@{
        benchmark = $test
        iterations = $Iterations
        repeats = $Repeats
        passes = $passing.Count
        runnerSha256 = $manifest.runnerSha256
        jetStreamRevision = $manifest.sourceCommit
        summary = $summary
        samples = $samples
    }
}

$report = [pscustomobject]@{
    schemaVersion = 1
    generatedAtUtc = [DateTime]::UtcNow.ToString("o")
    agentRevision = $agentRevision
    agentDirty = $agentDirty
    binary = $binary
    timeoutSeconds = $TimeoutSeconds
    benchmarks = $benchmarkResults
}
$parent = Split-Path -Parent $Output
if ($parent) { New-Item -ItemType Directory -Force -Path $parent | Out-Null }
$report | ConvertTo-Json -Depth 8 | Set-Content -Encoding utf8 $Output
$benchmarkResults | Select-Object benchmark, passes, repeats,
    @{Name="medianMs"; Expression={$_.summary.medianWallTimeMs}},
    @{Name="p90Ms"; Expression={$_.summary.p90WallTimeMs}} | Format-Table -AutoSize
Write-Output "summary=$Output"
