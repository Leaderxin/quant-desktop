// src/utils/changelog.ts
//
// 解析仓库根目录的 CHANGELOG.md（Keep a Changelog 风格），供「关于」页展示
// 最近几个版本的更新说明。文件在构建时以 ?raw 内联进产物，离线可用。

export interface ChangelogSection {
  /** 小节标题，如 Added / Changed / Fixed。 */
  title: string;
  items: string[];
}

export interface ChangelogEntry {
  /** 版本号，不含 v 前缀，如 1.5.1。 */
  version: string;
  /** 发布日期，如 2026-09-20；版本头没写则为空串。 */
  date: string;
  sections: ChangelogSection[];
}

const VERSION_RE = /^##\s+v?(\d+\.\d+\.\d+)\s*(?:\(([^)]*)\))?/;
const SECTION_RE = /^###\s+(.+)/;
const ITEM_RE = /^[-*]\s+(.+)/;

/**
 * 解析多版本更新日志。解析不出的行一律跳过 —— 这份文件打包进了安装包，
 * 格式手改坏了也不能让「关于」页挂掉（与 utils/prefs.ts 同一原则）。
 */
export function parseChangelog(raw: string): ChangelogEntry[] {
  const entries: ChangelogEntry[] = [];
  let entry: ChangelogEntry | null = null;
  let section: ChangelogSection | null = null;

  for (const line of raw.split(/\r?\n/)) {
    const version = line.match(VERSION_RE);
    if (version) {
      entry = { version: version[1], date: version[2] ?? '', sections: [] };
      entries.push(entry);
      section = null;
      continue;
    }
    // 第一个版本头之前的行（文件标题、说明文字）不属于任何版本
    if (!entry) continue;

    const heading = line.match(SECTION_RE);
    if (heading) {
      section = { title: heading[1].trim(), items: [] };
      entry.sections.push(section);
      continue;
    }

    const item = line.match(ITEM_RE);
    if (item && section) section.items.push(item[1].trim());
  }

  return entries;
}
