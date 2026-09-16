<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
const counts = ref<[number, number, number] | null>(null);
const stale = ref(false);
const updated = ref('');
let stopped = false;
let timer: ReturnType<typeof setTimeout> | undefined;
async function refresh() {
  let interval = 60;
  try {
    const data = await invoke<[number, number, number]>('get_ticker_breadth');
    if (stopped) return;
    if (!data.some(n => n > 0)) throw new Error('暂无涨跌数据');
    counts.value = data;
    stale.value = false;
    updated.value = new Date().toLocaleTimeString();
  } catch {
    if (!stopped) stale.value = true;
  }
  try { interval = await invoke<number>('get_overview_interval'); } catch { /* Retry at the default interval. */ }
  if (!stopped) timer = setTimeout(refresh, Math.max(1, interval) * 1000);
}
onMounted(() => { void refresh(); });
onUnmounted(() => { stopped = true; clearTimeout(timer); });
</script>

<template>
  <div class="market-breadth tabular-nums" :class="{ stale }"
    :title="`${stale ? (counts ? '更新失败，当前为旧数据。' : '暂无数据。') : ''}${updated ? `更新时间 ${updated}` : '等待更新'}`">
    <span class="up">涨 {{ counts?.[0] ?? '--' }}</span>
    <span class="flat">平 {{ counts?.[2] ?? '--' }}</span>
    <span class="down">跌 {{ counts?.[1] ?? '--' }}</span>
    <span v-if="stale" class="status">{{ counts ? '旧' : '!' }}</span>
  </div>
</template>

<style scoped>
.market-breadth { position: relative; display: flex; align-items: center; justify-content: space-between; flex-shrink: 0; height: 15px; box-sizing: border-box; padding-right: 9px; font-size: 10px; white-space: nowrap; box-shadow: inset 0 -1px 0 var(--color-border-0); }
.flat, .status { color: var(--color-text-tertiary); }
.up { color: var(--color-up); }
.down { color: var(--color-down); }
.status { position: absolute; right: 0; font-size: 8px; }
.stale { opacity: .7; }
</style>
