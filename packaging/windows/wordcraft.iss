; WordCraft Windows Installer — Inno Setup 6
; Builds a signed, all-users setup with full file-association registration.
; Double-clicking any .docx / .doc / .dotx / .rtf / .odt / .txt / .md opens WordCraft.
;
; Prerequisites:
;   - Inno Setup 6.x   https://jrsoftware.org/isdl.php
;   - Release binaries must exist at:
;       ..\..\dist\wordcraft\wordcraft-0.1.0-windows-x64-portable\wordcraft.exe
;       ..\..\dist\wordcraft\wordcraft-0.1.0-windows-x64-portable\wordcraft-cli.exe
;
; Build:
;   iscc packaging\windows\wordcraft.iss
; Output:
;   packaging\windows\Output\wordcraft-0.1.0-windows-x64-setup.exe

; ── Definitions ──────────────────────────────────────────────────────────────

#define MyAppName      "WordCraft"
#define MyAppVersion   "0.4.1"
#define MyAppPublisher "M S Rana / Izhaan Intellect"
#define MyAppURL       "https://github.com/rbr48/wordcraft"
#define MyAppExe       "wordcraft.exe"
#define MyCliExe       "wordcraft-cli.exe"
#define SrcDir         "..\..\dist\wordcraft\wordcraft-0.4.1-windows-x64-portable"
#define IconDir        "..\..\assets\app-icon"

