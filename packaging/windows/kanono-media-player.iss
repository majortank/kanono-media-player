#define AppName "Kanono Media Player"
#define AppPublisher "Kanono Contributors"
#define AppURL "https://github.com/majortank/kanono-media-player"
#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif

[Setup]
AppId={{A57A8A9A-04F1-4C2C-A1D9-9CC4F40359B3}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher={#AppPublisher}
AppPublisherURL={#AppURL}
DefaultDirName={autopf}\Kanono Media Player
DefaultGroupName={#AppName}
SetupIconFile=..\..\assets\icons\kanono-player.ico
UninstallDisplayIcon={app}\kanono-player.ico
OutputDir=..\..\dist
OutputBaseFilename=kanono-media-player-windows-x64-setup
Compression=lzma
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
VersionInfoCompany={#AppPublisher}
VersionInfoDescription=Kanono Media Player Setup
VersionInfoProductName={#AppName}
VersionInfoProductVersion={#AppVersion}
VersionInfoCopyright=Copyright (C) 2026 {#AppPublisher}

[Files]
Source: "..\..\target\x86_64-pc-windows-msvc\release\kanono-player.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\assets\icons\kanono-player.ico"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Kanono Media Player"; Filename: "{app}\kanono-player.exe"; IconFilename: "{app}\kanono-player.ico"
Name: "{autodesktop}\Kanono Media Player"; Filename: "{app}\kanono-player.exe"; Tasks: desktopicon; IconFilename: "{app}\kanono-player.ico"

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Additional shortcuts:"

[Run]
Filename: "{app}\kanono-player.exe"; Description: "Launch Kanono Media Player"; Flags: nowait postinstall skipifsilent