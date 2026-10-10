// Image pipeline for the website. Run by hand when a screenshot changes; the
// output is committed, so the Pages workflow never needs a browser or a native
// image library — which is the point. The site is a zero-dependency static
// page, and pulling in `sharp` (a native binary) to serve a dozen marketing
// images would tax every `npm ci` in CI for it.
//
// Two jobs:
//   1. WebP derivatives at three widths, plus the favicon / brand-icon sizes —
//      encoded by Chrome's own canvas (canvas.toDataURL), driven headlessly.
//   2. The 1200×630 social card — rendered from scripts/og-card.html, because
//      that one is a design rather than a resize.
//
// Usage:  node scripts/optimize-images.mjs
// Needs:  Chrome (auto-detected, or set CHROME_PATH).

import { readdir, writeFile, mkdir, rm } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { resolve, dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const run = promisify(execFile);
const __dirname = dirname(fileURLToPath(import.meta.url));
const root = resolve(__dirname, '..');

const SRC_DIR = resolve(root, 'screenshots');
const WEBSITE = resolve(root, 'website');
const ICON = resolve(root, 'src-tauri/icons/icon.png');
const CARD = resolve(__dirname, 'og-card.html');
const TMP = resolve(root, '.image-tmp');

const posix = (p) => p.replace(/\\/g, '/');

// Three widths, one per display context (the sizes= attributes in index.html
// say which): ~600 for the two-up feature pairs, ~1100 for a single feature
// image and the carousel at 1x, 2057 for the carousel at 2x.
const WIDTHS = [600, 1100, 2057];
const WEBP_QUALITY = 0.82;

// Square derivatives of the app icon. 32/180 are the favicon and the iOS home
// screen icon; 128 serves the hero (92px) and the brand marks (28/30px) — those
// were pulling the 512px master, 203 KB for a 28px slot.
const ICON_SIZES = [
  ['favicon-32.png', 32],
  ['favicon-180.png', 180],
  ['icon-128.png', 128],
];

const CHROME_CANDIDATES = [
  process.env.CHROME_PATH,
  'C:/Program Files/Google/Chrome/Application/chrome.exe',
  'C:/Program Files (x86)/Google/Chrome/Application/chrome.exe',
  '/usr/bin/google-chrome',
  '/usr/bin/chromium-browser',
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
].filter(Boolean);

const chromePath = CHROME_CANDIDATES.find((p) => existsSync(p));
if (!chromePath) {
  console.error('[img] Chrome not found. Set CHROME_PATH to its executable.');
  process.exit(1);
}

function withChrome(args) {
  return run(chromePath, [
    '--headless=new',
    '--disable-gpu',
    '--hide-scrollbars',
    '--no-first-run',
    // Without this, every file:// document is its own origin: drawing a
    // file:// image onto a canvas taints it and toDataURL throws.
    '--allow-file-access-from-files',
    `--user-data-dir=${posix(join(TMP, 'profile'))}`,
    ...args,
  ], { maxBuffer: 256 * 1024 * 1024 });
}

// Every derivative comes out of a single Chrome run: one page loads each source
// into an Image, draws it to a canvas at the target size, and hands back data
// URLs as JSON in the DOM. `--dump-dom` gives us that back without needing a
// devtools client (Node 20 here has no global WebSocket).
async function encodeDerivatives(sources) {
  const cfg = {
    sources: sources.map(posix),
    widths: WIDTHS,
    quality: WEBP_QUALITY,
    icons: ICON_SIZES,
    icon: posix(ICON),
  };

  const page = `<!DOCTYPE html><html><head><meta charset="utf-8"></head><body><script>
const cfg = ${JSON.stringify(cfg)};
const load = (p) => new Promise((ok, no) => {
  const i = new Image();
  i.onload = () => ok(i);
  i.onerror = () => no(new Error('cannot load ' + p));
  i.src = p;
});
function encode(img, url, w, h) {
  const c = document.createElement('canvas');
  c.width = w; c.height = h;
  const ctx = c.getContext('2d');
  ctx.imageSmoothingEnabled = true;
  ctx.imageSmoothingQuality = 'high';
  ctx.drawImage(img, 0, 0, w, h);
  return c.toDataURL(url, cfg.quality);
}
(async () => {
  const out = {};
  try {
    for (const src of cfg.sources) {
      const img = await load(src);
      const base = src.split('/').pop().replace(/\\.png$/, '');
      for (const w of cfg.widths) {
        // Never upscale, and never emit a file whose name lies about its size:
        // a srcset entry like "x-2057.webp 2057w" on a 1298px-wide image makes
        // the browser compute the wrong density. Sources smaller than every
        // target simply get no derivative and keep their PNG.
        if (w > img.naturalWidth) continue;
        const ch = Math.round(w * img.naturalHeight / img.naturalWidth);
        out['shot|' + base + '|' + w] = encode(img, 'image/webp', w, ch);
      }
    }
    const icon = await load(cfg.icon);
    for (const pair of cfg.icons) {
      out['icon|' + pair[0]] = encode(icon, 'image/png', pair[1], pair[1]);
    }
  } catch (e) {
    out.__error = String(e && e.message ? e.message : e);
  }
  const pre = document.createElement('pre');
  pre.id = 'enc';
  pre.textContent = JSON.stringify(out);
  document.body.appendChild(pre);
})();
</script></body></html>`;

  await writeFile(join(TMP, 'encode.html'), page);
  const { stdout } = await withChrome([
    '--virtual-time-budget=180000',
    '--dump-dom',
    `file:///${posix(join(TMP, 'encode.html'))}`,
  ]);

  const m = /<pre id="enc">([\s\S]*?)<\/pre>/.exec(stdout);
  if (!m) throw new Error('encoder produced no output (Chrome failed?)');
  const json = m[1].replace(/&amp;/g, '&').replace(/&lt;/g, '<').replace(/&gt;/g, '>');
  const out = JSON.parse(json);
  if (out.__error) throw new Error(out.__error);
  return out;
}

function decode(dataUrl) {
  return Buffer.from(dataUrl.slice(dataUrl.indexOf(',') + 1), 'base64');
}

async function main() {
  const pngs = (await readdir(SRC_DIR)).filter((f) => f.endsWith('.png')).sort();
  if (pngs.length === 0) throw new Error(`no PNGs in ${SRC_DIR}`);

  await rm(TMP, { recursive: true, force: true });
  await mkdir(TMP, { recursive: true });

  // Clear the previous run's derivatives first: the width rule can change, and
  // a leftover file is never referenced but still gets copied into the site and
  // committed. (The PNGs are sources and are left alone.)
  for (const f of await readdir(SRC_DIR)) {
    if (f.endsWith('.webp')) await rm(join(SRC_DIR, f), { force: true });
  }

  console.log(`[img] encoding ${pngs.length} screenshots at ${WIDTHS.join('/')}px …`);
  const out = await encodeDerivatives(pngs.map((f) => join(SRC_DIR, f)));

  let shots = 0;
  let icons = 0;
  for (const [key, dataUrl] of Object.entries(out)) {
    const [kind, a, b] = key.split('|');
    if (kind === 'shot') {
      await writeFile(join(SRC_DIR, `${a}-${b}.webp`), decode(dataUrl));
      shots += 1;
    } else {
      // Favicons and the small brand icon are site source, not derivatives of a
      // screenshot, so they live in website/ and are published as-is.
      await writeFile(join(WEBSITE, a), decode(dataUrl));
      icons += 1;
    }
  }
  console.log(`[img] screenshots/: ${shots} WebP derivatives`);
  console.log(`[img] website/: ${icons} icon sizes`);

  await withChrome([
    '--force-device-scale-factor=1',
    '--window-size=1200,630',
    '--virtual-time-budget=10000',
    `--screenshot=${posix(join(WEBSITE, 'og-cover.png'))}`,
    `file:///${posix(CARD)}`,
  ]);
  console.log('[img] website/og-cover.png (1200×630)');

  await rm(TMP, { recursive: true, force: true });
  console.log('[img] done — commit the output.');
}

main().catch(async (err) => {
  console.error(`[img] failed: ${err.message}`);
  await rm(TMP, { recursive: true, force: true }).catch(() => {});
  process.exit(1);
});
