// src/stores/quote.ts
import { defineStore } from 'pinia';
import { ref } from 'vue';
import type { Quote, IndexQuote } from '@/types';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export const useQuoteStore = defineStore('quote', () => {
  const quotes = ref<Map<string, Quote>>(new Map());
  const indices = ref<IndexQuote[]>([]);
  const lastUpdate = ref<number>(0);
  const error = ref<string | null>(null);

  let unlistenQuotes: UnlistenFn | null = null;
  let unlistenIndices: UnlistenFn | null = null;

  async function startListening() {
    try {
      unlistenQuotes = await listen<Quote[]>('quotes-updated', (event) => {
        const map = new Map<string, Quote>();
        for (const q of event.payload) {
          map.set(`${q.market}:${q.code}`, q);
        }
        quotes.value = map;
        lastUpdate.value = Date.now();
      });

      unlistenIndices = await listen<IndexQuote[]>('indices-updated', (event) => {
        const next = event.payload;
        const prev = indices.value;
        // Skip update if data is identical (avoids unnecessary Vue re-renders)
        if (prev.length === next.length && next.every((v, i) =>
          v.code === prev[i].code &&
          v.price === prev[i].price &&
          v.change === prev[i].change &&
          v.change_pct === prev[i].change_pct &&
          v.volume === prev[i].volume &&
          v.turnover === prev[i].turnover
        )) {
          return;
        }
        indices.value = next;
      });

      error.value = null;
    } catch (e) {
      error.value = `行情监听启动失败: ${e}`;
      console.error('[quote store] Failed to start listeners:', e);
    }
  }

  function stopListening() {
    unlistenQuotes?.();
    unlistenIndices?.();
  }

  function getQuote(code: string, market = 'CN'): Quote | undefined {
    return quotes.value.get(`${market}:${code}`);
  }

  return { quotes, indices, lastUpdate, error, startListening, stopListening, getQuote };
});
