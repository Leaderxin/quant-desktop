// src/utils/prefs.ts — 设置项的取值域、默认值与解析工具
//
// 设置以字符串存在 SQLite 的 `settings` 表里（key-value），复杂结构走 JSON。
// 「字符串 → 结构化值」的解析集中在这里，避免每个组件各写一份 JSON.parse
// 并在数据损坏时抛出未捕获异常。

/** 自选表可配置的列。顺序即设置页里的展示顺序，也是「全部显示」时的表格列序。 */
export const ALL_COLUMNS = [
  { key: 'code', label: '代码', required: true },
  { key: 'name', label: '名称', required: true },
  { key: 'price', label: '最新价' },
  { key: 'change_pct', label: '涨跌幅' },
  { key: 'change', label: '涨跌额' },
  { key: 'volume', label: '成交量' },
  { key: 'turnover', label: '成交额' },
  { key: 'turnover_rate', label: '换手率' },
] as const;

export type ColumnKey = (typeof ALL_COLUMNS)[number]['key'];

export const REQUIRED_COLUMNS: ColumnKey[] = ALL_COLUMNS
  .filter((c) => 'required' in c && c.required)
  .map((c) => c.key);

export function columnLabel(key: ColumnKey): string {
  return ALL_COLUMNS.find((c) => c.key === key)?.label ?? key;
}

/** 表格默认排序。「不排序」是合法状态（null），此时按自选自身顺序排列。 */
export interface DefaultSort {
  key: ColumnKey;
  order: 'ascend' | 'descend';
}

/** 市场概览板块榜单条数的合法区间，与后端 `market::SECTOR_TOP_N_MIN/MAX` 一致。 */
export const SECTOR_TOP_N_MIN = 1;
export const SECTOR_TOP_N_MAX = 50;

/** 分组名长度上限，与后端 `db::GROUP_NAME_MAX_LEN` 一致。 */
export const GROUP_NAME_MAX_LEN = 20;

export const TICKER_ITEMS_MIN = 1;
export const TICKER_ITEMS_MAX = 4;

export function clampTopN(n: number): number {
  if (!Number.isFinite(n)) return SECTOR_TOP_N_MIN;
  return Math.min(Math.max(Math.round(n), SECTOR_TOP_N_MIN), SECTOR_TOP_N_MAX);
}

export function clampTickerItems(n: number): number {
  if (!Number.isFinite(n)) return 1;
  return Math.min(Math.max(Math.round(n), TICKER_ITEMS_MIN), TICKER_ITEMS_MAX);
}

/**
 * 解析设置里的 JSON 数组。任何异常（缺键、非法 JSON、类型不符）都退回 `fallback`
 * 而不是抛出 —— 设置值可能被手工改坏或来自旧版本，一个坏键不该让整个界面白屏。
 */
export function parseJsonArray<T>(raw: string | undefined, fallback: T[]): T[] {
  if (!raw) return fallback;
  try {
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? (parsed as T[]) : fallback;
  } catch {
    return fallback;
  }
}

/**
 * 列配置：过滤掉未知列、去重，并在必选列被坏配置抹掉时补回队首。
 *
 * 除此之外完全尊重用户顺序 —— 必选列（代码/名称）不可**隐藏**，但可以拖动，
 * 把「名称」排到「代码」前面是合法的。
 */
export function parseColumns(raw: string | undefined): ColumnKey[] {
  const known = new Set<string>(ALL_COLUMNS.map((c) => c.key));
  const parsed = parseJsonArray<string>(raw, [])
    .filter((k): k is ColumnKey => known.has(k));
  const deduped = Array.from(new Set(parsed));
  if (deduped.length === 0) return ALL_COLUMNS.map((c) => c.key);
  const missing = REQUIRED_COLUMNS.filter((k) => !deduped.includes(k));
  return [...missing, ...deduped];
}

/** 默认排序配置。空串/非法值/未知列都表示「不排序」。 */
export function parseDefaultSort(raw: string | undefined): DefaultSort | null {
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw) as Partial<DefaultSort> | null;
    if (!parsed || typeof parsed.key !== 'string') return null;
    const known = ALL_COLUMNS.some((c) => c.key === parsed.key);
    if (!known) return null;
    const order = parsed.order === 'ascend' ? 'ascend' : 'descend';
    return { key: parsed.key as ColumnKey, order };
  } catch {
    return null;
  }
}

/** 布尔型设置：约定 "1" 为开、"0" 为关。缺键时取 `fallback`。 */
export function parseBool(raw: string | undefined, fallback: boolean): boolean {
  if (raw === '1') return true;
  if (raw === '0') return false;
  return fallback;
}

export function parseCount(raw: string | undefined, fallback: number, clamp: (n: number) => number): number {
  const n = Number.parseInt(raw ?? '', 10);
  return Number.isNaN(n) ? clamp(fallback) : clamp(n);
}
