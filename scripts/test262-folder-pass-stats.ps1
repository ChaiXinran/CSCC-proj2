param(
  [string]$Root = "test262\test",
  [string]$Log = "reports\.test262\test262-full-output\test262-final-output.txt",
  [ValidateSet(1, 2, 3)]
  [int]$Level = 1,
  [string]$Out = "reports\.test262\test262-folder-pass-stats.csv",

  # 默认不启用。
  # 启用后，层级不够的目录会按自身目录统计。
  # 例如三级统计时，built-ins/Temporal/keys.js 会归入 built-ins/Temporal。
  # 不启用时，它会被排除在三级统计之外。
  [switch]$IncludeShortAsSelf,

  [ValidateSet("dir", "total", "passed", "failed", "skipped", "pass_rate", "fail_rate")]
  [string]$SortBy = "passed",

  [int]$Top = 50
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Add-Count {
  param(
    [hashtable]$Map,
    [string]$Key
  )

  if ([string]::IsNullOrWhiteSpace($Key)) {
    return
  }

  if ($Map.ContainsKey($Key)) {
    $Map[$Key] = [int]$Map[$Key] + 1
  } else {
    $Map[$Key] = 1
  }
}

function Sum-MapValues {
  param(
    [hashtable]$Map
  )

  $sum = 0
  foreach ($value in $Map.Values) {
    $sum += [int]$value
  }
  return $sum
}

function Convert-LogPathToRelativeTestPath {
  param(
    [string]$PathText
  )

  $p = ($PathText -replace "\\", "/").Trim()
  $p = $p.Trim('"')

  $marker = "test262/test/"
  $idx = $p.IndexOf($marker, [StringComparison]::OrdinalIgnoreCase)

  if ($idx -ge 0) {
    return $p.Substring($idx + $marker.Length)
  }

  if ($p.StartsWith("test/", [StringComparison]::OrdinalIgnoreCase)) {
    return $p.Substring("test/".Length)
  }

  return $p
}

function Get-FolderKeyFromRelativeFilePath {
  param(
    [string]$RelativeFilePath,
    [int]$Level,
    [bool]$IncludeShort
  )

  $p = ($RelativeFilePath -replace "\\", "/").Trim("/")

  if ([string]::IsNullOrWhiteSpace($p)) {
    return $null
  }

  $parts = @($p -split "/" | Where-Object { $_ -ne "" })

  # 至少应当是：目录 / 文件.js
  if ($parts.Count -lt 2) {
    return $null
  }

  $fileName = $parts[$parts.Count - 1]

  if ($fileName -notlike "*.js") {
    return $null
  }

  if ($fileName -like "*_FIXTURE.js") {
    return $null
  }

  # 文件夹部分，不包含最后的文件名
  $dirCount = $parts.Count - 1

  # 层级不够：例如三级统计时，built-ins/Temporal/keys.js 只有 2 层目录
  if ($dirCount -lt $Level) {
    if ($IncludeShort) {
      return (@($parts[0..($dirCount - 1)]) -join "/")
    } else {
      return $null
    }
  }

  return (@($parts[0..($Level - 1)]) -join "/")
}

function Get-FailedOrSkippedRecordsFromLog {
  param(
    [string]$LogPath,
    [int]$Level,
    [bool]$IncludeShort
  )

  $records = @()

  foreach ($line in Get-Content $LogPath) {
    # 支持 progress 输出：
    # FAIL    test262\test\...
    # SKIP    test262\test\...
    #
    # 也兼容 verbose 可能出现的：
    # Failed  test262\test\...
    # Skipped test262\test\...
    if ($line -match "^(?<status>FAIL|SKIP|Failed|Skipped)\s+(?<path>.+?\.js)") {
      $statusText = $Matches["status"]
      $rawPath = $Matches["path"]

      $rel = Convert-LogPathToRelativeTestPath $rawPath
      $key = Get-FolderKeyFromRelativeFilePath `
        -RelativeFilePath $rel `
        -Level $Level `
        -IncludeShort $IncludeShort

      if ($null -eq $key) {
        $records += [pscustomobject]@{
          status = $statusText
          key = $null
          excluded = $true
        }
      } else {
        $records += [pscustomobject]@{
          status = $statusText
          key = $key
          excluded = $false
        }
      }
    }
  }

  return $records
}

if (!(Test-Path $Root)) {
  throw "Test262 root does not exist: $Root"
}

if (!(Test-Path $Log)) {
  throw "Log file does not exist: $Log"
}

$rootAbs = (Resolve-Path $Root).Path

$totalByDir = @{}
$excludedTotal = 0
$rawTotal = 0

Get-ChildItem $Root -Recurse -Filter *.js | ForEach-Object {
  if ($_.Name -like "*_FIXTURE.js") {
    return
  }

  $rawTotal += 1

  $rel = $_.FullName.Substring($rootAbs.Length + 1)
  $key = Get-FolderKeyFromRelativeFilePath `
    -RelativeFilePath $rel `
    -Level $Level `
    -IncludeShort ([bool]$IncludeShortAsSelf)

  if ($null -eq $key) {
    $excludedTotal += 1
  } else {
    Add-Count $totalByDir $key
  }
}

$failedByDir = @{}
$skippedByDir = @{}
$excludedFailed = 0
$excludedSkipped = 0
$rawFailed = 0
$rawSkipped = 0

$records = Get-FailedOrSkippedRecordsFromLog `
  -LogPath $Log `
  -Level $Level `
  -IncludeShort ([bool]$IncludeShortAsSelf)

foreach ($record in $records) {
  $isFail = $record.status -eq "FAIL" -or $record.status -eq "Failed"
  $isSkip = $record.status -eq "SKIP" -or $record.status -eq "Skipped"

  if ($isFail) {
    $rawFailed += 1
  } elseif ($isSkip) {
    $rawSkipped += 1
  }

  if ($record.excluded) {
    if ($isFail) {
      $excludedFailed += 1
    } elseif ($isSkip) {
      $excludedSkipped += 1
    }
    continue
  }

  if ($isFail) {
    Add-Count $failedByDir $record.key
  } elseif ($isSkip) {
    Add-Count $skippedByDir $record.key
  }
}

$includedTotal = Sum-MapValues $totalByDir
$includedFailed = Sum-MapValues $failedByDir
$includedSkipped = Sum-MapValues $skippedByDir
$includedPassed = $includedTotal - $includedFailed - $includedSkipped

$result = @()

foreach ($key in $totalByDir.Keys) {
  $total = [int]$totalByDir[$key]

  $failed = if ($failedByDir.ContainsKey($key)) {
    [int]$failedByDir[$key]
  } else {
    0
  }

  $skipped = if ($skippedByDir.ContainsKey($key)) {
    [int]$skippedByDir[$key]
  } else {
    0
  }

  $passed = $total - $failed - $skipped

  if ($passed -lt 0) {
    Write-Warning "Negative passed count for $key. total=$total failed=$failed skipped=$skipped. Check log/root mismatch."
  }

  $passRate = if ($total -gt 0) {
    [math]::Round($passed * 100.0 / $total, 2)
  } else {
    0
  }

  $failRate = if ($total -gt 0) {
    [math]::Round($failed * 100.0 / $total, 2)
  } else {
    0
  }

  $passedShare = if ($includedPassed -gt 0) {
    [math]::Round($passed * 100.0 / $includedPassed, 2)
  } else {
    0
  }

  $failedShare = if ($includedFailed -gt 0) {
    [math]::Round($failed * 100.0 / $includedFailed, 2)
  } else {
    0
  }

  $result += [pscustomobject]@{
    dir = $key
    total = $total
    passed = $passed
    failed = $failed
    skipped = $skipped
    pass_rate = $passRate
    fail_rate = $failRate
    passed_share = $passedShare
    failed_share = $failedShare
  }
}

if ($SortBy -eq "dir") {
  $result = $result | Sort-Object dir
} else {
  $result = $result | Sort-Object -Property $SortBy -Descending
}

$outDir = Split-Path -Parent $Out
if (![string]::IsNullOrWhiteSpace($outDir) -and !(Test-Path $outDir)) {
  New-Item -ItemType Directory -Force -Path $outDir | Out-Null
}

$result | Export-Csv $Out -NoTypeInformation -Encoding UTF8

Write-Host ""
Write-Host "=== Test262 folder pass stats ==="
Write-Host "Root: $Root"
Write-Host "Log:  $Log"
Write-Host "Level: $Level"
Write-Host "IncludeShortAsSelf: $IncludeShortAsSelf"
Write-Host "Output: $Out"
Write-Host ""
Write-Host "Raw total js files:       $rawTotal"
Write-Host "Included total files:     $includedTotal"
Write-Host "Excluded short-level total files: $excludedTotal"
Write-Host ""
Write-Host "Raw failed from log:      $rawFailed"
Write-Host "Included failed:          $includedFailed"
Write-Host "Excluded short-level failed: $excludedFailed"
Write-Host ""
Write-Host "Raw skipped from log:     $rawSkipped"
Write-Host "Included skipped:         $includedSkipped"
Write-Host "Excluded short-level skipped: $excludedSkipped"
Write-Host ""
Write-Host "Included passed:          $includedPassed"
Write-Host ""

$result |
  Select-Object -First $Top |
  Format-Table -AutoSize