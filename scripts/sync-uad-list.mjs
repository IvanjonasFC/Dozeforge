#!/usr/bin/env node
/**
 * Sync the full Universal Android Debloater Next Generation (UAD-NG) package
 * database into src-tauri/resources/uad_lists.json.
 *
 * DozeForge embeds this file at compile time (heuristics/uad_list.rs). Shipping
 * a curated subset keeps the repo light; run this to swap in the complete
 * community list (~3000 packages) before a release build.
 *
 * Usage:  node scripts/sync-uad-list.mjs
 *
 * Requires Node 18+ (global fetch). No dependencies.
 */
import { writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const UPSTREAM =
  'https://raw.githubusercontent.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation/main/resources/assets/uad_lists.json';

const __dirname = dirname(fileURLToPath(import.meta.url));
const OUT = join(__dirname, '..', 'src-tauri', 'resources', 'uad_lists.json');

async function main() {
  console.log(`Fetching UAD-NG list…\n  ${UPSTREAM}`);
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

  // Preserve provenance for the About screen / audit.
  data._meta = {
    source: 'Universal Android Debloater Next Generation (UAD-NG)',
    source_url:
      'https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation',
    license: 'GPL-3.0',
    synced_at: new Date().toISOString(),
    package_count: count,
  };

  await writeFile(OUT, JSON.stringify(data, null, 0) + '\n', 'utf8');
  console.log(`✓ Wrote ${count} packages to\n  ${OUT}`);
  console.log('Rebuild src-tauri to embed the updated list (cargo build / tauri build).');
}

main().catch((err) => {
  console.error(`\n✗ Sync failed: ${err.message}`);
  console.error('The bundled curated list still works; this only updates it.');
  process.exit(1);
});
