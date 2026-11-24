// Generates the target-based unplated icon assets for the Microsoft Store
// (MSIX) package. Run by hand when the app icon changes; the output is
// committed, so the Store workflow never needs an image library.
//
// Why these exist: the taskbar/Start icon of a *packaged* app comes from the
// AppxManifest resource, not from the icon embedded in the exe. Windows only
// skips the colored "plate" behind that icon when it can resolve an
// `altform-unplated` / `altform-lightunplated` candidate for the exact target
// size; otherwise it draws a plate colored with `BackgroundColor` (and
// `transparent` there resolves to the user's accent color — the blue square).
// `tauri icon` does not emit these variants, so we make them ourselves.
//
// The qualifier suffixes are only honored when the files are indexed by
// `resources.pri` — see the "Package as MSIX" step in store-release.yml.
//
// Usage:  node scripts/msix-icons.mjs
// Needs:  Chrome (auto-detected, or set CHROME_PATH).

import { writeFile, readdir, mkdir, rm } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { resolve, dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const run = promisify(execFile);
const __dirname = dirname(fileURLToPath(import.meta.url));
const root = resolve(__dirname, '..');

// The 512px master the website icon derivatives also come from.
const ICON = resolve(root, 'src-tauri/icons/icon.png');
const OUT_DIR = resolve(root, 'src-tauri/icons/msix');
const WORKER = resolve(root, '.msix-icons-tmp');

const posix = (p) => p.replace(/\\/g, '/');

// Sizes the shell asks for at the usual DPIs: 16/20/24 are the taskbar and
// list glyphs, 30–96 cover 125–400% scaling, 256 is the largest requested.
// Exact-size assets keep Windows from rescaling (and from re-plating).
const TARGET_SIZES = [16, 20, 24, 30, 32, 36, 40, 44, 48, 60, 64, 72, 80, 96, 256];

// `unplated` is used on the dark shell, `lightunplated` on the light shell.
// The artwork is the same here (a colorful logo that reads on both) but both
// files must exist: when the matching qualifier is missing, Windows falls back
// to the plated image and the plate comes back.
const FORMS = ['unplated', 'lightunplated'];

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
  console.error('[msix-icons] Chrome not found. Set CHROME_PATH to its executable.');
  process.exit(1);
}

function withChrome(args) {
  return run(chromePath, [
    '--headless=new',
    '--disable-gpu',
    '--hide-scrollbars',
    '--no-first-run',
    // file:// documents are otherwise separate origins: drawing the file://
    // icon onto a canvas taints it and toDataURL throws.
    '--allow-file-access-from-files',
    `--user-data-dir=${posix(join(WORKER, 'profile'))}`,
    ...args,
  ], { maxBuffer: 64 * 1024 * 1024 });
}

// One Chrome run renders every variant: the page loads the master, draws it at
// each target size, and hands back PNG data URLs as JSON via --dump-dom.
async function encodeIcons() {
  const cfg = { icon: posix(ICON), sizes: TARGET_SIZES };
  const page = `<!DOCTYPE html><html><head><meta charset="utf-8"></head><body><script>
const cfg = ${JSON.stringify(cfg)};
const load = (p) => new Promise((ok, no) => {
  const i = new Image();
  i.onload = () => ok(i);
  i.onerror = () => no(new Error('cannot load ' + p));
  i.src = p;
});
(async () => {
  const out = {};
  try {
    const img = await load(cfg.icon);
    for (const n of cfg.sizes) {
      const c = document.createElement('canvas');
      c.width = n; c.height = n;
      const ctx = c.getContext('2d');
      ctx.imageSmoothingEnabled = true;
      ctx.imageSmoothingQuality = 'high';
      ctx.drawImage(img, 0, 0, n, n);
      out[n] = c.toDataURL('image/png');
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

  await writeFile(join(WORKER, 'encode.html'), page);
  const { stdout } = await withChrome([
    '--virtual-time-budget=120000',
    '--dump-dom',
    `file:///${posix(join(WORKER, 'encode.html'))}`,
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
  await rm(WORKER, { recursive: true, force: true });
  await mkdir(WORKER, { recursive: true });

  // Clear the previous run: a size/form dropped from the lists above must not
  // linger and keep getting packed.
  await rm(OUT_DIR, { recursive: true, force: true });
  await mkdir(OUT_DIR, { recursive: true });

  console.log(`[msix-icons] rendering ${TARGET_SIZES.length} sizes from ${ICON} …`);
  const encoded = await encodeIcons();

  let count = 0;
  for (const n of TARGET_SIZES) {
    const png = decode(encoded[n]);
    for (const form of FORMS) {
      const name = `Square44x44Logo.targetsize-${n}_altform-${form}.png`;
      await writeFile(join(OUT_DIR, name), png);
      count += 1;
    }
  }

  console.log(`[msix-icons] src-tauri/icons/msix/: ${count} PNGs`);
  await rm(WORKER, { recursive: true, force: true });
  console.log('[msix-icons] done — commit the output.');
}

main().catch(async (err) => {
  console.error(`[msix-icons] failed: ${err.message}`);
  await rm(WORKER, { recursive: true, force: true }).catch(() => {});
  process.exit(1);
});
