// src/stores/quote.ts
import { defineStore } from 'pinia';
import { ref } from 'vue';
import type { Quote, IndexQuote } from '@/types';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export const useQuoteStore = defineStore('quote', () => {
  const quotes = ref<Map<string, Quote>>(new Map());
  const indices = ref<IndexQuote[]>([]);
  const lastUpdate = ref<number>(0);

  let unlistenQuotes: UnlistenFn | null = null;
  let unlistenIndices: UnlistenFn | null = null;

  async function startListening() {
    unlistenQuotes = await listen<Quote[]>('quotes-updated', (event) => {
      const map = new Map<string, Quote>();
      for (const q of event.payload) {
        map.set(`${q.market}:${q.code}`, q);
      }
      quotes.value = map;
      lastUpdate.value = Date.now();
    });

    unlistenIndices = await listen<IndexQuote[]>('indices-updated', (event) => {
      indices.value = event.payload;
    });
  }

  function stopListening() {
    unlistenQuotes?.();
    unlistenIndices?.();
  }

  function getQuote(code: string, market = 'CN'): Quote | undefined {
    return quotes.value.get(`${market}:${code}`);
  }

  return { quotes, indices, lastUpdate, startListening, stopListening, getQuote };
});
