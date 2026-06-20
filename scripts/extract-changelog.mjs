#!/usr/bin/env node
/**
 * Extract changelog entry for a specific version from CHANGELOG.md.
 *
 * Usage:
 *   node scripts/extract-changelog.mjs v1.2.0
 *
 * Expected CHANGELOG.md format (Keep a Changelog style):
 *   ## v1.2.0 (2026-06-20)
 *   ### Added
 *   - item 1
 *   - item 2
 *   ### Fixed
 *   - item 3
 *
 *   ## v1.1.1 (2026-06-15)
 *   ...
 */

import { readFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const version = process.argv[2];

if (!version) {
  console.error('Usage: node scripts/extract-changelog.mjs <version>');
  console.error('Example: node scripts/extract-changelog.mjs v1.2.0');
  process.exit(1);
}

// Normalize: accept both "1.2.0" and "v1.2.0"
const tag = version.startsWith('v') ? version : `v${version}`;
const altTag = version.startsWith('v') ? version.slice(1) : version;

const changelogPath = resolve(__dirname, '..', 'CHANGELOG.md');

let content;
try {
  content = readFileSync(changelogPath, 'utf-8');
} catch {
  console.error(`CHANGELOG.md not found at ${changelogPath}`);
  process.exit(1);
}

// Match the version section header
// e.g. "## v1.2.0" or "## 1.2.0" optionally followed by date
const versionRegex = new RegExp(
  `^##\\s+(${escapeRegex(tag)}|${escapeRegex(altTag)})\\b`,
  'm'
);
const match = content.match(versionRegex);

if (!match) {
  console.error(`Version ${tag} not found in CHANGELOG.md`);
  process.exit(1);
}

const startIndex = match.index;
// Find the next version header
const nextVersionMatch = content
  .slice(startIndex + match[0].length)
  .match(/^##\s+v?\d+\.\d+\.\d+/m);

const endIndex = nextVersionMatch
  ? startIndex + match[0].length + nextVersionMatch.index
  : content.length;

const entry = content.slice(startIndex, endIndex).trim();

if (!entry) {
  console.error(`Empty changelog entry for ${tag}`);
  process.exit(1);
}

// Output the extracted entry
process.stdout.write(entry + '\n');

function escapeRegex(s) {
  return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}
