// src/utils/changelog.spec.ts
//
// 「关于」页的更新说明直接渲染解析结果 —— 版本号、日期、条目哪一项错了
// 用户都看得见，所以解析契约钉在这里。样例结构与仓库 CHANGELOG.md 保持
// 同构（## 版本头 + ### 小节 + - 条目）。
import { describe, expect, it } from 'vitest';
import { parseChangelog } from './changelog';

const SAMPLE = `# Changelog

一些文件级说明文字。

## v1.5.1 (2026-09-20)

### Added
- 行情条高度适配系统显示缩放
- 状态栏版权信息可点击跳转

### Fixed
- 修复日志初始化失败时崩溃

## 1.5.0 (2026-09-15)

### Added
- 应用上架 Microsoft Store

## v1.4.7 (2026-09-14)

### Added
- 自选列表新增「行情条播报」开关列
`;

describe('parseChangelog', () => {
  it('按版本头切分，提取版本号与日期', () => {
    const entries = parseChangelog(SAMPLE);
    expect(entries.map((e) => e.version)).toEqual(['1.5.1', '1.5.0', '1.4.7']);
    expect(entries[0].date).toBe('2026-09-20');
  });

  it('版本头可不带 v 前缀', () => {
    // 标题是手写的，历史上出现过无 v 的写法，两种都放行
    const entries = parseChangelog('## 1.5.0 (2026-09-15)\n\n### Added\n- x');
    expect(entries[0].version).toBe('1.5.0');
  });

  it('条目收进所属小节并保序', () => {
    const [first] = parseChangelog(SAMPLE);
    expect(first.sections.map((s) => s.title)).toEqual(['Added', 'Fixed']);
    expect(first.sections[0].items).toEqual([
      '行情条高度适配系统显示缩放',
      '状态栏版权信息可点击跳转',
    ]);
  });

  it('第一个版本头之前的行不归入任何版本', () => {
    // 「# Changelog」和说明文字既不产生条目，也不混进 v1.5.1
    const entries = parseChangelog(SAMPLE);
    expect(entries).toHaveLength(3);
    expect(entries[0].sections).toHaveLength(2);
  });

  it('版本头没写日期时得到空串，而不是 undefined', () => {
    const entries = parseChangelog('## v1.0.0\n\n### Added\n- x');
    expect(entries[0].date).toBe('');
  });

  it('小节之外的游离文字与缩进列表不收', () => {
    const entries = parseChangelog(
      '## v1.0.0 (2026-01-01)\n\n一段游离文字\n  - 缩进项\n\n### Added\n- 正常项',
    );
    expect(entries[0].sections[0].items).toEqual(['正常项']);
  });

  it('CRLF 行尾同样可解析', () => {
    // 仓库文件在 Windows 上检出时可能带 \r\n
    const entries = parseChangelog('## v1.0.0 (2026-01-01)\r\n\r\n### Added\r\n- 项\r\n');
    expect(entries[0].sections[0].items).toEqual(['项']);
  });

  it('空串与纯噪音输入返回空数组，不抛错', () => {
    expect(parseChangelog('')).toEqual([]);
    expect(parseChangelog('随便什么\n都不是版本头')).toEqual([]);
  });
});
