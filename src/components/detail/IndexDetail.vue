<script setup lang="ts">
import { computed } from 'vue';
import type { IndexQuote } from '@/types';
import { formatPrice } from '@/utils/format';
import MinuteChart from './MinuteChart.vue';

const props = defineProps<{
  index: IndexQuote;
}>();

const emit = defineEmits<{
  close: [];
}>();

const isUp = computed(() => props.index.change_pct >= 0);

// 指数摘要卡片 (5 items — no open/high/low from API)
const statCards = computed(() => [
  {
    label: '最新价',
    value: formatPrice(props.index.price),
    up: isUp.value,
  },
  {
    label: '涨跌额',
    value: `${isUp.value ? '+' : ''}${formatPrice(props.index.change)}`,
    up: isUp.value,
  },
  {
    label: '涨跌幅',
    value: `${isUp.value ? '+' : ''}${props.index.change_pct.toFixed(2)}%`,
    up: isUp.value,
  },
  {
    label: '成交量',
    value: props.index.volume >= 10000
      ? (props.index.volume / 10000).toFixed(0) + '万手'
      : props.index.volume.toLocaleString() + '手',
    up: undefined,
  },
  {
    label: '成交额',
    value: props.index.turnover >= 100000000
      ? (props.index.turnover / 100000000).toFixed(2) + '亿'
      : (props.index.turnover / 10000).toFixed(2) + '万',
    up: undefined,
  },
]);
</script>

<template>
  <div class="index-detail">
    <div class="detail-header">
      <div class="detail-title">
        <span class="detail-name">{{ index.name }}</span>
        <span class="detail-code">{{ index.code }}</span>
      </div>
      <button class="detail-close" @click="emit('close')" aria-label="关闭指数详情">&times;</button>
    </div>

    <div class="detail-body">
      <!-- 摘要卡片网格 3×2 -->
      <div class="summary-grid">
        <div
          v-for="card in statCards"
          :key="card.label"
          class="summary-card"
          :class="{
            'card-up': card.up === true,
            'card-down': card.up === false,
          }"
        >
          <span class="card-label">{{ card.label }}</span>
          <span
            class="card-value tabular-nums"
            :class="{
              'up': card.up === true,
              'down': card.up === false,
            }"
          >{{ card.value }}</span>
        </div>
      </div>

      <!-- 全宽分时图 -->
      <div class="chart-section">
        <MinuteChart
          :code="index.code"
          market="CN"
          :name="index.name"
        />
      </div>
    </div>
  </div>
</template>

<style scoped>
.index-detail {
  position: absolute;
  top: 100%;
  left: 0;
  right: 0;
  z-index: 10;
  border-top: 1px solid var(--color-border, rgba(255,255,255,0.08));
  background: var(--color-surface-0);
  padding: 12px 16px;
  max-height: calc(100vh - 140px);
  overflow-y: auto;
  box-shadow: 0 4px 12px rgba(0,0,0,0.3);
}

.detail-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 12px;
}

.detail-title {
  display: flex;
  align-items: baseline;
  gap: 8px;
}

.detail-name {
  font-size: 15px;
  font-weight: 600;
  color: var(--color-text-primary);
}

.detail-code {
  font-size: 12px;
  color: var(--color-text-tertiary);
  font-family: var(--font-mono);
}

.detail-close {
  background: none;
  border: none;
  color: var(--color-text-tertiary);
  font-size: 20px;
  cursor: pointer;
  padding: 0 4px;
  line-height: 1;
}
.detail-close:hover { color: var(--color-text-primary); }

.detail-body {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

/* 摘要卡片网格 */
.summary-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 8px;
}

.summary-card {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 8px 10px;
  border-radius: var(--radius-md);
  background: var(--color-surface-1);
  border: 1px solid var(--color-border-0);
  transition: background var(--transition-fast);
}

.summary-card.card-up {
  background: var(--color-up-bg);
}
.summary-card.card-down {
  background: var(--color-down-bg);
}

.card-label {
  font-size: 10px;
  color: var(--color-text-tertiary);
}

.card-value {
  font-size: 14px;
  font-weight: 600;
  font-family: var(--font-mono);
  font-variant-numeric: tabular-nums;
  color: var(--color-text-primary);
}

.card-value.up { color: var(--color-up); }
.card-value.down { color: var(--color-down); }

.chart-section {
  min-height: 320px;
}
</style>
