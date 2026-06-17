<script setup lang="ts">
import { ref, onMounted, watch } from 'vue';
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

function formatVol(v: number): string {
  if (v >= 10000) return (v / 10000).toFixed(0) + '万';
  return v.toString();
}

// Pad to 5 levels
const paddedBids: (Level | null)[] = depth.value
  ? Array.from({ length: 5 }, (_, i) => depth.value!.bids[i] ?? null)
  : Array.from({ length: 5 }, () => null);
const paddedAsks: (Level | null)[] = depth.value
  ? Array.from({ length: 5 }, (_, i) => depth.value!.asks[i] ?? null)
  : Array.from({ length: 5 }, () => null);
</script>

<template>
  <div class="depth-panel">
    <div class="depth-title">五档盘口</div>
    <div v-if="loading" class="depth-loading">加载中...</div>
    <div v-else-if="error" class="depth-error">{{ error }}</div>
    <div v-else class="depth-body">
      <div class="depth-half bids">
        <div class="depth-header-row">
          <span>买价</span><span>买量</span>
        </div>
        <div
          v-for="(level, i) in paddedBids"
          :key="'b' + i"
          class="depth-row"
          :class="{ 'depth-empty': !level }"
        >
          <span class="depth-price bid-price">{{ level?.price?.toFixed(2) ?? '--' }}</span>
          <span class="depth-vol">{{ level ? formatVol(level.volume) : '--' }}</span>
        </div>
      </div>
      <div class="depth-half asks">
        <div class="depth-header-row">
          <span>卖价</span><span>卖量</span>
        </div>
        <div
          v-for="(level, i) in paddedAsks"
          :key="'a' + i"
          class="depth-row"
          :class="{ 'depth-empty': !level }"
        >
          <span class="depth-price ask-price">{{ level?.price?.toFixed(2) ?? '--' }}</span>
          <span class="depth-vol">{{ level ? formatVol(level.volume) : '--' }}</span>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.depth-panel {
  min-width: 260px;
  padding: 8px;
}
.depth-title {
  font-size: 13px;
  font-weight: 600;
  margin-bottom: 8px;
  color: var(--color-text-primary, #e0e0e0);
}
.depth-loading, .depth-error {
  font-size: 12px;
  color: var(--color-text-tertiary, #888);
}
.depth-body {
  display: flex;
  gap: 12px;
}
.depth-half {
  flex: 1;
}
.depth-header-row {
  display: flex;
  justify-content: space-between;
  font-size: 11px;
  color: var(--color-text-tertiary, #888);
  padding: 2px 0;
  border-bottom: 1px solid var(--color-border, rgba(255,255,255,0.08));
  margin-bottom: 4px;
}
.depth-row {
  display: flex;
  justify-content: space-between;
  padding: 3px 0;
  font-size: 12px;
  font-variant-numeric: tabular-nums;
}
.depth-empty { opacity: 0.25; }
.bid-price { color: #ef5350; }
.ask-price { color: #66bb6a; }
.depth-vol { color: var(--color-text-secondary, #ccc); }
</style>
