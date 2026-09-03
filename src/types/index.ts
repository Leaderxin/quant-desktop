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
  leader_name: string | null;
  leader_pct: number | null;
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
  sort_order: number;
  added_at: string;
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
