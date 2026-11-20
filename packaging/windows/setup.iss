; Script for PdfCraft Inno Setup Installer
#define MyAppName "PdfCraft"
#ifndef MyAppVersion
#define MyAppVersion "0.21.1"
#endif
#define MyAppPublisher "ArtCraft"
#define MyAppURL "https://getartcraft.com/apps/pdfcraft"
#define MyAppExeName "pdfcraft.exe"
#define MyAppCliName "pdfcraft-cli.exe"

#ifndef SourceDir
#define SourceDir "..\..\target\windows-package\x64"
#endif
#ifndef OutputDir
#define OutputDir "..\..\dist\release"
#endif

[Setup]
AppId={{EA00C367-7270-4C50-9D8B-A7A5C88AD27F}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
AllowNoIcons=yes
LicenseFile=..\..\LICENSE-MIT
OutputDir={#OutputDir}
OutputBaseFilename=PdfCraft-Setup-x64
SetupIconFile=..\..\assets\app-icon\pdfcraft.ico
UninstallDisplayIcon={app}\pdfcraft.ico
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
ChangesAssociations=yes
DisableProgramGroupPage=auto

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "associatepdf"; Description: "Set PdfCraft as default handler for PDF files (.pdf)"; GroupDescription: "File Associations:"; Flags: unchecked

[Files]
Source: "{#SourceDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "..\..\assets\app-icon\pdfcraft.ico"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\pdfcraft.ico"
Name: "{group}\PdfCraft Command Line"; Filename: "{app}\{#MyAppCliName}"
Name: "{group}\{cm:UninstallProgram,{#MyAppName}}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\pdfcraft.ico"; Tasks: desktopicon

[Registry]
Root: HKA; Subkey: "Software\Classes\.pdf\OpenWithProgids"; ValueType: string; ValueName: "PdfCraft.Document"; ValueData: ""; Flags: uninsdeletevalue; Tasks: associatepdf
Root: HKA; Subkey: "Software\Classes\PdfCraft.Document"; ValueType: string; ValueName: ""; ValueData: "PDF Document"; Flags: uninsdeletekey; Tasks: associatepdf
Root: HKA; Subkey: "Software\Classes\PdfCraft.Document\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\pdfcraft.ico"; Flags: uninsdeletekey; Tasks: associatepdf
Root: HKA; Subkey: "Software\Classes\PdfCraft.Document\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"" ""%1"""; Flags: uninsdeletekey; Tasks: associatepdf

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent
