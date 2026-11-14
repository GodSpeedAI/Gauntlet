; Per-user installer for Gauntlet (Inno Setup 6, requires >= 6.3 for the
; x64compatible architecture constants).
; Unsigned: no Authenticode certificate is used, and none is claimed.
; Driven by build.ps1, which passes AppVersion, BinDir, LicenseFile, OutDir,
; SetupBase, and writes RELEASE-NOTES.txt into BinDir (shown on the wizard's
; information page via InfoBeforeFile and installed to {app}).
;
; build.ps1 additionally passes /DIncludeWorkbench when gauntlet-workbench.exe
; was staged (i.e. when the Windows workbench was built and launch-qualified).
; The workbench [Files] entry exists only under that define; its uninstall
; record is the one Inno derives from [Files], so nothing else needs gating.
; The AppId is fixed either way, so CLI-only and full installs share one
; product identity for upgrade/uninstall.
;
; The installer adds {app} to the per-user PATH (HKCU\Environment) so
; `gauntlet` (and `gauntlet-workbench` when included) work from a fresh
; shell. It installs no service and requires no administrator rights.

#ifndef AppVersion
  #error AppVersion must be defined with /DAppVersion=<version>
#endif
#ifndef BinDir
  #error BinDir must be defined with /DBinDir=<directory with gauntlet.exe; gauntlet-workbench.exe optional via /DIncludeWorkbench>
#endif
#ifndef LicenseFile
  #error LicenseFile must be defined with /DLicenseFile=<path to LICENSE>
#endif
#ifndef OutDir
  #define OutDir "."
#endif
#ifndef SetupBase
  #define SetupBase "gauntlet-" + AppVersion + "-x86_64-pc-windows-msvc-setup"
#endif

[Setup]
AppId={{4C9A7E1F-8B2D-4E63-9F5A-2D8C0B7E1A34}
AppName=Gauntlet
AppVersion={#AppVersion}
AppVerName=Gauntlet {#AppVersion}
AppPublisher=GodSpeedAI
AppPublisherURL=https://github.com/GodSpeedAI/Gauntlet
DefaultDirName={localappdata}\Programs\Gauntlet
DefaultGroupName=Gauntlet
DisableProgramGroupPage=yes
DisableDirPage=yes
DisableReadyPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
ChangesEnvironment=yes
OutputDir={#OutDir}
OutputBaseFilename={#SetupBase}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
LicenseFile={#LicenseFile}
InfoBeforeFile={#BinDir}\RELEASE-NOTES.txt
UninstallDisplayName=Gauntlet {#AppVersion}

[Files]
Source: "{#BinDir}\gauntlet.exe"; DestDir: "{app}"; Flags: ignoreversion
#ifdef IncludeWorkbench
Source: "{#BinDir}\gauntlet-workbench.exe"; DestDir: "{app}"; Flags: ignoreversion
#endif
Source: "{#LicenseFile}"; DestDir: "{app}"; DestName: "LICENSE"; Flags: ignoreversion
Source: "{#BinDir}\RELEASE-NOTES.txt"; DestDir: "{app}"; Flags: ignoreversion

[Code]
// Removes every PATH entry equal (case-insensitively) to Entry and returns the rest.
function RemovePathEntry(const PathValue, Entry: string): string;
var
  Remaining, Item: string;
  P: Integer;
begin
  Result := '';
  Remaining := PathValue;
  while Remaining <> '' do
  begin
    P := Pos(';', Remaining);
    if P = 0 then
    begin
      Item := Remaining;
      Remaining := '';
    end
    else
    begin
      Item := Copy(Remaining, 1, P - 1);
      Remaining := Copy(Remaining, P + 1, Length(Remaining) - P);
    end;
    if (Item <> '') and (CompareText(Item, Entry) <> 0) then
    begin
      if Result <> '' then
        Result := Result + ';';
      Result := Result + Item;
    end;
  end;
end;

// Appends {app} to the per-user PATH (HKCU\Environment), once.
procedure AddAppToUserPath();
var
  PathValue, AppDir, NewValue: string;
begin
  AppDir := ExpandConstant('{app}');
  if not RegQueryStringValue(HKCU, 'Environment', 'Path', PathValue) then
    PathValue := '';
  NewValue := RemovePathEntry(PathValue, AppDir);
  if NewValue <> '' then
    NewValue := NewValue + ';';
  NewValue := NewValue + AppDir;
  RegWriteExpandStringValue(HKCU, 'Environment', 'Path', NewValue);
end;

// Removes {app} from the per-user PATH on uninstall.
procedure RemoveAppFromUserPath();
var
  PathValue, NewValue: string;
begin
  if RegQueryStringValue(HKCU, 'Environment', 'Path', PathValue) then
  begin
    NewValue := RemovePathEntry(PathValue, ExpandConstant('{app}'));
    if NewValue <> PathValue then
      RegWriteExpandStringValue(HKCU, 'Environment', 'Path', NewValue);
  end;
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
    AddAppToUserPath();
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
    RemoveAppFromUserPath();
end;
