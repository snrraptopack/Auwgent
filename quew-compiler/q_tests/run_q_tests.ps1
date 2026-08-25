# Runs every .quew fixture in q_tests/ through the compiled quew CLI,
# verifies the result against the expected value, and reports timing.
#
# Usage:
#   powershell -File q_tests\run_q_tests.ps1            # release binary (fast)
#   powershell -File q_tests\run_q_tests.ps1 -Debug     # cargo debug run (slow)
#
# Add a new test: drop <name>.quew in this folder and add an entry to $expected.

param(
    [switch]$Debug
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location (Join-Path $root "..")

if ($Debug) {
    cargo build -q -p quew-cli
    $quew = ".\target\debug\quew.exe"
} else {
    cargo build --release -q -p quew-cli
    $quew = ".\target\release\quew.exe"
}

$expected = @{
    "for_loop_sum.quew"      = "10"
    "while_accumulator.quew" = "6"
    "branch_merge.quew"      = "3"
    "elseif_chain.quew"      = "6"
    "nested_if_merge.quew"   = "20"
    "stress_while_100k.quew" = "4999950000"
    "expr_call2.quew"        = "42"
    "recursion_base.quew"    = "0"
    # Network-dependent (https://example.com) — requires connectivity.
    "early_return_if.quew"   = "status 200"
    "net_fetch.quew"         = "status 200"
    "json_roundtrip.quew"    = "ada ok"
}

# Files that must FAIL to check (compile-time rejection tests).
$expectError = @{
    "dynamic_model_rejected.quew" = "dynamic model selection"
}

# Files that must fail at RUNTIME (safety-limit tests). Key = expected error substring.
$expectRunError = @{
    "infinite_loop_capped.quew" = "iteration limit"
}

$pass = 0
$fail = 0

foreach ($file in Get-ChildItem "$root\*.quew" | Sort-Object Name) {
    $name = $file.Name
    $sw = [System.Diagnostics.Stopwatch]::StartNew()

    if ($expectError.ContainsKey($name)) {
        $ErrorActionPreference = "Continue"
        & $quew check $file.FullName *> $null
        $ErrorActionPreference = "Stop"
        $sw.Stop()
        $ms = [int]$sw.Elapsed.TotalMilliseconds
        if ($LASTEXITCODE -ne 0) {
            $pass++
            Write-Output ("PASS  {0,-28} rejected as expected  ({1} ms)" -f $name, $ms)
        } else {
            $fail++
            Write-Output ("FAIL  {0,-28} expected a check error, but check passed  ({1} ms)" -f $name, $ms)
        }
        continue
    }

    if ($expectRunError.ContainsKey($name)) {
        $ErrorActionPreference = "Continue"
        $raw = & $quew run $file.FullName 2>&1
        $ErrorActionPreference = "Stop"
        $sw.Stop()
        $ms = [int]$sw.Elapsed.TotalMilliseconds
        $output = [string]::Join(" ", @($raw | ForEach-Object { $_.ToString() }))
        if ($output -match $expectRunError[$name]) {
            $pass++
            Write-Output ("PASS  {0,-28} runtime error as expected ({1})  ({2} ms)" -f $name, $expectRunError[$name], $ms)
        } else {
            $fail++
            Write-Output ("FAIL  {0,-28} expected runtime error '{1}', got: {2}" -f $name, $expectRunError[$name], $output.Trim().Substring(0, [Math]::Min(120, $output.Trim().Length)))
        }
        continue
    }

    $output = & $quew run $file.FullName 2>&1 | Out-String
    $sw.Stop()
    $ms = [int]$sw.Elapsed.TotalMilliseconds

    $actual = if ($output -match "Execution result:\s*(.+?)\s*$") { $Matches[1].Trim() } else { "ERROR" }
    $want = $expected[$name]

    if ($null -eq $want) {
        Write-Output ("SKIP  {0,-28} no expected value registered ({1} ms)" -f $name, $ms)
    } elseif ($actual.StartsWith($want)) {
        $pass++
        Write-Output ("PASS  {0,-28} = {1}  ({2} ms)" -f $name, $actual, $ms)
    } else {
        $fail++
        Write-Output ("FAIL  {0,-28} want {1}, got {2}  ({3} ms)" -f $name, $want, $actual, $ms)
    }
}

Write-Output ""
Write-Output ("{0} passed, {1} failed" -f $pass, $fail)
if ($fail -gt 0) { exit 1 }
