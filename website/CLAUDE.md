# website/ — 官网

独立静态站，发布在 <https://leaderxin.github.io/quant-desktop/>，由 [../.github/workflows/pages.yml](../.github/workflows/pages.yml) 部署。**与 Tauri 应用没有代码关系**：不在 Vite 入口里、不参与 `npm run build`、不依赖任何 npm 包。`index.html` / `styles.css` / `app.js` + 内容页就是全部，改完刷新即见。应用内唯一入口是状态栏的 `SITE_URL`。

## 命令

```bash
npm run site:assets   # 资源复制 + 引用校验 + 顶栏一致性校验 + sitemap；CI 跑同一条
npm run site:images   # 仅换截图时手动跑；依赖本机 Chrome（CHROME_PATH 可覆盖）
```

改完任何页面都必须跑 `site:assets`。

## 素材与配色

- 截图住在仓库根 `screenshots/`（README 也用），`website/assets/` 由 `site-assets.mjs` 复制生成并 gitignore。**不要放回 `public/`**：`public/` 会整体进 `dist/`，而 `tauri.conf.json` 的 `frontendDist` 指向 `dist`，截图会被打进安装包（当时 1.9MB，应用一处没引用）。挪位置必须在**同一提交**里改 README —— github.com 按仓库相对路径渲染。
- 配色与 [../src/assets/styles/variables.css](../src/assets/styles/variables.css) 同源，且是仓库里唯一一处有意重复（站点没有构建步骤）。改应用的 surface / text / accent token 时要同步这里；`--page-*` / `--glow-*` 等氛围 token 是站点自己的。
- 主题契约与应用一致：`<html data-theme>`，`<head>` 内联脚本在首帧前定好，`app.js` 只做增强（禁用 JS 页面仍完整可读，`.reveal` 由 `.js` 类门控），没有网络请求。
- **不引 webfont**（`fonts.googleapis.com` 在国内基本加载不出）；科技感靠等宽栈。
- `--text-3` 只许用在装饰上；正文级灰用 `--text-2`。主按钮标签色跟主题（`--on-accent`），浅色主题下 hover 要变深。

## 图片管线（`scripts/optimize-images.mjs`）

2057px 源图 → `<picture>` + WebP 三档（600 / 1100 / 2057），外加 favicon 32/180、品牌图标 128、1200×630 社交卡。产物**提交进仓库**，CI 不需要浏览器。三条坑：

- `canvas.toDataURL` 在 `file://` 下会因 canvas taint 抛 SecurityError —— Chrome 必须加 `--allow-file-access-from-files`。
- **目标宽 > 源宽时必须整个跳过**（文件名永远等于真实宽度）：曾产出 1298px 却叫 `-2057.webp`，HTML 写 `2057w`，浏览器会算出错误的密度。源图比所有目标都小的（行情条 344×59）直接出 PNG。
- `background-clip: text` 会静默吃掉溢出元素框的字形；配 `width: max-content`。

另外：`<picture>` 需 `display: contents`（否则成为 grid/flex 子项，算错列宽与 `place-items`）；`<img>` 的 `height` 属性是定值，CSS 必须配 `height: auto`。`ticker-dark/light.png` 只有 344×59，必须原尺寸展示（`.ticker-runway` 为此存在，`aspect-ratio: auto`）。

## 内容页与顶栏一致性

- 7 个页面：`download` / `faq` / `changelog`（生成）/ `guide/`（索引 + 三篇）。阅读版式用 `.page-head` / `.prose`（680px 正文列）/ `.card-grid` / `.release*`；面包屑只在 `guide/*`。
- 顶栏、页脚、回到顶部**每页各一份**，由 `site-assets.mjs` 用 `<!-- chrome:nav:start -->` 等标记提取后**逐字节比对**（比对前剥掉 `../`）。`404.html` / `privacy.html` 用 `CHROME_SKIP`。
- 当前页高亮不能写进顶栏（写了就不一致），挂 `<body data-page>` + CSS，用 `href$=` 后缀匹配以避开 `../`；新增页面要给 `data-page` 并登记。
- `changelog.html` 由 `CHANGELOG.md` 生成；`sitemap.xml` 由脚本递归扫描 `website/**/*.html` 生成（排除 `SITEMAP_SKIP`，`lastmod` 取 `git log`）。引用校验按**页面自身目录**解析相对路径。

## SEO / GEO

- `<head>` 里的 `application/ld+json` 是一个 `@graph`：`SoftwareApplication` / `FAQPage` / `WebSite`。字段写事实；FAQ 答案必须与页面上看得见的正文逐字一致；**不写 `softwareVersion`**（没有静态版本号）。
- `og:image` 用 `main-light.png`（缩略图下深色截图看不清），并声明 `og:image:width/height`。
- `robots.txt` 与 `llms.txt` **不在本仓库**，唯一一份在 [Leaderxin/leaderxin.github.io](https://github.com/Leaderxin/leaderxin.github.io)（协议规定爬虫只读域名根）。
- `privacy.html` 的唯一来源是 [../docs/privacy.html](../docs/privacy.html)，由脚本复制到站点根；该地址是 Microsoft Store 登记过的，删不得。
- 改文案时同时看：`<title>`、`meta description`、OG/Twitter、JSON-LD 的 `description`/`featureList`、以及另一个仓库的 `llms.txt`。

## Hero 轮播与回到顶部

- 五张截图，首张是日间主题；**不自动播放**。
- 左右按钮常显（触摸设备没有 hover），跟主题走；非当前页设 `aria-hidden`。
- 方向键只在焦点进入 `.carousel` 时接管，否则会劫持页面滚动；触摸滑动不调 `preventDefault`。
- 首尾各挂一张克隆页，`transitionend` 后加 `.is-snap` 瞬移到真图 —— 否则「末张→首张」会横穿三张。
- 回到顶部是 `<a href="#top">`（禁用 JS 也能用，且不丢焦点），外圈进度环的分母 `scrollHeight` 只在 load/resize 量一次 —— 这依赖每张图都预留了宽高比。

## 部署

`pages.yml` 的 `paths` 含 `screenshots/**` 与 `docs/privacy.html`：换截图、改隐私政策同样要重新发布。**部署成功但内容没变**时先看 `gh api repos/{owner}/{repo}/pages/builds/latest` 的 commit；`build_type` 必须是 `workflow`，历史 `legacy` 构建会盖掉 Actions 的部署。
