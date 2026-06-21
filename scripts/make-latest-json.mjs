#!/usr/bin/env node
/**
 * Build latest.json manifest for tauri-plugin-updater (unsigned).
 *
 * Since pubkey is not configured, signature verification is disabled.
 * Only platform URLs are needed.
 *
 * Usage:
 *   node scripts/make-latest-json.mjs <version> <notes-json> <release-dir> <output-file>
 */

import { readdirSync, writeFileSync } from 'node:fs';

const version = process.argv[2];
const notesJson = process.argv[3];
const releaseDir = process.argv[4] || './release';
const outputFile = process.argv[5] || 'latest.json';

if (!version || !notesJson) {
  console.error('Usage: node scripts/make-latest-json.mjs <version> <notes-json> <release-dir> <output-file>');
  process.exit(1);
}

let notes;
try { notes = JSON.parse(notesJson); } catch { notes = notesJson; }

function detectPlatform(filename) {
  const lower = filename.toLowerCase();
  if (lower.endsWith('.msi') || lower.endsWith('.exe')) return 'windows-x86_64';
  if (lower.endsWith('.dmg')) return 'darwin-x86_64';
  if (lower.endsWith('.appimage') || lower.endsWith('.deb')) return 'linux-x86_64';
  return null;
}

const files = readdirSync(releaseDir).filter(f =>
  /\.(msi|exe|dmg|deb|AppImage)$/i.test(f)
);

if (files.length === 0) {
  console.error(`No installer files found in ${releaseDir}`);
  process.exit(1);
}

const cleanVersion = version.startsWith('v') ? version.slice(1) : version;
const tag = version.startsWith('v') ? version : `v${version}`;

const platforms = {};
for (const file of files) {
  const platform = detectPlatform(file);
  if (!platform) continue;
  platforms[platform] = {
    url: `https://github.com/Leaderxin/quant-desktop/releases/download/${tag}/${file}`
  };
}

const manifest = {
  version: cleanVersion,
  notes,
  pub_date: new Date().toISOString(),
  platforms,
};

writeFileSync(outputFile, JSON.stringify(manifest, null, 2) + '\n');
console.error(`latest.json written — ${Object.keys(platforms).length} platforms`);
