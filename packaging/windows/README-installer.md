# WordCraft Windows Installer

## What it produces

Running the Inno Setup script `wordcraft.iss` produces a single EXE installer:

```
packaging\windows\Output\wordcraft-0.1.0-windows-x64-setup.exe
```

The installer:
- Installs WordCraft + CLI to `C:\Program Files\WordCraft` (admin) or `%LOCALAPPDATA%\Programs\WordCraft` (user).
- Registers WordCraft in **Windows Settings → Default Apps** so users can choose it as their document viewer.
- Adds **"Open with WordCraft"** to the right-click context menu on all supported files.
- Optionally sets WordCraft as the **default handler** for each file type (user must opt-in per checkbox).
- Adds the install folder to `PATH` so `wordcraft-cli` works from any terminal.
- Refreshes shell icon/association cache immediately after install (no reboot needed).

## Supported file types

| Extension | Description                    | Default handler task |
|-----------|--------------------------------|----------------------|
| `.docx`   | Word Open XML Document         | `assoc_docx` |
| `.doc`    | Word 97–2003 Document          | `assoc_doc`  |
| `.dotx`   | Word Open XML Template         | (always "Open With") |
| `.dot`    | Word 97–2003 Template          | (always "Open With") |
| `.rtf`    | Rich Text Format               | `assoc_rtf`  |
| `.odt`    | OpenDocument Text              | `assoc_odt`  |
| `.txt`    | Plain Text                     | `assoc_txt`  |
| `.md`     | Markdown                       | `assoc_md`   |

## How to build

### Prerequisites
1. **Inno Setup 6** — download from https://jrsoftware.org/isdl.php
2. **Release binaries** — run first:
   ```powershell
   $env:PATH = "C:\...\mingw64\bin;$env:USERPROFILE\.cargo\bin;$env:PATH"
   cargo build --release -p wordcraft -p wordcraft-cli
   Copy-Item target\release\wordcraft.exe dist\wordcraft\wordcraft-0.1.0-windows-x64-portable\
   Copy-Item target\release\wordcraft-cli.exe dist\wordcraft\wordcraft-0.1.0-windows-x64-portable\
   ```

### Compile the installer
```powershell
# From the repo root:
& "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" packaging\windows\wordcraft.iss
```

The output `wordcraft-0.1.0-windows-x64-setup.exe` will be at `packaging\windows\Output\`.

## Signing (optional but recommended)

To avoid SmartScreen warnings, sign the output EXE with a code-signing certificate:
```powershell
signtool sign /tr http://timestamp.digicert.com /td sha256 /fd sha256 /a `
  packaging\windows\Output\wordcraft-0.1.0-windows-x64-setup.exe
```

## Uninstall

Via **Settings → Apps → Installed Apps → WordCraft → Uninstall**, or from **Control Panel → Programs**.
All registry keys written by the installer are automatically cleaned up.
