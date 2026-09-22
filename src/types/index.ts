// src/types/index.ts
export interface Quote {
  code: string;
  market: string;
  name: string;
  price: number;
  change: number;
  change_pct: number;
  open: number;
  high: number;
  low: number;
  volume: number;
  turnover: number;
  turnover_rate: number | null;
  timestamp: number;
}

export interface IndexQuote {
  code: string;
  name: string;
  price: number;
  change: number;
  change_pct: number;
  volume: number;
  turnover: number;
}

export interface StockBrief {
  code: string;
  market: string;
  name: string;
  category: string;
}

export interface SectorItem {
  code: string;
  name: string;
  change_pct: number;
  /** 板块内涨幅最高的成分股(涨幅榜用) */
  leader_name: string | null;
  leader_pct: number | null;
  /** 板块内跌幅最深的成分股(跌幅榜用) */
  laggard_name: string | null;
  laggard_pct: number | null;
}

export interface MarketOverview {
  turnover: number;
  up: number;
  down: number;
  flat: number;
  industry: SectorItem[];
  concept: SectorItem[];
}

export interface WatchItem {
  id: number;
  code: string;
  market: string;
  name: string;
  added_at: string;
  /** 是否参与行情条滚动播报。新增自选默认 true。 */
  ticker_enabled: boolean;
  /** 行情条轮播位次。与分组无关 —— 播报范围是跨分组的扁平列表。 */
  ticker_order: number;
}

export interface WatchGroup {
  id: number;
  name: string;
  sort_order: number;
  created_at: string;
  /** 组内成员的自选 id，按组内顺序。 */
  watch_ids: number[];
}

/**
 * 自选表的一次性全量状态。分组标签栏切换是纯本地过滤 —— 若分组与成员分两次
 * 拉取，切换时必然出现「分组已到、成员还没到」的中间态。
 *
 * 多归属模型：`items` 是全局股票池（一只股票只入池一次），`groups[].watch_ids`
 * 是 (分组, 股票) 多对多关联，同一只股票可以出现在多个分组里。
 */
export interface WatchlistSnapshot {
  items: WatchItem[];
  groups: WatchGroup[];
  /** 展示顺序最靠前的分组。新建自选在未指定分组时落进这里。 */
  default_group_id: number;
}

export interface Level {
  price: number;
  volume: number;
}

export interface Depth {
  code: string;
  bids: Level[];
  asks: Level[];
}

export interface MinuteData {
  time: string;
  price: number;
  open: number;
  high: number;
  low: number;
  volume: number;
  avg_price: number;
}

export interface KLineData {
  date: string;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
  turnover: number;
}

export type PeriodType =
  | 'minute'
  | '1min'
  | '5min'
  | '15min'
  | '30min'
  | '60min'
  | 'daily'
  | 'weekly'
  | 'monthly';

export type SubIndicatorType = 'VOL' | 'MACD';

/** 主图叠加指标 — 均线 / 布林通道，叠加在蜡烛 pane 上（区别于副图指标） */
export type MainOverlayType = 'MA' | 'BOLL';

export interface UpdateInfo {
  current_version: string;
  latest_version: string;
  release_date: string;
  notes: string;
  release_url: string;
  download_size: number | null;
}
