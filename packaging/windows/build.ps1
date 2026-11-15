# Build the Windows artifacts for Gauntlet: a standalone zip and a per-user
# Inno Setup installer.
#
# Usage: packaging/windows/build.ps1 -Version <v> -BinDir <dir> -OutDir <dir>
#
#   -Version <v>       x.y.z version (from packaging/version.sh / root Cargo.toml)
#   -BinDir <dir>      directory containing gauntlet.exe (required) and
#                      gauntlet-workbench.exe (OPTIONAL: stage it only when
#                      the Windows workbench was built AND launch-qualified;
#                      a CLI-only stage builds a CLI-only package)
#   -OutDir <dir>      output directory for both artifacts
#
# Produces:
#   gauntlet-<v>-x86_64-pc-windows-msvc.zip         (staged payload + LICENSE
#                                                    at the archive root)
#   gauntlet-<v>-x86_64-pc-windows-msvc-setup.exe   (per-user installer; the
#                                                    workbench is included
#                                                    only when staged)
#
# Both outputs are UNSIGNED: no Authenticode certificate is used and none is
# claimed anywhere. This script performs NO network fetches at package time:
# Inno Setup 6.3+ must already be installed (the CI workflow pre-installs
# it); the version is asserted below, never upgraded in place.
#
# This script also writes RELEASE-NOTES.txt into BinDir; gauntlet.iss shows
# it as the wizard's information page and installs it alongside the binaries.
#
# Pattern adapted from the proven sxr packaging/windows/build.ps1 +
# realitytrace.iss and sea-rs multi-binary layout.
param(
    [Parameter(Mandatory = $true, Position = 0)][string]$Version,
    [Parameter(Mandatory = $true, Position = 1)][string]$BinDir,
    [Parameter(Mandatory = $true, Position = 2)][string]$OutDir
)

$ErrorActionPreference = 'Stop'
if ($Version -notmatch '^[0-9]+\.[0-9]+\.[0-9]+([-+][0-9A-Za-z.-]+)?$') {
    throw "invalid version '$Version'"
}

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$bin = (Resolve-Path $BinDir).Path
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$out = (Resolve-Path $OutDir).Path
$license = Join-Path $repo 'LICENSE'
$base = "gauntlet-$Version-x86_64-pc-windows-msvc"
$cliExe = Join-Path $bin 'gauntlet.exe'
$workbenchExe = Join-Path $bin 'gauntlet-workbench.exe'

foreach ($file in @($cliExe, $license)) {
    if (-not (Test-Path $file)) { throw "missing required file $file" }
}
# The workbench is OPTIONAL: it is packaged only when the pipeline staged a
# launch-qualified Windows workbench. An unqualified workbench must never ride
# along as though it passed release validation. The decision is recorded on
# the build log either way.
$workbenchStaged = Test-Path $workbenchExe
if ($workbenchStaged) {
    Write-Host "workbench staged: $workbenchExe (packaging CLI + workbench)"
}
else {
    Write-Warning ("gauntlet-workbench.exe NOT staged in $bin — packaging CLI-ONLY " +
        '(the workbench is excluded from the zip and the installer and must not be ' +
        'presented as Windows-qualified)')
}

# Release notes: shown on the installer's information page and installed to
# {app}. Written before anything else so the .iss always finds it. The
# composition is stated honestly: the workbench is listed only when it was
# staged, and its absence is stated explicitly, never papered over.
$notes = Join-Path $bin 'RELEASE-NOTES.txt'
if ($workbenchStaged) {
    $composition = 'CLI + operator workbench (gauntlet-workbench.exe staged and included)'
    $installList = @"
  gauntlet.exe            the Gauntlet Rust CLI
  gauntlet-workbench.exe  the operator workbench TUI (Bun standalone executable)
  LICENSE                 the project licence
  RELEASE-NOTES.txt       this file
"@
    $workbenchPrereq = @"

  - gauntlet-workbench.exe is self-contained (Bun compiled standalone). The
    workbench TUI is a router/projection over the operator surface: set
    GAUNTLET_ENDPOINT (or GAUNTLET_WORKBENCH_ENDPOINT) and
    GAUNTLET_WORKBENCH_GAUNTLET_BINARY, and give it a writable working
    directory (or set GAUNTLET_WORKBENCH_PROJECTION_STORE).
"@
}
else {
    $composition = 'CLI ONLY (gauntlet-workbench.exe was NOT staged for this build and is NOT in this package)'
    $installList = @"
  gauntlet.exe            the Gauntlet Rust CLI
  LICENSE                 the project licence
  RELEASE-NOTES.txt       this file

  THIS PACKAGE DOES NOT INCLUDE gauntlet-workbench.exe: the Windows
  workbench was not staged/qualified for this build, so it is not shipped
  here as though it passed release validation.
"@
    $workbenchPrereq = ''
}
@"
Gauntlet $Version for Windows (x86_64-pc-windows-msvc)
Composition: $composition

What this installs
$installList

  The files are installed to %LOCALAPPDATA%\Programs\Gauntlet (per-user, no
  administrator rights) and that directory is appended to YOUR per-user PATH
  (HKCU\Environment). Uninstalling removes exactly these files and reverses
  the PATH edit.

