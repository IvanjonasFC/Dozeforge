#!/usr/bin/env node
/**
 * setup-scrcpy.mjs — fetches the official scrcpy Windows build and drops it into
 * `src-tauri/scrcpy/` so the "Screen Mirror" feature works out of the box.
 *
 * The binaries are NOT versioned in git (they are ~40 MB of third-party Windows
 * DLLs). This script is wired into `postinstall`, so a plain `npm install`
 * prepares everything. You can also run it on demand:
 *
 *     npm run setup:scrcpy            # download if missing
 *     npm run setup:scrcpy -- --force # re-download / upgrade
 *     SCRCPY_VERSION=v3.3.4 npm run setup:scrcpy
 *
 * Design goals:
 *   - Idempotent: skips work if scrcpy.exe is already present.
 *   - Non-fatal: never breaks `npm install`. On any error it warns and exits 0.
 *   - Zero dependencies: Node built-in fetch + PowerShell Expand-Archive.
 */
import { existsSync, mkdirSync, rmSync, renameSync, readdirSync, createWriteStream } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { tmpdir } from 'node:os';
import { spawnSync } from 'node:child_process';
import { Readable } from 'node:stream';
import { pipeline } from 'node:stream/promises';

const SCRCPY_VERSION = process.env.SCRCPY_VERSION || 'v3.3.4';
const FORCE = process.argv.includes('--force');

const here = dirname(fileURLToPath(import.meta.url));
const scrcpyDir = join(here, '..', 'src-tauri', 'scrcpy');
const marker = join(scrcpyDir, 'scrcpy.exe');

// Screen mirroring is bundled for Windows only. Elsewhere, do nothing.
if (process.platform !== 'win32') {
  console.log('[setup-scrcpy] Non-Windows platform detected — skipping (mirror bundle is Windows-only).');
  process.exit(0);
}

if (existsSync(marker) && !FORCE) {
  console.log('[setup-scrcpy] scrcpy.exe already present — nothing to do. Use --force to re-download.');
  process.exit(0);
}

const asset = `scrcpy-win64-${SCRCPY_VERSION}.zip`;
const url = `https://github.com/Genymobile/scrcpy/releases/download/${SCRCPY_VERSION}/${asset}`;

async function main() {
  mkdirSync(scrcpyDir, { recursive: true });
  const tmpZip = join(tmpdir(), asset);
  const tmpOut = join(tmpdir(), `scrcpy-extract-${Date.now()}`);

  console.log(`[setup-scrcpy] Downloading ${url}`);
  const res = await fetch(url, { redirect: 'follow' });
  if (!res.ok || !res.body) {
    throw new Error(`HTTP ${res.status} ${res.statusText}`);
  }
  await pipeline(Readable.fromWeb(res.body), createWriteStream(tmpZip));

  console.log('[setup-scrcpy] Extracting…');
  rmSync(tmpOut, { recursive: true, force: true });
  const unzip = spawnSync(
    'powershell',
    ['-NoProfile', '-Command', `Expand-Archive -LiteralPath '${tmpZip}' -DestinationPath '${tmpOut}' -Force`],
    { stdio: 'inherit' }
  );
  if (unzip.status !== 0) throw new Error('Expand-Archive failed');

  // The zip contains a single top-level folder (scrcpy-win64-vX.Y.Z/). Flatten it.
  const entries = readdirSync(tmpOut, { withFileTypes: true });
  const root = entries.length === 1 && entries[0].isDirectory()
    ? join(tmpOut, entries[0].name)
    : tmpOut;

  for (const name of readdirSync(root)) {
    const dest = join(scrcpyDir, name);
    rmSync(dest, { recursive: true, force: true });
    renameSync(join(root, name), dest);
  }

  rmSync(tmpZip, { force: true });
  rmSync(tmpOut, { recursive: true, force: true });
  console.log(`[setup-scrcpy] Done. scrcpy ${SCRCPY_VERSION} is ready in src-tauri/scrcpy/.`);
}

main().catch((err) => {
  console.warn(`[setup-scrcpy] Could not set up scrcpy automatically: ${err.message}`);
  console.warn('[setup-scrcpy] Screen mirroring will fall back to a system-wide scrcpy on PATH.');
  console.warn(`[setup-scrcpy] You can retry later with:  npm run setup:scrcpy`);
  process.exit(0); // never break install
});
