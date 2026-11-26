#ifndef AppVersion
  #define AppVersion "2.2.0"
#endif

[Setup]
AppId={{8C7A2F41-5D3B-4E19-9A62-4F8B1C2D3E4F}}
AppName=AutoQ
AppVersion={#AppVersion}
AppVerName=AutoQ {#AppVersion}
AppPublisher=AutoQ (Open Source)
AppPublisherURL=https://github.com/Speed17x/AutoQ
AppSupportURL=https://github.com/Speed17x/AutoQ/issues
AppUpdatesURL=https://github.com/Speed17x/AutoQ/releases
DefaultDirName={autopf}\AutoQ
DefaultGroupName=AutoQ
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
SetupIconFile=autoq.ico
UninstallDisplayIcon={app}\AutoQ.exe
OutputDir=dist
OutputBaseFilename=AutoQ-Setup-x64
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible

[Languages]
Name: "spanish"; MessagesFile: "compiler:Languages\Spanish.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "dist\AutoQ-Windows\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\AutoQ"; Filename: "{app}\AutoQ.exe"; IconFilename: "{app}\AutoQ.exe"
Name: "{group}\Desinstalar AutoQ"; Filename: "{uninstallexe}"
Name: "{autodesktop}\AutoQ"; Filename: "{app}\AutoQ.exe"; IconFilename: "{app}\AutoQ.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\AutoQ.exe"; Description: "{cm:LaunchProgram,AutoQ}"; Flags: nowait postinstall skipifsilent
