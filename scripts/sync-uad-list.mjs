#!/usr/bin/env node
/**
 * Download the full Universal Android Debloater Next Generation (UAD-NG)
 * community package database into DozeForge's app data directory as
 * `community_bloat.json`.
 *
 * IMPORTANT: the UAD-NG list is GPL-3.0. DozeForge itself is MIT and ships only
 * its own seed (src-tauri/resources/bloatware_seed.json). This script does NOT
 * write into the repo or the binary — it writes to your local app data dir, so
 * the GPL data stays a user-provided runtime overlay and is never redistributed
 * inside the DozeForge binary. DozeForge loads it automatically on next launch
 * and overlays it on top of the bundled seed.
 *
 * Usage:  node scripts/sync-uad-list.mjs
 *
 * Requires Node 18+ (global fetch). No dependencies.
 */
import { writeFile, mkdir } from 'node:fs/promises';
import { homedir, platform } from 'node:os';
import { join } from 'node:path';

const UPSTREAM =
  'https://raw.githubusercontent.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation/main/resources/assets/uad_lists.json';

// Must match tauri.conf.json "identifier".
const APP_ID = 'io.forgeandroid.app';

/** Resolve Tauri's app_data_dir for the current OS. */
function appDataDir() {
  const home = homedir();
  switch (platform()) {
    case 'win32':
      return join(process.env.APPDATA || join(home, 'AppData', 'Roaming'), APP_ID);
    case 'darwin':
      return join(home, 'Library', 'Application Support', APP_ID);
    default: // linux and others
      return join(process.env.XDG_DATA_HOME || join(home, '.local', 'share'), APP_ID);
  }
}

async function main() {
  const dir = appDataDir();
  const out = join(dir, 'community_bloat.json');

  console.log(`Fetching UAD-NG community list (GPL-3.0)…\n  ${UPSTREAM}`);
  const res = await fetch(UPSTREAM, { headers: { 'User-Agent': 'DozeForge-sync' } });
  if (!res.ok) throw new Error(`HTTP ${res.status} ${res.statusText}`);

  const text = await res.text();
  let data;
  try {
    data = JSON.parse(text);
  } catch (e) {
    throw new Error(`Upstream returned invalid JSON: ${e.message}`);
  }

  const count = Object.keys(data).filter((k) => !k.startsWith('_')).length;
  if (count < 500) {
    throw new Error(`Sanity check failed: only ${count} packages parsed. Aborting.`);
  }

  data._meta = {
    source: 'Universal Android Debloater Next Generation (UAD-NG)',
    source_url:
      'https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation',
    license: 'GPL-3.0',
    synced_at: new Date().toISOString(),
    package_count: count,
    note: 'User-downloaded runtime overlay. Not part of the DozeForge binary.',
  };

  await mkdir(dir, { recursive: true });
  await writeFile(out, JSON.stringify(data, null, 0) + '\n', 'utf8');
  console.log(`✓ Wrote ${count} packages to\n  ${out}`);
  console.log('Restart DozeForge to load the community overlay (no rebuild needed).');
}

main().catch((err) => {
  console.error(`\n✗ Sync failed: ${err.message}`);
  console.error('DozeForge still works with its bundled seed; this only adds the community overlay.');
  process.exit(1);
});
