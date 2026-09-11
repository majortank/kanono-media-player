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
UninstallDisplayIcon={app}\kanono-player.exe
OutputDir=dist
OutputBaseFilename=kanono-media-player-windows-x64-setup
Compression=lzma
SolidCompression=yes
WizardStyle=modern

[Files]
Source: "target\x86_64-pc-windows-msvc\release\kanono-player.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Kanono Media Player"; Filename: "{app}\kanono-player.exe"
Name: "{autodesktop}\Kanono Media Player"; Filename: "{app}\kanono-player.exe"; Tasks: desktopicon

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Additional shortcuts:"

[Run]
Filename: "{app}\kanono-player.exe"; Description: "Launch Kanono Media Player"; Flags: nowait postinstall skipifsilent