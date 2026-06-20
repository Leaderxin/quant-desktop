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

export type PeriodType = 'minute' | 'daily' | 'weekly' | 'monthly';

export interface UpdateInfo {
  current_version: string;
  latest_version: string;
  release_date: string;
  notes: string;
  release_url: string;
  download_size: number | null;
}
