# scrcpy bundled with DozeForge

This folder is packaged inside the installer (MSI/EXE) as a **Tauri resource**
(see `tauri.conf.json → bundle.resources`). At runtime, DozeForge looks here for
`scrcpy.exe` before falling back to the system `PATH`, so **anyone who installs
the app can use "Screen Mirror" without installing scrcpy separately**.

The binaries themselves are **not** committed to git (~40 MB of third-party
Windows DLLs). Only this README is versioned.

## Get the binaries (automatic — recommended)

Nothing to do by hand. `npm install` runs `postinstall`, which downloads the
official scrcpy Windows build into this folder. You can also run it explicitly:

```powershell
npm run setup:scrcpy             # download if missing
npm run setup:scrcpy -- --force  # re-download / upgrade
```

Pin a specific version with an env var:

```powershell
$env:SCRCPY_VERSION = "v3.3.4"; npm run setup:scrcpy
```

The script (`scripts/setup-scrcpy.mjs`) is idempotent and non-fatal: it skips
work if `scrcpy.exe` is already here, and it never breaks `npm install` if the
download fails (mirroring simply falls back to a system-wide scrcpy).

## Get the binaries (manual — offline machines)

1. Download the official Windows 64-bit release:
   https://github.com/Genymobile/scrcpy/releases  → `scrcpy-win64-vX.Y.zip`
2. Unzip and copy **all** of its contents into this folder (`src-tauri/scrcpy/`).
   scrcpy needs the whole set of files together to start:

   ```
   src-tauri/scrcpy/
     scrcpy.exe
     scrcpy-server
     adb.exe
     SDL3.dll
     avcodec-*.dll
     avformat-*.dll
     avutil-*.dll
     swresample-*.dll
     ... (the rest of the DLLs from the zip)
   ```
3. Rebuild: `npm run tauri build` (or `npm run tauri dev` to test).

## Backend resolution order

`resolve_scrcpy()` in `src-tauri/src/ipc/commands.rs` searches, in order:

1. The bundled resource (this folder, inside the installed app).
2. A `scrcpy/` folder next to the DozeForge executable.
3. The system `PATH`.
4. Common locations (scoop, chocolatey, `C:\Program Files\scrcpy`).
