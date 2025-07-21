[Setup]
AppName=glorp
AppVersion=0.0.1
AppPublisher=slav
DefaultDirName={localappdata}\glorp
DisableDirPage=yes
DisableProgramGroupPage=yes
DisableReadyPage=yes
DisableFinishedPage=yes
PrivilegesRequired=lowest
OutputBaseFilename=glorp-setup-x64
CloseApplications=force
UninstallDisplayIcon={app}\glorp.exe
UninstallDisplayName=glorp
UninstallFilesDir={app}

[Files]
Source: "..\target\x86_64-pc-windows-msvc\release\render.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\target\x86_64-pc-windows-msvc\release\webview.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\target\x86_64-pc-windows-msvc\release\glorp.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\glorp"; Filename: "{app}\glorp.exe"
Name: "{userdesktop}\glorp"; Filename: "{app}\glorp.exe"

[Run]
Filename: "{app}\glorp.exe"; Description: "Launch glorp"; Flags: postinstall nowait runasoriginaluser

[InstallDelete] 
Type: filesandordirs; Name: "{app}"
