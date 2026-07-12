param(
    [Parameter(Mandatory = $true)]
    [string]$LegacyRoot,
    [Parameter(Mandatory = $true)]
    [string]$ResultsPath
)

$ErrorActionPreference = 'Stop'
$currentRoot = Split-Path -Parent $PSScriptRoot
$corpus = Join-Path $env:RUNNER_TEMP 'win-cleaner-scan-corpus'
New-Item -ItemType Directory -Force $corpus | Out-Null

function Add-SizedFile([string]$Path, [int]$Size) {
    New-Item -ItemType Directory -Force (Split-Path -Parent $Path) | Out-Null
    [System.IO.File]::WriteAllBytes($Path, [byte[]]::new($Size))
}

$shallow = Join-Path $corpus 'shallow'
foreach ($directory in 0..249) {
    foreach ($file in 0..19) {
        Add-SizedFile (Join-Path $shallow "d$directory\f$file.bin") 32
    }
}

$deep = Join-Path $corpus 'deep'
$cursor = $deep
foreach ($level in 0..34) {
    $cursor = Join-Path $cursor "d$level"
    foreach ($file in 0..19) {
        Add-SizedFile (Join-Path $cursor "f$file.bin") 64
    }
}

$many = Join-Path $corpus 'many-files'
New-Item -ItemType Directory -Force $many | Out-Null
foreach ($file in 0..19999) {
    Add-SizedFile (Join-Path $many "f$file.bin") 8
}

$goHarness = @'
package cleaner

import (
    "fmt"
    "os"
    "path/filepath"
    "testing"
    "time"
)

func TestScannerBenchmark(t *testing.T) {
    root := os.Getenv("BENCH_ROOT")
    scenario := os.Getenv("BENCH_SCENARIO")
    target := filepath.Join(root, scenario)
    registry := Registry{Items: []Item{{App: "Benchmark", Label: scenario, Paths: []string{target}, DefaultOn: true}}}
    if _, err := BuildPlanWithProgress(registry, nil); err != nil {
        t.Fatal(err)
    }
    for sample := 1; sample <= 7; sample++ {
        started := time.Now()
        plan, err := BuildPlanWithProgress(registry, nil)
        if err != nil {
            t.Fatal(err)
        }
        elapsed := time.Since(started).Seconds() * 1000
        fmt.Printf("go,%s,%d,%.3f,%d,%d\n", scenario, sample, elapsed, len(plan.Groups[0].Paths), plan.Groups[0].Bytes)
    }
}
'@
Set-Content -Encoding utf8 (Join-Path $LegacyRoot 'internal\cleaner\scanner_benchmark_test.go') $goHarness

$rustHarness = @'
use std::path::PathBuf;
use std::time::Instant;

use cleaner_core::{Item, Registry, Roots, build_plan};

fn main() {
    let root = PathBuf::from(std::env::var_os("BENCH_ROOT").expect("BENCH_ROOT"));
    let scenario = std::env::var("BENCH_SCENARIO").expect("BENCH_SCENARIO");
    let target = root.join(&scenario);
    let roots = Roots {
        local_app_data: Some(root),
        ..Roots::default()
    };
    let registry = Registry {
        items: vec![Item::new("Benchmark", &scenario, true).paths([target])],
    };
    let _ = build_plan(&registry, &roots, |_| {});
    for sample in 1..=7 {
        let started = Instant::now();
        let plan = build_plan(&registry, &roots, |_| {});
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        println!(
            "rust,{scenario},{sample},{elapsed:.3},{},{}",
            plan.groups[0].paths.len(), plan.groups[0].bytes
        );
    }
}
'@
$rustBin = Join-Path $currentRoot 'crates\cleaner-core\src\bin'
New-Item -ItemType Directory -Force $rustBin | Out-Null
Set-Content -Encoding utf8 (Join-Path $rustBin 'scanner-benchmark.rs') $rustHarness

Push-Location $LegacyRoot
go test -c -o legacy-scanner.exe ./internal/cleaner
Pop-Location
Push-Location $currentRoot
cargo build --release -p cleaner-core --bin scanner-benchmark
Pop-Location

"implementation,scenario,sample,milliseconds,paths,bytes" | Set-Content -Encoding ascii $ResultsPath
$env:BENCH_ROOT = $corpus
$env:LOCALAPPDATA = $corpus
foreach ($scenario in @('shallow', 'deep', 'many-files')) {
    $env:BENCH_SCENARIO = $scenario
    & (Join-Path $LegacyRoot 'legacy-scanner.exe') -test.run TestScannerBenchmark -test.v |
        Select-String '^go,' | ForEach-Object { $_.Line } |
        Tee-Object -Append -FilePath $ResultsPath
    & (Join-Path $currentRoot 'target\release\scanner-benchmark.exe') |
        Select-String '^rust,' | ForEach-Object { $_.Line } |
        Tee-Object -Append -FilePath $ResultsPath
}

$rows = Import-Csv $ResultsPath
foreach ($scenario in @('shallow', 'deep', 'many-files')) {
    $go = @($rows | Where-Object { $_.implementation -eq 'go' -and $_.scenario -eq $scenario })
    $rust = @($rows | Where-Object { $_.implementation -eq 'rust' -and $_.scenario -eq $scenario })
    if ($go.Count -ne 7 -or $rust.Count -ne 7) {
        throw "Missing samples for $scenario"
    }
    if (($go.paths | Select-Object -Unique) -ne ($rust.paths | Select-Object -Unique) -or
        ($go.bytes | Select-Object -Unique) -ne ($rust.bytes | Select-Object -Unique)) {
        throw "Scanner results differ for $scenario"
    }
    $goTimes = @($go.milliseconds | ForEach-Object { [double]$_ } | Sort-Object)
    $rustTimes = @($rust.milliseconds | ForEach-Object { [double]$_ } | Sort-Object)
    $goMedian = $goTimes[3]
    $rustMedian = $rustTimes[3]
    $relative = if ($goMedian -eq 0) { 0 } else { (($rustMedian - $goMedian) / $goMedian) * 100 }
    Write-Output ("SUMMARY {0}: go={1:N3}ms rust={2:N3}ms rust_delta={3:N1}% paths={4} bytes={5}" -f
        $scenario, $goMedian, $rustMedian, $relative, $go[0].paths, $go[0].bytes)
}
