<script setup lang="ts">
import { ref, onMounted, watch, computed } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { Depth, Level } from '@/types';

const props = defineProps<{
  code: string;
  market: string;
}>();

const depth = ref<Depth | null>(null);
const loading = ref(false);
const error = ref('');

async function fetchDepth() {
  loading.value = true;
  error.value = '';
  try {
    depth.value = await invoke<Depth>('get_depth', { code: props.code, market: props.market });
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
}

onMounted(() => fetchDepth());
watch(() => props.code, () => fetchDepth());

// Pad to 5 levels
const levels = computed(() => {
  const bids: (Level | null)[] = depth.value
    ? Array.from({ length: 5 }, (_, i) => depth.value!.bids[i] ?? null)
    : Array.from({ length: 5 }, () => null);
  const asks: (Level | null)[] = depth.value
    ? Array.from({ length: 5 }, (_, i) => depth.value!.asks[i] ?? null)
    : Array.from({ length: 5 }, () => null);

  // Max volume across all levels for bar scaling
  const allVols = [...bids, ...asks]
    .filter((l): l is Level => l !== null)
    .map(l => l.volume);
  const maxVol = Math.max(...allVols, 1);

  return { bids, asks, maxVol };
});

function formatVol(v: number): string {
  if (v >= 1000000) return (v / 1000000).toFixed(2) + '万手';
  if (v >= 10000) return (v / 10000).toFixed(1) + '万手';
  if (v >= 100) return (v / 100).toFixed(0) + '手';
  return v.toString();
}

function barWidth(vol: number, max: number): string {
  return `${Math.max((vol / max) * 100, 2)}%`;
}
</script>

<template>
  <div class="depth-panel">
    <div class="depth-title">五档盘口</div>

    <div v-if="loading" class="depth-placeholder">加载中...</div>
    <div v-else-if="error" class="depth-placeholder error">{{ error }}</div>

    <div v-else class="depth-body">
      <!-- Bids (buy side) -->
      <div class="depth-side bids">
        <div class="depth-side-header">
          <span>买价</span>
          <span>买量</span>
        </div>
        <div
          v-for="(level, i) in levels.bids"
          :key="'b' + i"
          class="depth-row bid-row"
        >
          <span class="depth-bar-wrap">
            <span
              class="depth-bar bid-bar"
              :style="{ width: level ? barWidth(level.volume, levels.maxVol) : '0' }"
            ></span>
          </span>
          <span class="depth-price bid-price">{{ level?.price?.toFixed(2) ?? '--' }}</span>
          <span class="depth-vol">{{ level ? formatVol(level.volume) : '--' }}</span>
        </div>
      </div>

      <!-- Asks (sell side) -->
      <div class="depth-side asks">
        <div class="depth-side-header">
          <span>卖价</span>
          <span>卖量</span>
        </div>
        <div
          v-for="(level, i) in levels.asks"
          :key="'a' + i"
          class="depth-row ask-row"
        >
          <span class="depth-bar-wrap">
            <span
              class="depth-bar ask-bar"
              :style="{ width: level ? barWidth(level.volume, levels.maxVol) : '0' }"
            ></span>
          </span>
          <span class="depth-price ask-price">{{ level?.price?.toFixed(2) ?? '--' }}</span>
          <span class="depth-vol">{{ level ? formatVol(level.volume) : '--' }}</span>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.depth-panel {
  min-width: 280px;
  padding: 10px var(--space-3);
  background: var(--color-surface-1);
  border: 1px solid var(--color-border-0);
  border-radius: var(--radius-md);
}

.depth-title {
  font-size: var(--text-sm);
  font-weight: var(--font-weight-semibold);
  margin-bottom: var(--space-2);
  color: var(--color-text-primary);
}

.depth-placeholder {
  font-size: var(--text-xs);
  color: var(--color-text-tertiary);
  padding: var(--space-3) 0;
  text-align: center;
}
.depth-placeholder.error { color: var(--color-up); }

.depth-body {
  display: flex;
  gap: var(--space-4);
}

.depth-side {
  flex: 1;
  min-width: 0;
}

.depth-side-header {
  display: flex;
  justify-content: space-between;
  font-size: var(--text-xs);
  color: var(--color-text-tertiary);
  padding: 2px 0 4px;
  border-bottom: 1px solid var(--color-border-0);
  margin-bottom: 2px;
}

.depth-row {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 22px;
  padding: 0 4px;
  font-size: var(--text-xs);
  font-family: var(--font-mono);
  font-variant-numeric: tabular-nums;
  border-radius: 2px;
}

.depth-bar-wrap {
  position: absolute;
  left: 0;
  top: 0;
  height: 100%;
  width: 100%;
  z-index: 0;
}
.depth-bar {
  position: absolute;
  right: 0;
  top: 2px;
  bottom: 2px;
  border-radius: 2px;
  opacity: 0.12;
  transition: width var(--transition-fast);
}
.bid-bar { background: var(--color-down); }
.ask-bar { background: var(--color-up); }

.depth-price {
  position: relative;
  z-index: 1;
  font-weight: var(--font-weight-medium);
}
.depth-vol {
  position: relative;
  z-index: 1;
  color: var(--color-text-secondary);
}

.bid-price { color: var(--color-down); }
.ask-price { color: var(--color-up); }
</style>
