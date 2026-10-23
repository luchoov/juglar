; The installer (D60), compiled by Inno Setup 6 from the release build:
;
;   cargo build --release
;   iscc app\installer\juglar.iss
;
; which writes target\installer\juglar-<version>-setup.exe. The version is
; read from juglar.exe itself (build.rs puts Cargo.toml's there).
;
; Installed for the current user only, under %LOCALAPPDATA%\Programs, so it
; never asks for administrator rights; the zip stays for those who want
; nothing installed.

#define Exe "..\..\target\release\juglar.exe"
#define Version GetStringFileInfo(Exe, "ProductVersion")

[Setup]
; Never change the AppId: it is how a new version finds the old one.
AppId={{6F1D3C2A-9B47-4E8A-A5D2-1C7E0B9F4A31}
AppName=Juglar
AppVersion={#Version}
AppVerName=Juglar {#Version}
AppPublisher=luchoov
AppPublisherURL=https://github.com/luchoov/juglar
AppSupportURL=https://github.com/luchoov/juglar/issues
DefaultDirName={autopf}\Juglar
DefaultGroupName=Juglar
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
OutputDir=..\..\target\installer
OutputBaseFilename=juglar-{#Version}-setup
SetupIconFile=..\icon\juglar.ico
UninstallDisplayIcon={app}\juglar.exe
LicenseFile=..\..\LICENSE
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
; Juglar may be running (as the wallpaper, say) when it is updated or
; removed: close it through the Restart Manager, and start it again after.
CloseApplications=yes
RestartApplications=no

[Tasks]
Name: desktopicon; Description: "Put a shortcut on the desktop"; Flags: unchecked
Name: startup; Description: "Start Juglar with Windows, as the wallpaper"; Flags: unchecked

[Files]
Source: "{#Exe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"

[Icons]
Name: "{autoprograms}\Juglar"; Filename: "{app}\juglar.exe"
Name: "{autoprograms}\Juglar as the wallpaper"; Filename: "{app}\juglar.exe"; Parameters: "--wallpaper"
Name: "{autodesktop}\Juglar"; Filename: "{app}\juglar.exe"; Tasks: desktopicon

[Registry]
; Starting with Windows is one value under the user's Run key, removed with
; the program; Task Manager's Startup apps page can turn it off too.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "Juglar"; ValueData: """{app}\juglar.exe"" --wallpaper"; Flags: uninsdeletevalue; Tasks: startup
; Updated with the box unticked, it stops starting with Windows.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueName: "Juglar"; Flags: deletevalue dontcreatekey; Tasks: not startup

[Run]
Filename: "{app}\juglar.exe"; Description: "Start Juglar now"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
; The log files (D56), the only thing Juglar writes outside its folder.
Type: filesandordirs; Name: "{localappdata}\Juglar"

[Code]
// The uninstaller does not close running programs the way Setup does
// (CloseApplications), so it stops the Juglar it is removing, found by its
// path: one started from the zip elsewhere is left alone. Juglar keeps
// nothing that stopping it would lose.
function InitializeUninstall(): Boolean;
var
  Code: Integer;
begin
  Exec(ExpandConstant('{sys}\WindowsPowerShell\v1.0\powershell.exe'),
    '-NoProfile -NonInteractive -Command "Get-Process juglar -ErrorAction SilentlyContinue | Where-Object Path -eq ''' + ExpandConstant('{app}\juglar.exe') + ''' | Stop-Process -Force; Start-Sleep -Milliseconds 300"',
    '', SW_HIDE, ewWaitUntilTerminated, Code);
  Result := True;
end;
