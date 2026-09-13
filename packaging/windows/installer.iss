; Inno Setup script for Game of Life (conway_life.exe).
; Built by .github/workflows/release.yml on windows-latest via:
;   iscc packaging\windows\installer.iss /DAppVersion=X.Y.Z
; expecting the release binary already at target\release\conway_life.exe.
; AppVersion falls back to 0.0.0 so the script is still runnable standalone
; (`iscc packaging\windows\installer.iss`, no /D flag) for local testing.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

[Setup]
AppId={{6F6E8B6B-6A6E-4F0A-9C9C-2E6D6C2E9C1A}
AppName=Game of Life
AppVersion={#AppVersion}
AppPublisher=David7ce
AppPublisherURL=https://github.com/David7ce/celular-automata-rust
DefaultDirName={autopf}\GameOfLife
DefaultGroupName=Game of Life
DisableProgramGroupPage=yes
OutputDir=..\..\dist
OutputBaseFilename=ConwayLife-{#AppVersion}-Windows-Setup
Compression=lzma2
SolidCompression=yes
SetupIconFile=..\icons\icon.ico
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"

[Files]
Source: "..\..\target\release\conway_life.exe"; DestDir: "{app}"; DestName: "GameOfLife.exe"; Flags: ignoreversion

[Icons]
Name: "{group}\Game of Life"; Filename: "{app}\GameOfLife.exe"
Name: "{group}\{cm:UninstallProgram,Game of Life}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\Game of Life"; Filename: "{app}\GameOfLife.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\GameOfLife.exe"; Description: "{cm:LaunchProgram,Game of Life}"; Flags: nowait postinstall skipifsilent
