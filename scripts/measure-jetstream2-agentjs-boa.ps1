param(
    [Parameter(Mandatory = $true)]
    [string[]]$Tests,
    [int]$Iterations = 1,
    [int]$Warmup = 2,
    [int]$Repeats = 7,
    [int]$TimeoutSeconds = 180,
    [int]$MaxRssMiB = 1536,
    [string]$JetStreamRoot = "benchmarks/JetStream2",
    [string]$Output = "presentation/assets/video/jetstream2-agentjs-boa.json"
)

$ErrorActionPreference = "Stop"
if ($Tests.Count -eq 0) { throw "At least one workload is required" }
if ($Iterations -lt 1) { throw "Iterations must be at least 1" }
if ($Warmup -lt 0) { throw "Warmup cannot be negative" }
if ($Repeats -lt 1) { throw "Repeats must be at least 1" }

$root = (Get-Location).Path
$gitRoot = $root.Replace('\', '/')
$jetStreamRootAbsolute = (Resolve-Path $JetStreamRoot).Path
$gitJetStreamRoot = $jetStreamRootAbsolute.Replace('\', '/')
$agentBinary = (Resolve-Path "target/release/agentjs.exe").Path
$boaBinary = (Resolve-Path "boa/target/release/boa.exe").Path
$runDirectory = Join-Path $root "target/jetstream2-agentjs-boa"
$runnerDirectory = Join-Path $runDirectory "runners"
New-Item -ItemType Directory -Force -Path $runnerDirectory | Out-Null
$gitExcludeFile = Join-Path $runDirectory "empty-git-excludes"
if (-not (Test-Path -LiteralPath $gitExcludeFile)) {
    [IO.File]::WriteAllText($gitExcludeFile, "", [Text.UTF8Encoding]::new($false))
}
$gitExcludeFile = $gitExcludeFile.Replace('\', '/')

function Get-Percentile([double[]]$Values, [double]$Percentile) {
    $sorted = @($Values | Sort-Object)
    if ($sorted.Count -eq 1) { return $sorted[0] }
    $position = ($sorted.Count - 1) * $Percentile
    $lower = [math]::Floor($position)
    $upper = [math]::Ceiling($position)
    if ($lower -eq $upper) { return $sorted[$lower] }
    return $sorted[$lower] + ($sorted[$upper] - $sorted[$lower]) * ($position - $lower)
}

function Get-Sha256([string]$Path) {
    return (Get-FileHash -Algorithm SHA256 -LiteralPath $Path).Hash.ToLowerInvariant()
}

function Get-GitValue([string[]]$Arguments) {
    try {
        $lines = @(& git -c "safe.directory=$gitRoot" -c "safe.directory=$gitJetStreamRoot" -c "core.excludesFile=$gitExcludeFile" @Arguments 2>$null)
        if ($LASTEXITCODE -eq 0 -and $lines.Count) {
            return (($lines -join "`n").Trim())
        }
    } catch { }
    return $null
}

function Quote-ProcessArgument([string]$Value) {
    return '"' + $Value.Replace('"', '\\"') + '"'
}

function Invoke-EngineSample(
    [string]$Engine,
    [string]$Runner,
    [string]$Test,
    [int]$Timeout,
    [int]$RssLimitMiB
) {
    $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = if ($Engine -eq "agentjs") { $agentBinary } else { $boaBinary }
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    $startInfo.Arguments = if ($Engine -eq "agentjs") {
        "run $(Quote-ProcessArgument $Runner)"
    } else {
        Quote-ProcessArgument $Runner
    }

    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $startInfo
    $timer = [System.Diagnostics.Stopwatch]::StartNew()
    [void]$process.Start()
    $stdoutTask = $process.StandardOutput.ReadToEndAsync()
    $stderrTask = $process.StandardError.ReadToEndAsync()
    $deadline = [DateTime]::UtcNow.AddSeconds($Timeout)
    [long]$peak = 0
    $memoryLimit = $false
    while (-not $process.HasExited) {
        $process.Refresh()
        $peak = [math]::Max($peak, $process.WorkingSet64)
        if ($peak -gt ([long]$RssLimitMiB * 1MB)) {
            $memoryLimit = $true
            Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
            break
        }
        if ([DateTime]::UtcNow -ge $deadline) { break }
        Start-Sleep -Milliseconds 50
    }
    $timedOut = -not $process.HasExited
    if ($timedOut) {
        Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
        [void]$process.WaitForExit(10000)
    }
    $timer.Stop()
    $stdout = $stdoutTask.GetAwaiter().GetResult()
    $stderr = $stderrTask.GetAwaiter().GetResult()
    $combined = "$stdout`n$stderr"
    $avgMatch = [regex]::Match($combined, [regex]::Escape("$Test avg:") + "\s+([0-9]+(?:\.[0-9]+)?)ms")
    $exitCode = if ($timedOut -or $memoryLimit) { $null } else { $process.ExitCode }
    $status = if ($memoryLimit) { "MEMORY_LIMIT" } elseif ($timedOut) { "TIMEOUT" } elseif ($exitCode -eq 0 -and $avgMatch.Success) { "PASS" } else { "FAIL" }
    $cpuMs = $null
    if (-not $timedOut -and -not $memoryLimit) {
        try { $cpuMs = [math]::Round($process.TotalProcessorTime.TotalMilliseconds, 3) } catch { }
    }
    $process.Dispose()
    return [pscustomobject]@{
        status = $status
        wallTimeMs = [math]::Round($timer.Elapsed.TotalMilliseconds, 3)
        workloadTimeMs = if ($avgMatch.Success) { [double]$avgMatch.Groups[1].Value } else { $null }
        cpuTimeMs = $cpuMs
        peakWorkingSetBytes = $peak
        exitCode = $exitCode
        outputTail = if ($combined.Length -gt 1000) { $combined.Substring($combined.Length - 1000) } else { $combined }
    }
}

function Get-Summary($Samples) {
    $passing = @($Samples | Where-Object status -eq "PASS")
    if ($passing.Count -eq 0) { return $null }
    $walls = [double[]]@($passing | ForEach-Object wallTimeMs)
    $workloads = [double[]]@($passing | ForEach-Object workloadTimeMs)
    $median = Get-Percentile $walls 0.5
    $deviations = [double[]]@($walls | ForEach-Object { [math]::Abs($_ - $median) })
    $mean = ($walls | Measure-Object -Average).Average
    $sumSquares = ($walls | ForEach-Object { ($_ - $mean) * ($_ - $mean) } | Measure-Object -Sum).Sum
    return [pscustomobject]@{
        medianWorkloadTimeMs = [math]::Round((Get-Percentile $workloads 0.5), 3)
        medianWallTimeMs = [math]::Round($median, 3)
        minWallTimeMs = [math]::Round(($walls | Measure-Object -Minimum).Minimum, 3)
        maxWallTimeMs = [math]::Round(($walls | Measure-Object -Maximum).Maximum, 3)
        p90WallTimeMs = [math]::Round((Get-Percentile $walls 0.9), 3)
        madWallTimeMs = [math]::Round((Get-Percentile $deviations 0.5), 3)
        standardDeviationMs = [math]::Round([math]::Sqrt($sumSquares / $walls.Count), 3)
        peakWorkingSetBytes = ($passing | Measure-Object peakWorkingSetBytes -Maximum).Maximum
    }
}

$engineResults = [ordered]@{ agentjs = [ordered]@{}; boa = [ordered]@{} }
$runnerMetadata = [ordered]@{}
foreach ($test in $Tests) {
    $runner = Join-Path $runnerDirectory "$test.js"
    $generatorOutput = @(node scripts/prepare-simple-benchmark.mjs $JetStreamRoot $test $Iterations $runner)
    if ($LASTEXITCODE -ne 0) { throw "failed to generate $test" }
    $runnerMetadata[$test] = [ordered]@{
        sha256 = Get-Sha256 $runner
        generator = "scripts/prepare-simple-benchmark.mjs"
        iterations = $Iterations
        discovery = (($generatorOutput -join "`n") | ConvertFrom-Json)
    }
    foreach ($engine in @("agentjs", "boa")) {
        Write-Host "[$engine] $test"
        for ($warm = 0; $warm -lt $Warmup; $warm++) {
            $warmSample = Invoke-EngineSample $engine $runner $test $TimeoutSeconds $MaxRssMiB
            if ($warmSample.status -ne "PASS") { throw "warmup failed: $engine/$test ($($warmSample.status))" }
        }
        $samples = @()
        for ($repeat = 1; $repeat -le $Repeats; $repeat++) {
            $sample = Invoke-EngineSample $engine $runner $test $TimeoutSeconds $MaxRssMiB
            $sample | Add-Member -NotePropertyName repeat -NotePropertyValue $repeat
            $samples += $sample
            Write-Host ("  {0}/{1}: {2} {3} ms" -f $repeat, $Repeats, $sample.status, $sample.wallTimeMs)
        }
        $passing = @($samples | Where-Object status -eq "PASS")
        $engineResults[$engine][$test] = [ordered]@{
            passes = $passing.Count
            repeats = $Repeats
            summary = Get-Summary $samples
            samples = $samples
        }
    }
}

$ratios = [ordered]@{}
$ratioValues = @()
foreach ($test in $Tests) {
    $a = $engineResults.agentjs[$test].summary.medianWorkloadTimeMs
    $b = $engineResults.boa[$test].summary.medianWorkloadTimeMs
    if ($null -ne $a -and $null -ne $b -and $a -gt 0 -and $b -gt 0) {
        $ratio = $b / $a
        $ratios[$test] = [math]::Round($ratio, 6)
        $ratioValues += $ratio
    } else { $ratios[$test] = $null }
}
$geomean = if ($ratioValues.Count) {
    [math]::Exp((($ratioValues | ForEach-Object { [math]::Log($_) } | Measure-Object -Sum).Sum) / $ratioValues.Count)
} else { $null }

$projectRevision = Get-GitValue @("rev-parse", "HEAD")
$projectStatus = @(& git -c "safe.directory=$gitRoot" -c "safe.directory=$gitJetStreamRoot" -c "core.excludesFile=$gitExcludeFile" status --porcelain 2>$null)
$projectDirty = $projectStatus.Count -gt 0
$jetstreamRevision = Get-GitValue @("-C", $JetStreamRoot, "rev-parse", "HEAD")
$report = [ordered]@{
    schemaVersion = 1
    generatedAtUtc = [DateTime]::UtcNow.ToString("o")
    protocol = [ordered]@{
        benchmark = "JetStream2 portable workload kernels"
        runner = "self-contained runner from prepare-simple-benchmark.mjs"
        iterationsPerProcess = $Iterations
        warmupProcesses = $Warmup
        measuredProcesses = $Repeats
        timeoutSeconds = $TimeoutSeconds
        maxRssMiB = $MaxRssMiB
        engines = @("agentjs", "boa")
        timing = "workload marker average; process wall time and WorkingSet64 sampled every 50 ms"
    }
    projectRevision = $projectRevision
    projectDirty = $projectDirty
    jetstreamRevision = $jetstreamRevision
    binaries = [ordered]@{
        agentjs = [ordered]@{ path = "target/release/agentjs.exe"; sizeBytes = (Get-Item $agentBinary).Length; sha256 = Get-Sha256 $agentBinary }
        boa = [ordered]@{ path = "boa/target/release/boa.exe"; sizeBytes = (Get-Item $boaBinary).Length; sha256 = Get-Sha256 $boaBinary }
    }
    runners = $runnerMetadata
    engines = $engineResults
    boaOverAgentjsWorkloadP50 = $ratios
    boaOverAgentjsWorkloadP50Geomean = if ($null -eq $geomean) { $null } else { [math]::Round($geomean, 6) }
}
$parent = Split-Path -Parent $Output
if ($parent) { New-Item -ItemType Directory -Force -Path $parent | Out-Null }
$outputPath = if ([IO.Path]::IsPathRooted($Output)) { $Output } else { Join-Path $root $Output }
$jsonText = ($report | ConvertTo-Json -Depth 20) + [Environment]::NewLine
[IO.File]::WriteAllText($outputPath, $jsonText, [Text.UTF8Encoding]::new($false))
Write-Output "summary=$Output"
