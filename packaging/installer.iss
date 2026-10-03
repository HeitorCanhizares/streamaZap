; Instalador do StreamaZap (Inno Setup 6).
; Compilado pelo GitHub Actions:  iscc /DAppVersion=1.2.3 packaging\installer.iss

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

[Setup]
AppId={{6B1F3E2A-8C4D-4F7B-9A51-2D3E8F0C7A11}
AppName=StreamaZap
AppVersion={#AppVersion}
AppPublisher=StreamaZap
AppPublisherURL=https://github.com/HeitorCanhizares/streamaZap
AppSupportURL=https://github.com/HeitorCanhizares/streamaZap/issues
AppUpdatesURL=https://github.com/HeitorCanhizares/streamaZap/releases
DefaultDirName={autopf}\StreamaZap
DefaultGroupName=StreamaZap
DisableProgramGroupPage=yes
OutputDir=..\installer
OutputBaseFilename=StreamaZap-Setup-{#AppVersion}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
; Administrador é necessário para liberar o app no Firewall do Windows
; (a rede do Radmin costuma ser classificada como "Pública").
PrivilegesRequired=admin
SetupIconFile=icon.ico
UninstallDisplayIcon={app}\StreamaZap.exe
CloseApplications=yes

[Languages]
Name: "brazilianportuguese"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"

[Files]
Source: "..\dist\StreamaZap\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\StreamaZap"; Filename: "{app}\StreamaZap.exe"
Name: "{group}\{cm:UninstallProgram,StreamaZap}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\StreamaZap"; Filename: "{app}\StreamaZap.exe"; Tasks: desktopicon

[Run]
; Apaga TODAS as regras do programa — inclusive as de BLOQUEIO que o Windows cria quando
; alguém clica em "Cancelar" no aviso do Firewall (elas vencem a regra de liberação e
; derrubam a conexão de vídeo na rede do Radmin, que costuma ser "Pública").
Filename: "{sys}\netsh.exe"; Parameters: "advfirewall firewall delete rule name=all program=""{app}\StreamaZap.exe"""; Flags: runhidden; StatusMsg: "Configurando o Firewall do Windows..."
Filename: "{sys}\netsh.exe"; Parameters: "advfirewall firewall add rule name=""StreamaZap"" dir=in action=allow program=""{app}\StreamaZap.exe"" enable=yes profile=any"; Flags: runhidden
Filename: "{sys}\netsh.exe"; Parameters: "advfirewall firewall add rule name=""StreamaZap"" dir=out action=allow program=""{app}\StreamaZap.exe"" enable=yes profile=any"; Flags: runhidden
Filename: "{app}\StreamaZap.exe"; Description: "{cm:LaunchProgram,StreamaZap}"; Flags: nowait postinstall skipifsilent
; Atualização automática (instalador rodando com /VERYSILENT): reabre o app já
; atualizado, como o usuário normal e não como administrador.
Filename: "{app}\StreamaZap.exe"; Flags: nowait runasoriginaluser; Check: WizardSilent

[UninstallRun]
Filename: "{sys}\netsh.exe"; Parameters: "advfirewall firewall delete rule name=all program=""{app}\StreamaZap.exe"""; Flags: runhidden; RunOnceId: "RemoveFirewallRule"
