#ifndef AppVersion
  #define AppVersion "3.0.0"
#endif

[Setup]
AppId={{8C7A2F41-5D3B-4E19-9A62-4F8B1C2D3E4F}}
AppName=Trivium
AppVersion={#AppVersion}
AppVerName=Trivium {#AppVersion}
AppPublisher=Trivium (Open Source)
AppPublisherURL=https://github.com/Speed17x/Trivium
AppSupportURL=https://github.com/Speed17x/Trivium/issues
AppUpdatesURL=https://github.com/Speed17x/Trivium/releases
DefaultDirName={autopf}\Trivium
DefaultGroupName=Trivium
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
SetupIconFile=trivium.ico
UninstallDisplayIcon={app}\Trivium.exe
OutputDir=dist
OutputBaseFilename=Trivium-Setup-x64
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
Source: "dist\Trivium-Windows\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\Trivium"; Filename: "{app}\Trivium.exe"; IconFilename: "{app}\Trivium.exe"
Name: "{group}\Desinstalar Trivium"; Filename: "{uninstallexe}"
Name: "{autodesktop}\Trivium"; Filename: "{app}\Trivium.exe"; IconFilename: "{app}\Trivium.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\Trivium.exe"; Description: "{cm:LaunchProgram,Trivium}"; Flags: nowait postinstall skipifsilent