; ProgID names (must be unique, never re-use Microsoft's ProgIDs)
#define PIDOCX  "WordCraft.Document.Docx"
#define PIDOC   "WordCraft.Document.Doc"
#define PIDOTX  "WordCraft.Document.Dotx"
#define PIDOT   "WordCraft.Document.Dot"
#define PIRTF   "WordCraft.Document.Rtf"
#define PIODT   "WordCraft.Document.Odt"
#define PIOTDT  "WordCraft.Document.Odt.Template"
#define PITXT   "WordCraft.Document.Txt"
#define PIMD    "WordCraft.Document.Md"

; ── Setup section ─────────────────────────────────────────────────────────────

[Setup]
; Unique GUID — never change once published or "upgrade" detection breaks
AppId={{9F27C705-5926-4CD8-B479-7DFBC17233D2}

AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} {#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}

; Install for all users by default; switches to current-user if not elevated
DefaultDirName={autopf}\{#MyAppName}
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog

; Tell Windows to refresh file-association UI after install/uninstall
ChangesAssociations=yes

OutputDir=Output
OutputBaseFilename=wordcraft-{#MyAppVersion}-windows-x64-setup
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
WizardSmallImageFile={#IconDir}\wordcraft-1024.png
SetupIconFile={#IconDir}\wordcraft.ico
UninstallDisplayIcon={app}\{#MyAppExe}
UninstallDisplayName={#MyAppName} {#MyAppVersion}

; x64 only
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible

; Minimum Windows 10
MinVersion=10.0

; ── Languages ────────────────────────────────────────────────────────────────

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

; ── Setup Tasks ──────────────────────────────────────────────────────────────

[Tasks]
; Desktop shortcut — opt-in
Name: "desktopicon"; \
  Description: "{cm:CreateDesktopIcon}"; \
  GroupDescription: "{cm:AdditionalIcons}"; \
  Flags: unchecked

; File-association tasks — all opt-in by default so we don't hijack without consent
Name: "assoc_docx"; Description: "Set as default app for .docx (Word documents)";   GroupDescription: "Open with WordCraft:"; Flags: unchecked
Name: "assoc_doc";  Description: "Set as default app for .doc (Word 97–2003)";       GroupDescription: "Open with WordCraft:"; Flags: unchecked
Name: "assoc_rtf";  Description: "Set as default app for .rtf (Rich Text Format)";   GroupDescription: "Open with WordCraft:"; Flags: unchecked
Name: "assoc_odt";  Description: "Set as default app for .odt (OpenDocument Text)";  GroupDescription: "Open with WordCraft:"; Flags: unchecked
Name: "assoc_txt";  Description: "Set as default app for .txt (Plain text)";         GroupDescription: "Open with WordCraft:"; Flags: unchecked
Name: "assoc_md";   Description: "Set as default app for .md  (Markdown)";           GroupDescription: "Open with WordCraft:"; Flags: unchecked

; ── Files ────────────────────────────────────────────────────────────────────

[Files]
Source: "{#SrcDir}\{#MyAppExe}";      DestDir: "{app}"; Flags: ignoreversion
Source: "{#SrcDir}\{#MyCliExe}";      DestDir: "{app}"; Flags: ignoreversion
Source: "{#SrcDir}\README.md";         DestDir: "{app}"; Flags: ignoreversion
Source: "{#SrcDir}\LICENSE-APACHE";    DestDir: "{app}"; Flags: ignoreversion
Source: "{#SrcDir}\LICENSE-MIT";       DestDir: "{app}"; Flags: ignoreversion
Source: "{#SrcDir}\OFL-biz-ud-mincho.txt";  DestDir: "{app}"; Flags: ignoreversion
Source: "{#SrcDir}\OFL-biz-ud-pgothic.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SrcDir}\OFL-shippori-mincho.txt";DestDir: "{app}"; Flags: ignoreversion

; ── Icons (Start menu + Desktop + Quick-launch) ───────────────────────────────

[Icons]
; Start-menu
Name: "{autoprograms}\{#MyAppName}"; Filename: "{app}\{#MyAppExe}"

; Desktop (task-gated)
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExe}"; Tasks: desktopicon

[Registry]
; ── Registry ─────────────────────────────────────────────────────────────────
;
; Strategy
; ─────────
; 1. Always register our ProgIDs (WordCraft.Document.*) so "Open With" lists us.
; 2. Add OpenWithProgids for every extension we support (allows "Set as default" via
;    Windows Settings without touching UserChoice — touch UserChoice silently fails
;    since Windows 8).
; 3. For extensions the user ticked, write UserChoice via SHAssocEnumHandlers path
;    — we do NOT write UserChoice directly (Windows would ignore or reset it).
;    Instead we set the Default value on the extension key which Windows honours
;    for elevated/per-machine installs in HKA/HKLM, and the user can confirm in
;    the "How do you want to open this file?" dialog that appears on first open.
;
; HKA  = HKCU when non-elevated, HKLM when elevated (Inno auto-selects).

; ─── 1. Register ProgIDs for every supported format ─────────────────────────

; --- .docx ---
Root: HKA; Subkey: "Software\Classes\{#PIDOCX}";                         ValueType: string; ValueName: ""; ValueData: "Word Document";                           Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\{#PIDOCX}";                         ValueType: string; ValueName: "FriendlyTypeName"; ValueData: "WordCraft Document"
Root: HKA; Subkey: "Software\Classes\{#PIDOCX}\DefaultIcon";             ValueType: string; ValueName: ""; ValueData: "{app}\{#MyAppExe},0"
Root: HKA; Subkey: "Software\Classes\{#PIDOCX}\shell";                   ValueType: string; ValueName: ""; ValueData: "open"
Root: HKA; Subkey: "Software\Classes\{#PIDOCX}\shell\open";              ValueType: string; ValueName: "FriendlyAppName"; ValueData: "{#MyAppName}"
Root: HKA; Subkey: "Software\Classes\{#PIDOCX}\shell\open\command";      ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExe}"" ""%1"""
Root: HKA; Subkey: "Software\Classes\{#PIDOCX}\shell\open\ddeexec";      ValueType: string; ValueName: ""; ValueData: ""

; --- .doc ---
Root: HKA; Subkey: "Software\Classes\{#PIDOC}";                          ValueType: string; ValueName: ""; ValueData: "Word 97-2003 Document";                   Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\{#PIDOC}";                          ValueType: string; ValueName: "FriendlyTypeName"; ValueData: "WordCraft Document"
Root: HKA; Subkey: "Software\Classes\{#PIDOC}\DefaultIcon";              ValueType: string; ValueName: ""; ValueData: "{app}\{#MyAppExe},0"
Root: HKA; Subkey: "Software\Classes\{#PIDOC}\shell";                    ValueType: string; ValueName: ""; ValueData: "open"
Root: HKA; Subkey: "Software\Classes\{#PIDOC}\shell\open\command";       ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExe}"" ""%1"""

; --- .dotx ---
Root: HKA; Subkey: "Software\Classes\{#PIDOTX}";                         ValueType: string; ValueName: ""; ValueData: "Word Template";                           Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\{#PIDOTX}\DefaultIcon";             ValueType: string; ValueName: ""; ValueData: "{app}\{#MyAppExe},0"
Root: HKA; Subkey: "Software\Classes\{#PIDOTX}\shell\open\command";      ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExe}"" ""%1"""

; --- .dot ---
Root: HKA; Subkey: "Software\Classes\{#PIDOT}";                          ValueType: string; ValueName: ""; ValueData: "Word 97-2003 Template";                   Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\{#PIDOT}\DefaultIcon";              ValueType: string; ValueName: ""; ValueData: "{app}\{#MyAppExe},0"
Root: HKA; Subkey: "Software\Classes\{#PIDOT}\shell\open\command";       ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExe}"" ""%1"""

; --- .rtf ---
Root: HKA; Subkey: "Software\Classes\{#PIRTF}";                          ValueType: string; ValueName: ""; ValueData: "Rich Text Document";                      Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\{#PIRTF}\DefaultIcon";              ValueType: string; ValueName: ""; ValueData: "{app}\{#MyAppExe},0"
Root: HKA; Subkey: "Software\Classes\{#PIRTF}\shell\open\command";       ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExe}"" ""%1"""

; --- .odt ---
Root: HKA; Subkey: "Software\Classes\{#PIODT}";                          ValueType: string; ValueName: ""; ValueData: "OpenDocument Text";                       Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\{#PIODT}\DefaultIcon";              ValueType: string; ValueName: ""; ValueData: "{app}\{#MyAppExe},0"
Root: HKA; Subkey: "Software\Classes\{#PIODT}\shell\open\command";       ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExe}"" ""%1"""

; --- .txt ---
Root: HKA; Subkey: "Software\Classes\{#PITXT}";                          ValueType: string; ValueName: ""; ValueData: "Plain Text Document";                     Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\{#PITXT}\DefaultIcon";              ValueType: string; ValueName: ""; ValueData: "{app}\{#MyAppExe},0"
Root: HKA; Subkey: "Software\Classes\{#PITXT}\shell\open\command";       ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExe}"" ""%1"""

; --- .md ---
Root: HKA; Subkey: "Software\Classes\{#PIMD}";                           ValueType: string; ValueName: ""; ValueData: "Markdown Document";                       Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\{#PIMD}\DefaultIcon";               ValueType: string; ValueName: ""; ValueData: "{app}\{#MyAppExe},0"
Root: HKA; Subkey: "Software\Classes\{#PIMD}\shell\open\command";        ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExe}"" ""%1"""

; ─── 2. OpenWithProgids — always register (adds "Open with WordCraft" to right-click) ───

; .docx
Root: HKA; Subkey: "Software\Classes\.docx\OpenWithProgids"; ValueType: string; ValueName: "{#PIDOCX}"; ValueData: ""; Flags: uninsdeletevalue
; .doc
Root: HKA; Subkey: "Software\Classes\.doc\OpenWithProgids";  ValueType: string; ValueName: "{#PIDOC}";  ValueData: ""; Flags: uninsdeletevalue
; .dotx
Root: HKA; Subkey: "Software\Classes\.dotx\OpenWithProgids"; ValueType: string; ValueName: "{#PIDOTX}"; ValueData: ""; Flags: uninsdeletevalue
; .dot
Root: HKA; Subkey: "Software\Classes\.dot\OpenWithProgids";  ValueType: string; ValueName: "{#PIDOT}";  ValueData: ""; Flags: uninsdeletevalue
; .rtf
Root: HKA; Subkey: "Software\Classes\.rtf\OpenWithProgids";  ValueType: string; ValueName: "{#PIRTF}";  ValueData: ""; Flags: uninsdeletevalue
; .odt
Root: HKA; Subkey: "Software\Classes\.odt\OpenWithProgids";  ValueType: string; ValueName: "{#PIODT}";  ValueData: ""; Flags: uninsdeletevalue
; .txt
Root: HKA; Subkey: "Software\Classes\.txt\OpenWithProgids";  ValueType: string; ValueName: "{#PITXT}";  ValueData: ""; Flags: uninsdeletevalue
; .md
Root: HKA; Subkey: "Software\Classes\.md\OpenWithProgids";   ValueType: string; ValueName: "{#PIMD}";   ValueData: ""; Flags: uninsdeletevalue

; ─── 3. Default handler — only when user ticked the task ────────────────────

; .docx → set as default (task-gated)
Root: HKA; Subkey: "Software\Classes\.docx"; ValueType: string; ValueName: ""; ValueData: "{#PIDOCX}"; Flags: uninsdeletevalue; Tasks: assoc_docx

; .doc → set as default (task-gated)
Root: HKA; Subkey: "Software\Classes\.doc";  ValueType: string; ValueName: ""; ValueData: "{#PIDOC}";  Flags: uninsdeletevalue; Tasks: assoc_doc

; .rtf → set as default (task-gated)
Root: HKA; Subkey: "Software\Classes\.rtf";  ValueType: string; ValueName: ""; ValueData: "{#PIRTF}";  Flags: uninsdeletevalue; Tasks: assoc_rtf

; .odt → set as default (task-gated)
Root: HKA; Subkey: "Software\Classes\.odt";  ValueType: string; ValueName: ""; ValueData: "{#PIODT}";  Flags: uninsdeletevalue; Tasks: assoc_odt

; .txt → set as default (task-gated)
Root: HKA; Subkey: "Software\Classes\.txt";  ValueType: string; ValueName: ""; ValueData: "{#PITXT}";  Flags: uninsdeletevalue; Tasks: assoc_txt

; .md → set as default (task-gated)
Root: HKA; Subkey: "Software\Classes\.md";   ValueType: string; ValueName: ""; ValueData: "{#PIMD}";   Flags: uninsdeletevalue; Tasks: assoc_md

; ─── 4. Default Programs / "Registered Applications" registration ────────────
; This adds WordCraft to "Default Apps" in Windows Settings (Apps → Default apps)

Root: HKA; Subkey: "Software\{#MyAppName}"; ValueType: string; ValueName: ""; ValueData: "{#MyAppName}"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\RegisteredApplications"; ValueType: string; ValueName: "{#MyAppName}"; ValueData: "Software\{#MyAppName}\Capabilities"; Flags: uninsdeletevalue

Root: HKA; Subkey: "Software\{#MyAppName}\Capabilities"; ValueType: string; ValueName: "ApplicationDescription"; ValueData: "Open-source word processor compatible with Microsoft Word documents"
Root: HKA; Subkey: "Software\{#MyAppName}\Capabilities"; ValueType: string; ValueName: "ApplicationIcon"; ValueData: "{app}\{#MyAppExe},0"
Root: HKA; Subkey: "Software\{#MyAppName}\Capabilities"; ValueType: string; ValueName: "ApplicationName"; ValueData: "{#MyAppName}"

Root: HKA; Subkey: "Software\{#MyAppName}\Capabilities\FileAssociations"; ValueType: string; ValueName: ".docx"; ValueData: "{#PIDOCX}"
Root: HKA; Subkey: "Software\{#MyAppName}\Capabilities\FileAssociations"; ValueType: string; ValueName: ".doc";  ValueData: "{#PIDOC}"
Root: HKA; Subkey: "Software\{#MyAppName}\Capabilities\FileAssociations"; ValueType: string; ValueName: ".dotx"; ValueData: "{#PIDOTX}"
Root: HKA; Subkey: "Software\{#MyAppName}\Capabilities\FileAssociations"; ValueType: string; ValueName: ".dot";  ValueData: "{#PIDOT}"
Root: HKA; Subkey: "Software\{#MyAppName}\Capabilities\FileAssociations"; ValueType: string; ValueName: ".rtf";  ValueData: "{#PIRTF}"
Root: HKA; Subkey: "Software\{#MyAppName}\Capabilities\FileAssociations"; ValueType: string; ValueName: ".odt";  ValueData: "{#PIODT}"
Root: HKA; Subkey: "Software\{#MyAppName}\Capabilities\FileAssociations"; ValueType: string; ValueName: ".txt";  ValueData: "{#PITXT}"
Root: HKA; Subkey: "Software\{#MyAppName}\Capabilities\FileAssociations"; ValueType: string; ValueName: ".md";   ValueData: "{#PIMD}"

; ─── 5. App Path — lets the OS find wordcraft.exe by name ───────────────────
Root: HKLM; Subkey: "Software\Microsoft\Windows\CurrentVersion\App Paths\{#MyAppExe}"; ValueType: string; ValueName: ""; ValueData: "{app}\{#MyAppExe}"; Flags: uninsdeletekey; Check: IsAdminInstallMode
Root: HKLM; Subkey: "Software\Microsoft\Windows\CurrentVersion\App Paths\{#MyAppExe}"; ValueType: string; ValueName: "Path"; ValueData: "{app}"

; ─── 6. Add to PATH so wordcraft-cli works from any terminal ────────────────
Root: HKCU; Subkey: "Environment"; ValueType: expandsz; ValueName: "Path"; ValueData: "{olddata};{app}"; Check: NeedsAddPath('{app}'); Flags: preservestringtype

; ── Run section ──────────────────────────────────────────────────────────────

[Run]
; After install: offer to launch
Filename: "{app}\{#MyAppExe}"; \
  Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; \
  Flags: nowait postinstall skipifsilent

; Refresh the shell so icons and associations update immediately (no reboot needed)
Filename: "{cmd}"; Parameters: "/c ie4uinit.exe -show"; Flags: runhidden; StatusMsg: "Refreshing file associations..."

; ── Uninstall Run ─────────────────────────────────────────────────────────────

[UninstallRun]
; Refresh shell after uninstall too
Filename: "{cmd}"; Parameters: "/c ie4uinit.exe -show"; Flags: runhidden; RunOnceId: "RefreshShellAssoc"

; ── Code section ─────────────────────────────────────────────────────────────

[Code]
/// Returns True when {app} is not yet on the PATH environment variable,
/// so the registry write only happens when actually needed.
function NeedsAddPath(AppPath: string): boolean;
var
  OrigPath: string;
begin
  if not RegQueryStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', OrigPath) then begin
    Result := True;
    exit;
  end;
  Result := Pos(';' + Uppercase(AppPath) + ';', ';' + Uppercase(OrigPath) + ';') = 0;
end;