THIS INSTALLER IS UNSIGNED
  No Authenticode certificate signs this installer or the binaries inside
  it. SmartScreen may show "Windows protected your PC"; choose More info ->
  Run anyway only after verifying the download against SHA256SUMS.

Runtime prerequisites (stated honestly)
  - A PostgreSQL server is REQUIRED for run/status/inspect/verify/export/
    serve/fabric: set GAUNTLET_DATABASE_URL to a real server. The doctor
    command does not probe PostgreSQL and never simulates one.
  - The bubblewrap sandbox backend is optional; the default is 'none', and a
    configured non-'none' backend without the sandbox fails closed.
  - The Bun runtime is optional (codemode only).$workbenchPrereq
"@ | Set-Content -Path $notes -Encoding UTF8

# Standalone zip: the staged payload and the licence at the archive root
# (gauntlet.exe always; gauntlet-workbench.exe only when staged).
$zip = Join-Path $out "$base.zip"
Remove-Item -Force -ErrorAction SilentlyContinue $zip
$stage = Join-Path ([System.IO.Path]::GetTempPath()) ("gauntlet-zip-" + [guid]::NewGuid())
New-Item -ItemType Directory -Force -Path $stage | Out-Null
try {
    Copy-Item $cliExe (Join-Path $stage 'gauntlet.exe')
    if ($workbenchStaged) {
        Copy-Item $workbenchExe (Join-Path $stage 'gauntlet-workbench.exe')
    }
    Copy-Item $license (Join-Path $stage 'LICENSE')
    Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip
}
finally {
    Remove-Item -Recurse -Force $stage -ErrorAction SilentlyContinue
}

# Installer: locate Inno Setup 6 (ISCC.exe).
$iscc = $null
$cmd = Get-Command iscc.exe -ErrorAction SilentlyContinue
if ($cmd) { $iscc = $cmd.Source }
if (-not $iscc) {
    foreach ($root in @(${env:ProgramFiles(x86)}, ${env:ProgramFiles})) {
        if (-not $root) { continue }
        $candidate = Join-Path $root 'Inno Setup 6\ISCC.exe'
        if (Test-Path $candidate) { $iscc = $candidate; break }
    }
}
if (-not $iscc) { throw 'ISCC.exe not found; install Inno Setup 6 (choco install innosetup)' }

function Get-InnoSetupVersion {
    foreach ($key in @(
            'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*',
            'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*',
            'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*'
        )) {
        $entry = Get-ItemProperty -Path $key -ErrorAction SilentlyContinue |
            Where-Object { $_.DisplayName -like 'Inno Setup*' -and $_.DisplayVersion } |
            Select-Object -First 1
        if ($entry) {
            try { return [version]$entry.DisplayVersion } catch { continue }
        }
    }
    return $null
}

# The .iss uses the x64compatible architecture constants, which Inno Setup
# introduced in 6.3. Assert, never upgrade in place: this script performs no
# network fetches at package time (the CI workflow pre-installs 6.3+).
$minimum = [version]'6.3'
$innoVersion = Get-InnoSetupVersion
if ($innoVersion -and $innoVersion -lt $minimum) {
    throw ("Inno Setup $innoVersion found at $iscc; the x64compatible constants require " +
        "Inno Setup >= 6.3. Upgrade with 'choco upgrade innosetup -y' or have the CI " +
        'workflow pre-install it; this script does not fetch from the network.')
}
if (-not $innoVersion) {
    Write-Warning ('could not determine the Inno Setup version from the registry; ' +
        'x64compatible requires >= 6.3, and ISCC fails with a clear compile error on older versions')
}

# Per-user installer via Inno Setup 6.
$setup = Join-Path $out "$base-setup.exe"
Remove-Item -Force -ErrorAction SilentlyContinue $setup
# The .iss lives at packaging/gauntlet.iss (one level above this script).
# /DIncludeWorkbench is passed only when the workbench exe was staged; the
# .iss gates the workbench [Files] entry (and therefore its uninstall record,
# which Inno derives from [Files]) on that define. The AppId is fixed either
# way, so CLI-only and full installs share one product identity for
# upgrade/uninstall.
$issFile = Join-Path (Split-Path $PSScriptRoot -Parent) 'gauntlet.iss'
$issDefines = @(
    "/DAppVersion=$Version",
    "/DBinDir=$bin",
    "/DLicenseFile=$license",
    "/DOutDir=$out",
    "/DSetupBase=$base-setup"
)
if ($workbenchStaged) { $issDefines += '/DIncludeWorkbench' }
& $iscc @issDefines $issFile
if ($LASTEXITCODE -ne 0) { throw "ISCC failed with exit code $LASTEXITCODE" }
if (-not (Test-Path $setup)) { throw "installer not produced: $setup" }

Get-ChildItem -Path $out -Filter "$base*" | Select-Object Name, Length |
    Format-Table -AutoSize | Out-String | Write-Host
foreach ($artifact in @($zip, $setup)) {
    $hash = (Get-FileHash -Path $artifact -Algorithm SHA256).Hash.ToLowerInvariant()
    Write-Host "sha256 ($(Split-Path -Leaf $artifact)): $hash"
}
