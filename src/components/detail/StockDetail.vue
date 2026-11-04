<script setup lang="ts">
import { ref, computed } from 'vue';
import type { WatchItem, PeriodType, SubIndicatorType, MainOverlayType } from '@/types';
import { useQuoteStore } from '@/stores/quote';
import { useMinuteKUnavailable } from '@/composables/minutePeriod';
import { formatCode, cnCategory } from '@/utils/format';
import MarketTag from '@/components/watchlist/MarketTag.vue';
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
        <span class="detail-name-cell">
          <MarketTag :code="item.code" :category="cnCategory(item.code)" />
          <span class="detail-name">{{ item.name }}</span>
        </span>
        <span class="detail-code">{{ formatCode(item.code) }}</span>
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
          :prev-close="quote ? quote.price - quote.change : undefined"
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
  /* 详情是看盘界面的下半部分：高度按视口取一个确定值，剩下的让给自选表。
     不能由内容撑开 —— 内容里的图表一旦按内容算高度，小视口下整块就会超出
     .main-content 的 overflow:hidden，底部副图直接被裁掉。
     52vh 在 1280×720（1080p @150%）下约 374px，自选表还能留 2 行。 */
  flex: 0 1 auto;
  height: clamp(240px, 52vh, 560px);
  display: flex;
  flex-direction: column;
  border-top: 1px solid var(--color-border, rgba(255,255,255,0.08));
  background: var(--color-surface-1);
  padding: 12px 16px;
}
.detail-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  flex-shrink: 0;
  margin-bottom: 10px;
}
.detail-title {
  display: flex;
  align-items: center;
  gap: 8px;
}
/* 标签 + 名称作为一组，间距与自选表名称列一致 —— 详情页与列表读的是同一个东西，
   两处间距不同会显得名称与代码的疏密关系不一致 */
.detail-name-cell {
  display: inline-flex;
  align-items: center;
  gap: 6px;
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
  flex: 1;
  min-height: 0;
  display: flex;
  gap: 16px;
}
.detail-left {
  display: flex;
  flex-direction: column;
  gap: 10px;
  flex: 0 0 300px;
  min-width: 280px;
  min-height: 0;
  /* 窗口被压到极短（市场概览展开 + 详情同时开着）时左列滚动而不是被裁 */
  overflow-y: auto;
}
.detail-right {
  flex: 1;
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.chart-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  flex-shrink: 0;
}

.chart-toolbar-group {
  display: flex;
  align-items: center;
  gap: 8px;
}

/* 矮视口（1080p @150% 全屏约 720px）收紧留白，把空间让给图表 */
@media (max-height: 820px) {
  .stock-detail {
    padding: 8px 16px 10px;
  }
  .detail-header {
    margin-bottom: 6px;
  }
  .detail-content {
    gap: 12px;
  }
  .detail-left {
    gap: 8px;
  }
}
</style>
