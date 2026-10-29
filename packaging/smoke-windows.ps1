# Functional smoke of an INSTALLED (packaged) Gauntlet on Windows.
#
# Usage: packaging/smoke-windows.ps1 -Gauntlet <path> [-Workbench <path>] -ExpectedVersion <v>
#
# Covers installed behavior only (uninstall assertions live in the CI
# workflow). It runs entirely inside a disposable temp root so it never
# touches real user data:
#
#   1. `--version` prints exactly "gauntlet <ExpectedVersion>"
#   2. `--help` exits 0 with non-empty output
#   3. `doctor` exits 0 with its machine report inside a temp
#      GAUNTLET_REPORT_DIR (disposable env-only dir; doctor does not probe
#      PostgreSQL and none is provided or simulated)
#   4. `gauntlet init` scaffolds a fresh project, and a second init refuses
#      to overwrite it
#   5. workbench leg (only when -Workbench is passed): HONEST BEST-EFFORT.
#      Spawning a TUI inside a real PTY from pwsh is genuinely hard on
#      Windows CI, so this leg asserts what Windows can assert: the
#      workbench binary STARTS (an unstartable binary fails the smoke with
#      the exact error — it must never ship silently), and — when
#      observable through redirected pipes — render markers / a clean exit.
#      Full TUI render qualification (real PTY 120x40, markers, clean 'q'
#      exit) happens on the macOS/Linux legs. Every downgrade is printed
#      and recorded; skips are never silent.
param(
    [Parameter(Mandatory = $true, Position = 0)][string]$Gauntlet,
    [Parameter(Mandatory = $true, Position = 1)][string]$ExpectedVersion,
    [Parameter()][string]$Workbench
)

$ErrorActionPreference = 'Stop'

$work = Join-Path ([System.IO.Path]::GetTempPath()) ("gauntlet-smoke-" + [guid]::NewGuid())
New-Item -ItemType Directory -Force -Path $work | Out-Null

function Fail([string]$Message) {
    Write-Host "smoke: FAIL $Message" -ForegroundColor Red
    exit 1
}
function Pass([string]$Message) {
    Write-Host "smoke: PASS $Message" -ForegroundColor Green
}
function Note([string]$Message) {
    Write-Host "smoke: NOTE $Message"
}
function Warn([string]$Message) {
    Write-Host "smoke: WARNING $Message"
}

if (-not (Test-Path $Gauntlet)) { Fail "not found: $Gauntlet" }
$GauntletBin = (Resolve-Path $Gauntlet).Path
if ($Workbench) {
    if (-not (Test-Path $Workbench)) { Fail "not found: $Workbench" }
    $WorkbenchBin = (Resolve-Path $Workbench).Path
}

Write-Host "smoke: gauntlet binary $GauntletBin"
if ($WorkbenchBin) { Write-Host "smoke: workbench binary $WorkbenchBin" }
else { Note "no -Workbench passed; the workbench start leg is not part of this run — recorded" }
Write-Host "smoke: disposable root $work"

