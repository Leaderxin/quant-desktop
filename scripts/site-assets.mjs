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

// 递归收集**所有**页面，含子目录。只扫顶层的话 guide/* 会被静默漏掉 ——
// 不校验、不进 sitemap，而这两件事都不会有人注意到。
async function publishedPages() {
  const found = [];

  async function walk(dir, prefix) {
    for (const entry of await readdir(dir, { withFileTypes: true })) {
      if (entry.name.startsWith('.')) continue;
      if (entry.isDirectory()) {
        if (entry.name === 'assets') continue; // 产物，不是页面
        await walk(join(dir, entry.name), `${prefix}${entry.name}/`);
      } else if (entry.name.endsWith('.html')) {
        found.push(prefix + entry.name);
      }
    }
  }

  await walk(siteDir, '');
  return found.sort();
}

async function verifyReferences(pages) {
  const broken = [];

  for (const page of pages) {
    const html = await readFile(resolve(siteDir, page), 'utf8');
    // 相对引用要按**页面自己所在的目录**解析，不是站点根 —— guide/ 下的
    // ../styles.css 指的是 website/styles.css，按站点根解会解到仓库外面去。
    const baseDir = dirname(resolve(siteDir, page));
    const seen = new Set();

    const check = (ref) => {
      const clean = ref.split(/[?#]/)[0];
      if (!clean || !isLocalRef(clean) || seen.has(clean)) return;
      seen.add(clean);

      const target = resolve(baseDir, clean);
      if (!target.startsWith(siteDir)) {
        // 跳出站点目录的引用在本地能跑（file:// 尚可），发布后一定 404
        broken.push(`${page} → ${clean}（跳出了站点目录）`);
        return;
      }
      if (!existsSync(target)) broken.push(`${page} → ${clean}`);
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
    // 目录索引的规范地址是目录本身：guide/index.html → /guide/
    const slug = page === ENTRY ? '' : page.replace(/index\.html$/, '');
    const loc = SITE_URL + slug;
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

// ── 顶栏 / 页脚 / 回到顶部的一致性 ──────────────────────────────
//
// 这三段被复制进每一个页面，好让每页都是「打开就能看」的独立文件（站点没有
// 构建步骤是它的一条底线）。这种复制正是本仓库平时拒绝的东西 —— 让它可以
// 接受的是下面这个校验：所有页面里这三段必须逐字节相同，不一致就让 CI 失败。
//
// 比对前剥掉 `../`：子目录页面确实需要它，而目录深度不是漂移。
const CHROME_BLOCKS = ['nav', 'footer', 'totop'];

// Two pages are deliberately standalone and carry no chrome:
//   404.html     —— 项目站会对任意深度的失效路径返回它，所有链接只能是
//                   站点绝对路径，套上顶栏反而会把相对链接带坏
//   privacy.html —— 从 docs/ 复制来的隐私政策，商店登记在册，保持原样
const CHROME_SKIP = new Set(['404.html', 'privacy.html']);

const normalizeChrome = (block) => block.replace(/\.\.\//g, '').trim();

async function verifyChrome(pages) {
  const problems = [];
  let checked = 0;

  for (const name of CHROME_BLOCKS) {
    const re = new RegExp(
      `<!-- chrome:${name}:start -->([\\s\\S]*?)<!-- chrome:${name}:end -->`
    );
    let reference = null;
    let referencePage = null;

    for (const page of pages) {
      if (CHROME_SKIP.has(page)) continue;
      const html = await readFile(resolve(siteDir, page), 'utf8');
      const m = re.exec(html);
      if (!m) {
        problems.push(`${page}: 缺少 chrome:${name} 块`);
        continue;
      }
      checked += 1;
      const block = normalizeChrome(m[1]);
      if (reference === null) {
        reference = block;
        referencePage = page;
      } else if (block !== reference) {
        problems.push(`chrome:${name}：${page} 与 ${referencePage} 不一致`);
      }
    }
  }

  return { problems, checked };
}

// 返回整块（含标记本身）—— 生成出来的页面必须连标记一起带上，
// 否则它自己就通不过上面的一致性校验。
async function readChrome(name) {
  const html = await readFile(resolve(siteDir, ENTRY), 'utf8');
  const m = new RegExp(
    `<!-- chrome:${name}:start -->[\\s\\S]*?<!-- chrome:${name}:end -->`
  ).exec(html);
  if (!m) throw new Error(`chrome:${name} block not found in ${ENTRY}`);
  return m[0];
}

// ── 更新日志页 ──────────────────────────────────────────────────
//
// 从 CHANGELOG.md 生成而不是手抄一份：手抄的那份在下一次发版就过期了，而且
// 过期是静默的 —— 只有人主动去比对才会发现。这是 site-assets 里唯一「凭内容
// 造页面」的部分，所以范围压到最小：只认 CHANGELOG.md 实际用到的那几种写法
// （`##` 版本、`###` 小节、`-` 条目、行内 **粗体** 与 `代码`），遇到别的一律
// 原样当段落输出，不抛错。
const escapeHtml = (s) =>
  s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');

const inline = (s) =>
  escapeHtml(s)
    .replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
    .replace(/`([^`]+)`/g, '<code>$1</code>')
    .replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<a href="$2">$1</a>');

function renderChangelog(md) {
  const out = [];
  let list = null;
  let inRelease = false;
  let inSection = false;

  const closeList = () => {
    if (!list) return;
    out.push(`<ul>${list.join('')}</ul>`);
    list = null;
  };
  const closeSection = () => {
    if (!inSection) return;
    out.push('  </div>');
    inSection = false;
  };
  const closeRelease = () => {
    closeSection();
    if (!inRelease) return;
    out.push('</section>');
    inRelease = false;
  };

  for (const raw of md.split(/\r?\n/)) {
    const line = raw.trimEnd();

    // 文件顶部那个 `# Changelog`，页面自己有 h1，跳过
    if (/^#\s+/.test(line)) continue;

    const head = /^##\s+(.+)$/.exec(line);
    if (head) {
      closeList();
      closeRelease();
      // "v1.6.1 (2026-10-09)" → 版本与日期分开排，日期用等宽数字
      const m = /^(v?[\d.]+)\s*\((\d{4}-\d{2}-\d{2})\)$/.exec(head[1].trim());
      const version = m ? m[1] : head[1].trim();
      const date = m ? m[2] : '';
      out.push(
        '<section class="release">',
        '  <div class="release__head">',
        `    <h2 class="release__version">${escapeHtml(version)}</h2>`,
        ...(date ? [`    <span class="release__date">${date}</span>`] : []),
        '  </div>'
      );
      inRelease = true;
      continue;
    }

    const kind = /^###\s+(.+)$/.exec(line);
    if (kind) {
      closeList();
      closeSection();
      out.push(
        '  <div class="release__section">',
        `    <h3 class="release__kind">${escapeHtml(kind[1].trim())}</h3>`
      );
      inSection = true;
      continue;
    }

    const item = /^\s*[-*]\s+(.+)$/.exec(line);
    if (item) {
      list = list || [];
      list.push(`<li>${inline(item[1])}</li>`);
      continue;
    }

    if (!line.trim()) {
      closeList();
      continue;
    }

    // 兜底：认不出的行原样当段落，不猜也不抛错
    closeList();
    out.push(`  <p>${inline(line)}</p>`);
  }

  closeList();
  closeRelease();
  return out.join('\n');
}

function pageShell({ title, description, path, bodyAttrs, chrome, content }) {
  const url = SITE_URL + path;
  return `<!DOCTYPE html>
<html lang="zh-CN" data-theme="dark">
<head>
<meta charset="UTF-8" />
<meta name="viewport" content="width=device-width, initial-scale=1.0" />
<title>${title}</title>
<meta name="description" content="${description}" />
<meta name="robots" content="index, follow, max-image-preview:large" />
<meta name="theme-color" content="#0d1117" />
<link rel="canonical" href="${url}" />
<link rel="icon" type="image/png" sizes="32x32" href="favicon-32.png" />
<link rel="apple-touch-icon" sizes="180x180" href="favicon-180.png" />

<meta property="og:type" content="website" />
<meta property="og:site_name" content="QuantDesktop" />
<meta property="og:locale" content="zh_CN" />
<meta property="og:title" content="${title}" />
<meta property="og:description" content="${description}" />
<meta property="og:url" content="${url}" />
<meta property="og:image" content="${SITE_URL}og-cover.png" />
<meta property="og:image:width" content="1200" />
<meta property="og:image:height" content="630" />
<meta property="og:image:alt" content="QuantDesktop —— 免费的 A 股桌面看盘工具，安装包不到 5 MB" />
<meta name="twitter:card" content="summary_large_image" />
<meta name="twitter:title" content="${title}" />
<meta name="twitter:description" content="${description}" />
<meta name="twitter:image" content="${SITE_URL}og-cover.png" />

<link rel="stylesheet" href="styles.css" />
<script>
  // 与首页同一段：首帧前定好主题，否则深色访问者会先看到一帧白闪。
  (function () {
    var root = document.documentElement;
    root.classList.add('js');
    var saved = null;
    try { saved = localStorage.getItem('qd-theme'); } catch (e) { /* private mode */ }
    var prefersLight = window.matchMedia && window.matchMedia('(prefers-color-scheme: light)').matches;
    root.setAttribute('data-theme', saved || (prefersLight ? 'light' : 'dark'));
  })();
</script>
</head>

<body ${bodyAttrs}>

${chrome.nav}

<main id="top">
${content}
</main>

${chrome.footer}

${chrome.totop}

<script src="app.js" defer></script>
</body>
</html>
`;
}

async function writeChangelog(chrome) {
  const md = await readFile(resolve(root, 'CHANGELOG.md'), 'utf8');
  const releases = renderChangelog(md);

  const content = `  <header class="page-head container">
    <h1 class="page-head__title">更新日志</h1>
    <p class="page-head__lede">
      每个版本改了什么。完整历史也在
      <a href="https://github.com/Leaderxin/quant-desktop/blob/master/CHANGELOG.md">GitHub</a> 上。
    </p>
  </header>

  <div class="container" style="padding-bottom: clamp(56px, 7vw, 88px)">
    <div class="prose" style="max-width: 760px">
${releases}
    </div>
  </div>
`;

  await writeFile(
    resolve(siteDir, 'changelog.html'),
    pageShell({
      title: 'QuantDesktop 更新日志 —— 每个版本改了什么',
      description:
        'QuantDesktop 各版本的更新记录：新增功能、修复的问题与行为变更。全部版本均免费，可从 GitHub Releases 或 Microsoft Store 升级。',
      path: 'changelog.html',
      bodyAttrs: 'data-page="changelog"',
      chrome,
      content,
    })
  );
}

async function main() {
  const missing = [];
  const copied = await assemble(missing);

  if (missing.length > 0) {
    console.error(`[site] Missing source assets: ${missing.join(', ')}`);
    process.exit(1);
  }

  // 生成的页面要先落盘，后面的校验与 sitemap 才会把它算进去。
  const chrome = {
    nav: await readChrome('nav'),
    footer: await readChrome('footer'),
    totop: await readChrome('totop'),
  };
  await writeChangelog(chrome);

  const pages = await publishedPages();

  const broken = await verifyReferences(pages);
  if (broken.length > 0) {
    console.error('[site] These references do not resolve:');
    for (const ref of broken) console.error(`  - ${ref}`);
    process.exit(1);
  }

  const { problems, checked } = await verifyChrome(pages);
  if (problems.length > 0) {
    console.error('[site] chrome blocks differ between pages:');
    for (const p of problems) console.error(`  - ${p}`);
    process.exit(1);
  }

  const listed = await writeSitemap(pages);

  console.log(`[site] Copied ${copied} files into website/assets/`);
  console.log(`[site] ${pages.length} page(s): every local reference resolves.`);
  console.log(`[site] ${checked} chrome blocks match across pages.`);
  console.log(`[site] sitemap.xml lists ${listed.length} URL(s).`);
  console.log('[site] Open website/index.html in a browser to preview.');
}

main().catch((err) => {
  console.error(`[site] Failed: ${err.message}`);
  process.exit(1);
});
