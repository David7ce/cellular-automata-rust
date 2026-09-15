; Inno Setup script for Cellular Automata (cellular_automata.exe).
; Built by .github/workflows/release.yml on windows-latest via:
;   iscc packaging\windows\installer.iss /DAppVersion=X.Y.Z
; expecting the release binary already at target\release\cellular_automata.exe.
; AppVersion falls back to 0.0.0 so the script is still runnable standalone
; (`iscc packaging\windows\installer.iss`, no /D flag) for local testing.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

[Setup]
AppId={{6F6E8B6B-6A6E-4F0A-9C9C-2E6D6C2E9C1A}
AppName=Cellular Automata
AppVersion={#AppVersion}
AppPublisher=David7ce
AppPublisherURL=https://github.com/David7ce/cellular-automata-rust
DefaultDirName={autopf}\CellularAutomata
DefaultGroupName=Cellular Automata
DisableProgramGroupPage=yes
OutputDir=..\..\dist
OutputBaseFilename=CellularAutomata-{#AppVersion}-Windows-Setup
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
Source: "..\..\target\release\cellular_automata.exe"; DestDir: "{app}"; DestName: "CellularAutomata.exe"; Flags: ignoreversion

[Icons]
Name: "{group}\Cellular Automata"; Filename: "{app}\CellularAutomata.exe"
Name: "{group}\{cm:UninstallProgram,Cellular Automata}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\Cellular Automata"; Filename: "{app}\CellularAutomata.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\CellularAutomata.exe"; Description: "{cm:LaunchProgram,Cellular Automata}"; Flags: nowait postinstall skipifsilent
