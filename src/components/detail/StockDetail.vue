<script setup lang="ts">
import { ref, computed } from 'vue';
import type { WatchItem, PeriodType, SubIndicatorType, MainOverlayType } from '@/types';
import { useQuoteStore } from '@/stores/quote';
import { useMinuteKUnavailable } from '@/composables/minutePeriod';
import StockSummary from './StockSummary.vue';
import DepthPanel from './DepthPanel.vue';
import MinuteChart from './MinuteChart.vue';
import KLineChart from './KLineChart.vue';
import ChartSwitcher from './ChartSwitcher.vue';
import SubIndicatorSwitcher from './SubIndicatorSwitcher.vue';
import MainOverlaySwitcher from './MainOverlaySwitcher.vue';

const props = defineProps<{
  item: WatchItem;
}>();

const emit = defineEmits<{
  close: [];
}>();

const quoteStore = useQuoteStore();
const quote = computed(() => quoteStore.getQuote(props.item.code, props.item.market));

const activePeriod = ref<PeriodType>('minute');
const activeSubIndicator = ref<SubIndicatorType>('VOL');
const activeMainOverlay = ref<MainOverlayType>('MA');

// 新浪数据源不支持 1 分钟：若正在查看 1 分钟时切到新浪，自动回落到 5 分钟。
useMinuteKUnavailable(activePeriod);
</script>

<template>
  <div class="stock-detail">
    <div class="detail-header">
      <div class="detail-title">
        <span class="detail-name">{{ item.name }}</span>
        <span class="detail-code">{{ item.code }}</span>
      </div>
      <button class="detail-close" @click="emit('close')" aria-label="关闭详情">&times;</button>
    </div>

    <div class="detail-content">
      <div class="detail-left">
        <StockSummary v-if="quote" :quote="quote" />
        <DepthPanel :code="item.code" :market="item.market" />
      </div>
      <div class="detail-right">
        <div class="chart-toolbar">
          <div class="chart-toolbar-group">
            <ChartSwitcher v-model="activePeriod" />
            <MainOverlaySwitcher
              v-if="activePeriod !== 'minute'"
              v-model="activeMainOverlay"
            />
          </div>
          <SubIndicatorSwitcher
            v-if="activePeriod !== 'minute'"
            v-model="activeSubIndicator"
          />
        </div>
        <MinuteChart
          v-if="activePeriod === 'minute'"
          :code="item.code"
          :market="item.market"
          :name="item.name"
        />
        <KLineChart
          v-else
          :code="item.code"
          :market="item.market"
          :name="item.name"
          :period="activePeriod"
          :sub-indicator="activeSubIndicator"
          :main-overlay="activeMainOverlay"
        />
      </div>
    </div>
  </div>
</template>

<style scoped>
.stock-detail {
  border-top: 1px solid var(--color-border, rgba(255,255,255,0.08));
  background: var(--color-surface-1);
  padding: 12px 16px;
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
.detail-content {
  display: flex;
  gap: 16px;
}
.detail-left {
  display: flex;
  flex-direction: column;
  gap: 12px;
  flex-shrink: 0;
}
.detail-right {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.chart-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.chart-toolbar-group {
  display: flex;
  align-items: center;
  gap: 8px;
}
</style>
