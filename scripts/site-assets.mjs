// Assemble the static website's assets, then verify every local reference in
// website/index.html actually resolves.
//
// Why assemble at all: website/ is published to GitHub Pages as a self-contained
// directory, so its images must live inside it. Those images are already tracked
// under public/screenshots/ (the README uses them), and a second committed copy
// would drift the moment either side is updated. So they are copied here into
// website/assets/ — a build product, gitignored like dist/.
//
// Why verify: a mistyped screenshot name is invisible locally (the browser just
// shows a broken image) and invisible in review. Here it fails the build.
//
// Node alone, no dependencies: this runs both locally (`npm run site:assets`
// before opening website/index.html) and in .github/workflows/pages.yml.

import { cp, mkdir, readdir, readFile, rm } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { resolve, dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = resolve(__dirname, '..');
const siteDir = resolve(root, 'website');
const outDir = resolve(siteDir, 'assets');

// [source relative to repo root, destination relative to website/assets]
const FILES = [
  ['public/qrcode.png', 'qrcode.png'],
  ['src-tauri/icons/icon.png', 'icon.png'],
];

const SCREENSHOT_SRC = 'public/screenshots';
const ENTRY = 'index.html';

async function assemble(missing) {
  // Rebuild from scratch: a screenshot deleted upstream must not linger here
  // and get published as a ghost image.
  await rm(outDir, { recursive: true, force: true });
  await mkdir(outDir, { recursive: true });

  let copied = 0;

  for (const [src, dest] of FILES) {
    const from = resolve(root, src);
    if (!existsSync(from)) {
      missing.push(src);
      continue;
    }
    await cp(from, join(outDir, dest));
    copied += 1;
  }

  const shotDir = resolve(root, SCREENSHOT_SRC);
  if (!existsSync(shotDir)) {
    missing.push(SCREENSHOT_SRC);
  } else {
    const shots = await readdir(shotDir);
    if (shots.length === 0) missing.push(`${SCREENSHOT_SRC} (empty)`);
    await cp(shotDir, join(outDir, 'screenshots'), { recursive: true });
    copied += shots.length;
  }

  return copied;
}

// Every src/href in the page that points at something in this repo must exist.
// Absolute URLs (http:, //, mailto:, data:) and pure fragments are none of our
// business — they can rot without breaking the build.
function isLocalRef(ref) {
  return !/^([a-z][a-z0-9+.-]*:|\/\/|#)/i.test(ref);
}

async function verifyReferences() {
  const html = await readFile(resolve(siteDir, ENTRY), 'utf8');
  const broken = [];

  for (const [, ref] of html.matchAll(/(?:src|href)="([^"]+)"/g)) {
    const clean = ref.split(/[?#]/)[0];
    if (!clean || !isLocalRef(clean)) continue;
    if (!existsSync(resolve(siteDir, clean))) broken.push(ref);
  }

  return [...new Set(broken)];
}

async function main() {
  const missing = [];
  const copied = await assemble(missing);

  if (missing.length > 0) {
    console.error(`[site] Missing source assets: ${missing.join(', ')}`);
    process.exit(1);
  }

  const broken = await verifyReferences();
  if (broken.length > 0) {
    console.error(`[site] ${ENTRY} references files that do not exist:`);
    for (const ref of broken) console.error(`  - ${ref}`);
    process.exit(1);
  }

  console.log(`[site] Copied ${copied} files into website/assets/`);
  console.log(`[site] All local references in ${ENTRY} resolve.`);
  console.log('[site] Open website/index.html in a browser to preview.');
}

main().catch((err) => {
  console.error(`[site] Failed: ${err.message}`);
  process.exit(1);
});