try {
    # Disposable env-only configuration. The CLI under test may only write
    # inside this tree: GAUNTLET_REPORT_DIR is temp-rooted and every CLI
    # invocation runs with the temp root (or a subdir) as its cwd.
    $env:GAUNTLET_REPORT_DIR = Join-Path $work 'report'
    Remove-Item Env:GAUNTLET_PROJECT_ROOT -ErrorAction SilentlyContinue
    Remove-Item Env:GAUNTLET_CONFIG_DIR -ErrorAction SilentlyContinue
    Remove-Item Env:GAUNTLET_DATABASE_URL -ErrorAction SilentlyContinue
    Remove-Item Env:GAUNTLET_ENDPOINT -ErrorAction SilentlyContinue
    Remove-Item Env:GAUNTLET_MOCK_SCENARIO -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force -Path $env:GAUNTLET_REPORT_DIR | Out-Null

    # 1. --version exact match
    & $GauntletBin --version > (Join-Path $work 'version.out') 2> (Join-Path $work 'version.err')
    if ($LASTEXITCODE -ne 0) {
        Fail "--version exited $LASTEXITCODE: $(Get-Content (Join-Path $work 'version.err') -Raw)"
    }
    $got = ('' + (Get-Content (Join-Path $work 'version.out') -Raw)).Trim()
    if ($got -ne "gauntlet $ExpectedVersion") {
        Fail "version: got '$got', want 'gauntlet $ExpectedVersion'"
    }
    Pass "version -> $got"

    # 2. --help
    & $GauntletBin --help > (Join-Path $work 'help.out') 2> (Join-Path $work 'help.err')
    if ($LASTEXITCODE -ne 0) {
        Fail "--help exited $LASTEXITCODE: $(Get-Content (Join-Path $work 'help.err') -Raw)"
    }
    if ((Get-Item (Join-Path $work 'help.out')).Length -eq 0) { Fail "--help produced no output" }
    Pass "--help exits 0 with usage output"

    # 3. doctor in a disposable env-only dir
    Push-Location $work
    try {
        & $GauntletBin doctor > (Join-Path $work 'doctor.out') 2> (Join-Path $work 'doctor.err')
        if ($LASTEXITCODE -ne 0) {
            Fail "doctor exited $LASTEXITCODE in a disposable env-only dir: $(Get-Content (Join-Path $work 'doctor.err') -Raw)$(Get-Content (Join-Path $work 'doctor.out') -Raw)"
        }
    }
    finally { Pop-Location }
    if (-not (Test-Path (Join-Path $env:GAUNTLET_REPORT_DIR 'doctor.json'))) {
        Fail "doctor did not write its machine report under GAUNTLET_REPORT_DIR ($env:GAUNTLET_REPORT_DIR)"
    }
    Pass "doctor exits 0; report written inside the disposable GAUNTLET_REPORT_DIR"

    # 4. init scaffolds, and refuses to overwrite
    $project = Join-Path $work 'project'
    New-Item -ItemType Directory -Force -Path $project | Out-Null
    Push-Location $project
    try {
        & $GauntletBin init > (Join-Path $work 'init.out') 2> (Join-Path $work 'init.err')
        if ($LASTEXITCODE -ne 0) {
            Fail "init exited $LASTEXITCODE in a fresh temp project: $(Get-Content (Join-Path $work 'init.err') -Raw)$(Get-Content (Join-Path $work 'init.out') -Raw)"
        }
        foreach ($required in @('GAUNTLET.md', '.env.example', '.gauntlet')) {
            if (-not (Test-Path (Join-Path $project $required))) {
                Fail "init did not create $required"
            }
        }
        & $GauntletBin init > (Join-Path $work 'init2.out') 2> (Join-Path $work 'init2.err')
        if ($LASTEXITCODE -eq 0) {
            Fail "a second init overwrote an existing project; it must refuse"
        }
    }
    finally { Pop-Location }
    Pass "init scaffolds a project and refuses to overwrite an existing one"

    # 5. workbench start leg (only when -Workbench is passed)
    if (-not $WorkbenchBin) {
        Note "workbench start leg skipped: not requested (no -Workbench) — recorded"
    }
    else {
        $wbCwd = Join-Path $work 'workbench-cwd'
        $projection = Join-Path $work 'projection'
        New-Item -ItemType Directory -Force -Path $wbCwd, $projection | Out-Null

        $proc = [System.Diagnostics.Process]::new()
        $started = $false
        try {
            $psi = $proc.StartInfo
            $psi.FileName = $WorkbenchBin
            $psi.WorkingDirectory = $wbCwd
            $psi.UseShellExecute = $false
            $psi.CreateNoWindow = $true
            $psi.RedirectStandardInput = $true
            $psi.RedirectStandardOutput = $true
            $psi.RedirectStandardError = $true
            $psi.EnvironmentVariables['GAUNTLET_WORKBENCH_GAUNTLET_BINARY'] = $GauntletBin
            $psi.EnvironmentVariables['GAUNTLET_WORKBENCH_PROJECTION_STORE'] = $projection

            try {
                $null = $proc.Start()
                $started = $true
            }
            catch {
                Fail "workbench binary cannot start on Windows: $($_.Exception.GetType().Name): $($_.Exception.Message)"
            }
            $outTask = $proc.StandardOutput.ReadToEndAsync()
            $errTask = $proc.StandardError.ReadToEndAsync()

            $startDeadline = (Get-Date).AddSeconds(15)
            while (-not $proc.HasExited -and (Get-Date) -lt $startDeadline) {
                Start-Sleep -Milliseconds 200
            }

            if ($proc.HasExited) {
                $code = $proc.ExitCode
                $stdout = $outTask.GetAwaiter().GetResult()
                $stderr = $errTask.GetAwaiter().GetResult()
                if ($code -ne 0) {
                    Fail ("workbench binary exited $code immediately on Windows; an unstartable " +
                        "binary must not be shipped. stdout: $stdout; stderr: $stderr")
                }
                $missing = @('NAVIGATOR', 'PRIMARY', 'INSPECTOR', 'COMPOSER') |
                    Where-Object { $stdout -notmatch [regex]::Escape($_) }
                if (-not $missing) {
                    Pass "workbench rendered all TUI markers through the redirected pipe and exited cleanly (exit 0)"
                }
                else {
                    Warn ("workbench started and exited cleanly (exit 0) but stdout carried no TUI render " +
                        "markers (missing: $($missing -join ', ')); a redirected pipe is not a PTY, so " +
                        "render qualification happens on the macOS/Linux legs — recorded")
                    Pass "workbench starts and exits cleanly on Windows (started-only downgrade recorded)"
                }
            }
            else {
                # Started and stayed up: a real launch. Try a graceful 'q'
                # through redirected stdin; a non-TTY stdin may not drive the
                # TUI's raw-mode reader, which is recorded honestly.
                try {
                    $proc.StandardInput.Write("q`r`n")
                    $proc.StandardInput.Flush()
                }
                catch { }
                $exitDeadline = (Get-Date).AddSeconds(5)
                while (-not $proc.HasExited -and (Get-Date) -lt $exitDeadline) {
                    Start-Sleep -Milliseconds 200
                }
                if ($proc.HasExited -and $proc.ExitCode -eq 0) {
                    Pass "workbench started, ran, and exited cleanly on 'q' (stdin pipe)"
                }
                else {
                    if (-not $proc.HasExited) {
                        $null = $proc.Kill()
                        $null = $proc.WaitForExit(5000)
                    }
                    Warn ("workbench started and stayed up but did not exit on 'q' via redirected " +
                        "(non-TTY) stdin; it was stopped by the smoke. Full TUI qualification " +
                        "(real PTY 120x40, markers, clean 'q' exit) happens on the macOS/Linux " +
                        "legs — recorded")
                    Pass "workbench starts on Windows (started-only downgrade recorded)"
                }
            }
        }
        finally {
            if ($started -and -not $proc.HasExited) {
                try { $null = $proc.Kill() } catch { }
            }
            $proc.Dispose()
        }
    }

    Write-Host 'smoke: ALL CHECKS PASSED'
}
finally {
    Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
}
