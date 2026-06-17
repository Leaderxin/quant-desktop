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
