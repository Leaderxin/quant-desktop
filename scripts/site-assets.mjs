// Assemble the static website's assets, then verify every local reference in
// website/index.html actually resolves.
//
// Why assemble at all: website/ is published to GitHub Pages as a self-contained
// directory, so its images must live inside it. Those images are already tracked
// under screenshots/ (the README uses them), and a second committed copy
// would drift the moment either side is updated. So they are copied here into
// website/assets/ — a build product, gitignored like dist/.
//
// Why verify: a mistyped screenshot name is invisible locally (the browser just
// shows a broken image) and invisible in review. Here it fails the build.
//
// Node alone, no dependencies: this runs both locally (`npm run site:assets`
// before opening website/index.html) and in .github/workflows/pages.yml.

import { cp, mkdir, readdir, readFile, rm, writeFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
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

// Copied to the *site root*, not into assets/. GitHub Pages was previously
// serving master:/docs, so https://leaderxin.github.io/quant-desktop/privacy.html
// is already the privacy-policy URL registered with the Microsoft Store (the
// page was added in 183bba7 for exactly that). Publishing website/ as the new
// site root would 404 it. Same rule as the screenshots: docs/privacy.html stays
// the single source and this is a copy, so the two can never disagree.
const ROOT_FILES = [['docs/privacy.html', 'privacy.html']];

const SCREENSHOT_SRC = 'screenshots';
const ENTRY = 'index.html';
const SITE_URL = 'https://leaderxin.github.io/quant-desktop/';

// Pages that must not appear in the sitemap. 404.html is an error page, and
// privacy.html carries `noindex` — Search Console reports a noindex URL that is
// also listed in a sitemap as an error, so the two are mutually exclusive by
// design, not by oversight.
const SITEMAP_SKIP = new Set(['404.html', 'privacy.html']);

async function assemble(missing) {
  // Rebuild from scratch: a screenshot deleted upstream must not linger here
  // and get published as a ghost image.
  await rm(outDir, { recursive: true, force: true });
  await mkdir(outDir, { recursive: true });

  // The root-level copies sit outside assets/, so the rm above misses them —
  // clear them explicitly or deleting docs/privacy.html upstream would leave a
  // stale copy published at the site root.
  for (const [, dest] of ROOT_FILES) {
    await rm(resolve(siteDir, dest), { force: true });
  }

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

  for (const [src, dest] of ROOT_FILES) {
    const from = resolve(root, src);
    if (!existsSync(from)) {
      missing.push(src);
      continue;
    }
    await cp(from, join(siteDir, dest));
    copied += 1;
  }

  return copied;
}

// Every src/href/srcset in every published page that points at something in
// this repo must exist. Absolute URLs (http:, //, /path, mailto:, data:) and
// pure fragments are none of our business — they can rot without breaking the
// build. The `/` case matters: 404.html has to use site-absolute paths, because
// a project site serves it for missing paths at *any* depth, where a relative
// link would resolve against the broken URL instead of the site root.
function isLocalRef(ref) {
  return !/^([a-z][a-z0-9+.-]*:|\/|#)/i.test(ref);
}

async function publishedPages() {
  return (await readdir(siteDir)).filter((f) => f.endsWith('.html')).sort();
}

async function verifyReferences(pages) {
  const broken = [];

  for (const page of pages) {
    const html = await readFile(resolve(siteDir, page), 'utf8');
    const seen = new Set();

    const check = (ref) => {
      const clean = ref.split(/[?#]/)[0];
      if (!clean || !isLocalRef(clean) || seen.has(clean)) return;
      seen.add(clean);
      if (!existsSync(resolve(siteDir, clean))) broken.push(`${page} → ${clean}`);
    };

    for (const [, ref] of html.matchAll(/(?:src|href)="([^"]+)"/g)) check(ref);

    // srcset is a comma-separated list of "url descriptor" pairs. A stale entry
    // here is exactly as invisible as a stale src — the browser just falls back
    // to a smaller candidate — so it is worth catching.
    for (const [, list] of html.matchAll(/srcset="([^"]+)"/g)) {
      for (const part of list.split(',')) {
        const url = part.trim().split(/\s+/)[0];
        if (url) check(url);
      }
    }
  }

  return broken;
}

// lastmod comes from git, not from "now": a date that changes on every deploy
// is noise to a crawler and teaches it to distrust the field.
function lastModified(page) {
  try {
    const out = execFileSync('git', ['log', '-1', '--format=%cs', '--', `website/${page}`], {
      cwd: root,
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'ignore'],
    });
    return out.trim() || null;
  } catch {
    return null; // not a git checkout, or a shallow clone with no history
  }
}

// Generated rather than hand-written: the moment a new page is added by hand,
// a hand-maintained sitemap starts drifting — and the failure is silent, since
// nothing links to the sitemap for a human to notice.
async function writeSitemap(pages) {
  const listed = pages.filter((p) => !SITEMAP_SKIP.has(p));
  const urls = listed.map((page) => {
    const loc = SITE_URL + (page === ENTRY ? '' : page);
    const stamp = lastModified(page);
    return `  <url>\n    <loc>${loc}</loc>${stamp ? `\n    <lastmod>${stamp}</lastmod>` : ''}\n  </url>`;
  });

  await writeFile(
    resolve(siteDir, 'sitemap.xml'),
    '<?xml version="1.0" encoding="UTF-8"?>\n' +
      '<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n' +
      urls.join('\n') +
      '\n</urlset>\n'
  );

  return listed;
}

async function main() {
  const missing = [];
  const copied = await assemble(missing);

  if (missing.length > 0) {
    console.error(`[site] Missing source assets: ${missing.join(', ')}`);
    process.exit(1);
  }

  const pages = await publishedPages();

  const broken = await verifyReferences(pages);
  if (broken.length > 0) {
    console.error('[site] These references do not resolve:');
    for (const ref of broken) console.error(`  - ${ref}`);
    process.exit(1);
  }

  const listed = await writeSitemap(pages);

  console.log(`[site] Copied ${copied} files into website/assets/`);
  console.log(`[site] ${pages.length} page(s): every local reference resolves.`);
  console.log(`[site] sitemap.xml lists ${listed.length} URL(s).`);
  console.log('[site] Open website/index.html in a browser to preview.');
}

main().catch((err) => {
  console.error(`[site] Failed: ${err.message}`);
  process.exit(1);
});
