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
#   3. `doctor` in an env-only dir (no GAUNTLET.md anywhere above) FAILS
#      CLOSED, by design: the §8.5 reference check fails with no project /
#      reference manifest (CI run 40949652982 + adversarial F1 proved the
#      previous exit-0 assertion could never pass; the product is correct
#      and is never weakened to pass a smoke). The contract asserted here:
#      a typed nonzero exit, the summary naming the failure
#      (`exit: FAIL ...`), the `codemode (optional):` line, the
#      `report: <path>` line pointing at
#      $GAUNTLET_REPORT_DIR\doctor.json, and that file written non-empty.
#   4. `gauntlet init` scaffolds a fresh project, and a second init refuses
#      to overwrite it. (The comments-only scaffold is deliberately NOT
#      doctor-clean, so the in-project leg below does not use it.)
#   5. `doctor` exits 0 (`exit: OK`, codemode line, `report: <path>` line,
#      non-empty report file) in a COPY of $env:GAUNTLET_SMOKE_PROJECT — a
#      real, reference-manifested project. GAUNTLET_SMOKE_PROJECT is a
#      REQUIRED caller obligation (CI passes the checkout's
#      tests/fixtures/target); the script refuses with typed exit-2 errors
#      rather than skip the leg silently when it is unset, not a readable
#      directory, or lacking GAUNTLET.md.
#   6. workbench leg (only when -Workbench is passed): HONEST BEST-EFFORT.
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
function Refuse([string]$Message) {
    # Typed caller-obligation refusal (exit 2, before any temp state is
    # created): the in-project doctor leg has no ruler, and a leg is never
    # skipped silently.
    Write-Host "smoke: REFUSE $Message" -ForegroundColor Yellow
    exit 2
}

if (-not (Test-Path $Gauntlet)) { Fail "not found: $Gauntlet" }
$GauntletBin = (Resolve-Path $Gauntlet).Path
if ($Workbench) {
    if (-not (Test-Path $Workbench)) { Fail "not found: $Workbench" }
    $WorkbenchBin = (Resolve-Path $Workbench).Path
}

# The in-project doctor leg's ruler: a doctor-clean project directory. The
# smoke cannot discover one itself, so the caller supplies it (a checkout's
# tests/fixtures/target). Validated BEFORE any temp state is created, so a
# refusal leaves nothing behind.
$smokeProjectInput = $env:GAUNTLET_SMOKE_PROJECT
if (-not $smokeProjectInput) {
    Refuse "no smoke project: set GAUNTLET_SMOKE_PROJECT to a doctor-clean project directory (a checkout's tests/fixtures/target)"
}
if (-not (Test-Path $smokeProjectInput -PathType Container)) {
    Refuse "GAUNTLET_SMOKE_PROJECT is not a readable directory: $smokeProjectInput"
}
$smokeProject = (Resolve-Path $smokeProjectInput).Path
if (-not (Test-Path (Join-Path $smokeProject 'GAUNTLET.md') -PathType Leaf)) {
    Refuse "GAUNTLET_SMOKE_PROJECT carries no GAUNTLET.md (want a doctor-clean project like tests/fixtures/target): $smokeProject"
}

