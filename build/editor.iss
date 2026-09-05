; Inno Setup definition for the Windows desktop installer.
; Build with: iscc /DMyAppVersion=0.1.0 build/editor.iss
#ifndef MyAppVersion
  #define MyAppVersion "0.1.0"
#endif

[Setup]
AppName=Editor
AppVersion={#MyAppVersion}
DefaultDirName={autopf}\Editor
DefaultGroupName=Editor
OutputDir=..\target\package
OutputBaseFilename=Editor-{#MyAppVersion}-setup
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=lowest
Compression=lzma2
SolidCompression=yes

[Files]
Source: "..\target\release\editor.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Editor"; Filename: "{app}\editor.exe"
Name: "{autodesktop}\Editor"; Filename: "{app}\editor.exe"; Tasks: desktopicon

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Additional icons:"
