<#
.SYNOPSIS
  Compares execution time of a 100k-iteration while-loop sum across
  Quew, Python, and Node.js.

.DESCRIPTION
  Runs the same logical benchmark (sum 0..99999 via a while loop) in:
    - Quew (your .quew stress test, via the compiled quew.exe)
    - Python (python -c "...")
    - Node.js (node -e "...")

  Each is run N times (default 5) and min/mean/max are reported.

.USAGE
  # From quew-compiler directory, or pass -QuewExe / -QuewFile explicitly:
  .\bench-compare.ps1

  # Custom paths / iteration count / repeat count:
  .\bench-compare.ps1 -QuewExe .\target\release\quew.exe `
                       -QuewFile .\q_tests\stress_while_100k.quew `
                       -Iterations 100000 `
                       -Runs 5
#>

param(
    [string]$QuewExe = ".\target\release\quew.exe",
    [string]$QuewFile = ".\q_tests\stress_while_100k.quew",
    [int]$Iterations = 100000,
    [int]$Runs = 5
)

function Measure-Runs {
    param(
        [string]$Label,
        [scriptblock]$Block,
        [int]$Runs
    )

    $times = @()
    for ($i = 0; $i -lt $Runs; $i++) {
        $elapsed = (Measure-Command { & $Block | Out-Null }).TotalMilliseconds
        $times += $elapsed
    }

    [PSCustomObject]@{
        Label   = $Label
        MinMs   = [math]::Round(($times | Measure-Object -Minimum).Minimum, 3)
        MeanMs  = [math]::Round(($times | Measure-Object -Average).Average, 3)
        MaxMs   = [math]::Round(($times | Measure-Object -Maximum).Maximum, 3)
        Runs    = $Runs
    }
}

Write-Host "Benchmarking with $Runs run(s) each, N = $Iterations`n" -ForegroundColor Cyan

$results = @()

# --- Quew ---
if (Test-Path $QuewExe) {
    if (Test-Path $QuewFile) {
        $results += Measure-Runs -Label "Quew ($QuewFile)" -Runs $Runs -Block {
            & $QuewExe run $QuewFile
        }
    } else {
        Write-Warning "Quew file not found: $QuewFile (skipping Quew benchmark)"
    }
} else {
    Write-Warning "Quew executable not found: $QuewExe (skipping Quew benchmark)"
}

# --- Python ---
$pythonCmd = Get-Command python -ErrorAction SilentlyContinue
if (-not $pythonCmd) { $pythonCmd = Get-Command python3 -ErrorAction SilentlyContinue }

if ($pythonCmd) {
    $pyCode = @"
total = 0
i = 0
while i < ${Iterations}:
    total += i
    i += 1
print(total)
"@
    $results += Measure-Runs -Label "Python ($($pythonCmd.Name))" -Runs $Runs -Block {
        & $pythonCmd.Source -c $pyCode
    }
} else {
    Write-Warning "Python not found on PATH (skipping Python benchmark)"
}

# --- Node ---
$nodeCmd = Get-Command node -ErrorAction SilentlyContinue

if ($nodeCmd) {
    $jsCode = "let total=0,i=0;while(i<$Iterations){total+=i;i++};console.log(total);"
    $results += Measure-Runs -Label "Node ($((& node --version)))" -Runs $Runs -Block {
        & $nodeCmd.Source -e $jsCode
    }
} else {
    Write-Warning "Node not found on PATH (skipping Node benchmark)"
}

Write-Host "`nResults (sorted fastest to slowest by mean):`n" -ForegroundColor Cyan
$results | Sort-Object MeanMs | Format-Table Label, MinMs, MeanMs, MaxMs, Runs -AutoSize

# Show relative slowdown vs the fastest result
if ($results.Count -gt 1) {
    $fastest = ($results | Sort-Object MeanMs)[0]
    Write-Host "`nRelative to fastest ($($fastest.Label) @ $($fastest.MeanMs) ms mean):`n" -ForegroundColor Cyan
    $results | Sort-Object MeanMs | ForEach-Object {
        $factor = if ($fastest.MeanMs -gt 0) { [math]::Round($_.MeanMs / $fastest.MeanMs, 2) } else { "n/a" }
        Write-Host ("  {0,-30} {1,10}x" -f $_.Label, $factor)
    }
}