$work = Join-Path ([System.IO.Path]::GetTempPath()) ("gauntlet-smoke-" + [guid]::NewGuid())
New-Item -ItemType Directory -Force -Path $work | Out-Null

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
        Fail "--version exited ${LASTEXITCODE}: $(Get-Content (Join-Path $work 'version.err') -Raw)"
    }
    $got = ('' + (Get-Content (Join-Path $work 'version.out') -Raw)).Trim()
    if ($got -ne "gauntlet $ExpectedVersion") {
        Fail "version: got '$got', want 'gauntlet $ExpectedVersion'"
    }
    Pass "version -> $got"

    # 2. --help
    & $GauntletBin --help > (Join-Path $work 'help.out') 2> (Join-Path $work 'help.err')
    if ($LASTEXITCODE -ne 0) {
        Fail "--help exited ${LASTEXITCODE}: $(Get-Content (Join-Path $work 'help.err') -Raw)"
    }
    if ((Get-Item (Join-Path $work 'help.out')).Length -eq 0) { Fail "--help produced no output" }
    Pass "--help exits 0 with usage output"

    # 3. doctor in an env-only dir FAILS CLOSED (the pinned contract)
    $envDir = Join-Path $work 'envdir'
    New-Item -ItemType Directory -Force -Path $envDir | Out-Null
    Push-Location $envDir
    try {
        & $GauntletBin doctor > (Join-Path $work 'doctor.out') 2> (Join-Path $work 'doctor.err')
        $doctorCode = $LASTEXITCODE
    }
    finally { Pop-Location }
    if ($doctorCode -eq 0) {
        Fail ("doctor exited 0 in an env-only dir; the fail-closed contract wants a typed " +
            "nonzero exit. stdout: $(Get-Content (Join-Path $work 'doctor.out') -Raw) " +
            "stderr: $(Get-Content (Join-Path $work 'doctor.err') -Raw)")
    }
    $doctorOut = Get-Content (Join-Path $work 'doctor.out') -Raw
    if (-not ($doctorOut -match '(?m)^exit: FAIL ')) {
        Fail "env-only doctor output lacks the typed 'exit: FAIL' summary line: $doctorOut"
    }
    if (-not ($doctorOut -match '(?m)^codemode \(optional\): ')) {
        Fail "env-only doctor output lacks the 'codemode (optional):' line"
    }
    $reportMatch = [regex]::Match($doctorOut, '(?m)^report: (.*)$')
    if (-not $reportMatch.Success) {
        Fail "env-only doctor output lacks the 'report: <path>' line"
    }
    $reportPath = $reportMatch.Groups[1].Value.Trim()
    $wantReport = Join-Path $env:GAUNTLET_REPORT_DIR 'doctor.json'
    if ($reportPath -ne $wantReport) {
        Fail "env-only doctor wrote its report at '$reportPath', want $wantReport"
    }
    if (-not (Test-Path $reportPath -PathType Leaf) -or (Get-Item $reportPath).Length -eq 0) {
        Fail "env-only doctor report file missing or empty: $reportPath"
    }
    Pass "doctor fails closed in an env-only dir (exit $doctorCode, report at $reportPath)"

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

    # 5. doctor exits 0 in a real, reference-manifested project. The ruler
    #    is a COPY of GAUNTLET_SMOKE_PROJECT (the original is never
    #    touched). An init scaffold does NOT suffice — the comments-only
    #    template configures no reference manifest, so doctor fails closed
    #    there by design.
    $projectCopy = Join-Path $work 'project-check'
    Copy-Item -Path $smokeProject -Destination $projectCopy -Recurse
    Push-Location $projectCopy
    try {
        & $GauntletBin doctor > (Join-Path $work 'doctor-project.out') 2> (Join-Path $work 'doctor-project.err')
        if ($LASTEXITCODE -ne 0) {
            Fail "doctor exited $LASTEXITCODE in the GAUNTLET_SMOKE_PROJECT copy: $(Get-Content (Join-Path $work 'doctor-project.err') -Raw)$(Get-Content (Join-Path $work 'doctor-project.out') -Raw)"
        }
    }
    finally { Pop-Location }
    $doctorProjectOut = Get-Content (Join-Path $work 'doctor-project.out') -Raw
    if (-not ($doctorProjectOut -match '(?m)^exit: OK')) {
        Fail "in-project doctor output lacks the 'exit: OK' summary line: $doctorProjectOut"
    }
    if (-not ($doctorProjectOut -match '(?m)^codemode \(optional\): ')) {
        Fail "in-project doctor output lacks the 'codemode (optional):' line"
    }
    $projectReportMatch = [regex]::Match($doctorProjectOut, '(?m)^report: (.*)$')
    if (-not $projectReportMatch.Success) {
        Fail "in-project doctor output lacks the 'report: <path>' line"
    }
    $projectReport = $projectReportMatch.Groups[1].Value.Trim()
    if (-not (Test-Path $projectReport -PathType Leaf) -or (Get-Item $projectReport).Length -eq 0) {
        Fail "in-project doctor report file missing or empty: $projectReport"
    }
    Pass "doctor exit 0 in the copied smoke project, report at $projectReport"

    # 6. workbench start leg (only when -Workbench is passed)
    if (-not $WorkbenchBin) {
        Note "workbench start leg skipped: not requested (no -Workbench) — recorded"
    }
    else {
        $wbCwd = Join-Path $work 'workbench-cwd'
        $projection = Join-Path $work 'projection/projections.db'
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
            $psi.EnvironmentVariables['GAUNTLET_WORKBENCH_PROJECTION_STORE'] = Join-Path $projection 'projections.db'

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
