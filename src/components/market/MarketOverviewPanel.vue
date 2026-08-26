<script setup lang="ts">
import { computed, onMounted, onUnmounted } from 'vue';
import { useMarketStore, type MarketDirection } from '@/stores/market';
import { formatAmount } from '@/utils/format';

const market = useMarketStore();

const turnoverText = computed(() => formatAmount(market.overview?.turnover));
const up = computed(() => market.overview?.up ?? 0);
const down = computed(() => market.overview?.down ?? 0);
const flat = computed(() => market.overview?.flat ?? 0);
const breadthAvailable = computed(() => up.value + down.value + flat.value > 0);

const directionOptions: { key: MarketDirection; label: string }[] = [
  { key: 'up', label: '涨幅榜' },
  { key: 'down', label: '跌幅榜' },
];

function isUp(pct: number) {
  return pct >= 0;
}

function pctText(pct: number) {
  return `${pct >= 0 ? '+' : ''}${pct.toFixed(2)}%`;
}

/** 背景色条宽度:涨跌幅绝对值相对 10%(主板涨停)归一化,封顶 100%。 */
function barWidth(pct: number) {
  const capped = Math.min(Math.abs(pct) / 10, 1);
  return `${(capped * 100).toFixed(1)}%`;
}

function toggleExpand() {
  market.setExpanded(!market.expanded);
}

onMounted(() => {
  if (market.expanded) {
    market.fetchOverview();
    market.startRefresh();
  }
});

onUnmounted(() => {
  market.stopRefresh();
});
</script>

<template>
  <section class="market-overview" aria-label="市场概览">
    <!-- 收起态/标题栏 —— 始终显示 -->
    <button
      class="overview-header"
      type="button"
      :aria-expanded="market.expanded"
      @click="toggleExpand"
    >
      <span class="header-title">
        <svg
          class="chevron"
          :class="{ 'chevron-expanded': market.expanded }"
          viewBox="0 0 12 12"
          width="10"
          height="10"
          fill="none"
          aria-hidden="true"
        >
          <path d="M3 4.5 6 7.5 9 4.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
        市场概览
      </span>

      <span class="header-right">
        <span class="turnover">
          <span class="turnover-label">两市成交额</span>
          <span class="turnover-value tabular-nums">{{ turnoverText }}</span>
        </span>

        <span v-if="breadthAvailable" class="breadth">
          <span class="breadth-counts tabular-nums">
            <span class="up">涨 {{ up }}</span>
            <span class="flat">平 {{ flat }}</span>
            <span class="down">跌 {{ down }}</span>
          </span>
          <span
            class="breadth-bar"
            role="img"
            :aria-label="`上涨${up}家，下跌${down}家，平盘${flat}家`"
          >
            <span class="bar-up" :style="{ flex: up }"></span>
            <span class="bar-flat" :style="{ flex: flat }"></span>
            <span class="bar-down" :style="{ flex: down }"></span>
          </span>
        </span>
        <span v-else class="breadth-missing tabular-nums">--</span>
      </span>
    </button>

    <!-- 展开态 body -->
    <div v-if="market.expanded" class="overview-body">
      <div class="direction-toggle" role="tablist" aria-label="板块榜单方向">
        <button
          v-for="opt in directionOptions"
          :key="opt.key"
          class="direction-btn"
          :class="{ 'direction-btn-active': market.direction === opt.key }"
          type="button"
          role="tab"
          :aria-selected="market.direction === opt.key"
          @click="market.toggleDirection()"
        >
          {{ opt.label }}
        </button>
      </div>

      <div class="sector-columns">
        <div class="sector-column">
          <h4 class="sector-column-title">行业板块</h4>
          <ul v-if="market.overview && market.overview.industry.length > 0" class="sector-list">
            <li
              v-for="(s, i) in market.overview.industry"
              :key="s.code"
              class="sector-row"
              :class="isUp(s.change_pct) ? 'row-up' : 'row-down'"
            >
              <span class="sector-bar" aria-hidden="true" :style="{ width: barWidth(s.change_pct) }"></span>
              <span class="sector-rank tabular-nums">{{ i + 1 }}</span>
              <span class="sector-name">{{ s.name }}</span>
              <span class="sector-pct tabular-nums" :class="isUp(s.change_pct) ? 'pct-up' : 'pct-down'">
                {{ pctText(s.change_pct) }}
              </span>
              <span class="sector-leader">
                <template v-if="s.leader_name">
                  <span class="leader-label">领涨</span>{{ s.leader_name }}
                </template>
                <template v-else>--</template>
              </span>
            </li>
          </ul>
          <p v-else class="sector-empty">行业板块暂不可用</p>
        </div>

        <div class="sector-column">
          <h4 class="sector-column-title">概念板块</h4>
          <ul v-if="market.overview && market.overview.concept.length > 0" class="sector-list">
            <li
              v-for="(s, i) in market.overview.concept"
              :key="s.code"
              class="sector-row"
              :class="isUp(s.change_pct) ? 'row-up' : 'row-down'"
            >
              <span class="sector-bar" aria-hidden="true" :style="{ width: barWidth(s.change_pct) }"></span>
              <span class="sector-rank tabular-nums">{{ i + 1 }}</span>
              <span class="sector-name">{{ s.name }}</span>
              <span class="sector-pct tabular-nums" :class="isUp(s.change_pct) ? 'pct-up' : 'pct-down'">
                {{ pctText(s.change_pct) }}
              </span>
              <span class="sector-leader">
                <template v-if="s.leader_name">
                  <span class="leader-label">领涨</span>{{ s.leader_name }}
                </template>
                <template v-else>--</template>
              </span>
            </li>
          </ul>
          <p v-else class="sector-empty">概念板块暂不可用</p>
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.market-overview {
  flex-shrink: 0;
  border-bottom: 1px solid var(--color-border-0);
  background: var(--color-surface-0);
}

/* ── 标题栏 ── */
.overview-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
  width: 100%;
  padding: var(--space-2) var(--space-4);
  background: none;
  border: none;
  cursor: pointer;
  font-family: var(--font-sans);
  color: var(--color-text-primary);
  transition: background var(--transition-fast);
}
.overview-header:hover {
  background: var(--color-bg-elevated);
}
.overview-header:focus-visible {
  outline: 2px solid var(--color-accent);
  outline-offset: -2px;
}

.header-title {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--text-sm);
  font-weight: var(--font-weight-semibold);
  color: var(--color-text-primary);
  flex-shrink: 0;
}
.chevron {
  color: var(--color-text-tertiary);
  transition: transform var(--transition-fast);
}
.chevron-expanded {
  transform: rotate(90deg);
}

.header-right {
  display: inline-flex;
  align-items: center;
  gap: var(--space-5);
  flex-shrink: 0;
}

/* 成交额 —— 视觉重心 */
.turnover {
  display: inline-flex;
  align-items: baseline;
  gap: var(--space-2);
}
.turnover-label {
  font-size: var(--text-xs);
  color: var(--color-text-tertiary);
}
.turnover-value {
  font-size: var(--text-md);
  font-weight: var(--font-weight-semibold);
  color: var(--color-text-primary);
}

/* 涨跌分布条 */
.breadth {
  display: inline-flex;
  align-items: center;
  gap: var(--space-3);
}
.breadth-counts {
  display: inline-flex;
  gap: var(--space-2);
  font-size: var(--text-xs);
  white-space: nowrap;
}
.breadth-bar {
  display: flex;
  width: 150px;
  height: 6px;
  border-radius: var(--radius-full);
  overflow: hidden;
  background: var(--color-surface-2);
}
.bar-up { background: var(--color-up); }
.bar-flat { background: var(--color-text-tertiary); }
.bar-down { background: var(--color-down); }

.breadth-missing {
  font-size: var(--text-xs);
  color: var(--color-text-tertiary);
}

.up { color: var(--color-up); }
.down { color: var(--color-down); }
.flat { color: var(--color-text-tertiary); }

/* ── 展开体 ── */
.overview-body {
  padding: var(--space-3) var(--space-4) var(--space-3);
}

.direction-toggle {
  display: inline-flex;
  gap: var(--space-1);
  margin-bottom: var(--space-3);
  padding: 2px;
  border-radius: var(--radius-md);
  background: var(--color-surface-2);
}
.direction-btn {
  padding: 2px 12px;
  border: none;
  border-radius: var(--radius-sm);
  background: none;
  color: var(--color-text-secondary);
  font-size: var(--text-xs);
  font-family: var(--font-sans);
  cursor: pointer;
  transition: background var(--transition-fast), color var(--transition-fast);
}
.direction-btn:hover {
  color: var(--color-text-primary);
}
.direction-btn-active {
  background: var(--color-surface-3);
  color: var(--color-text-primary);
  font-weight: var(--font-weight-medium);
}

.sector-columns {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: var(--space-6);
}

.sector-column-title {
  margin: 0 0 var(--space-2);
  font-size: var(--text-xs);
  font-weight: var(--font-weight-medium);
  color: var(--color-text-secondary);
}

.sector-list {
  list-style: none;
  margin: 0;
  padding: 0;
}

/* 行 —— flex 布局:序号/名称居左,涨跌幅/领涨股靠右,减少三列均分的留白 */
.sector-row {
  position: relative;
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: 4px 6px;
  border-radius: var(--radius-sm);
}

/* 涨幅背景色条:渐变实色 → 透明,宽度随涨幅强度,不遮挡文字 */
.sector-bar {
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  border-radius: var(--radius-sm);
  pointer-events: none;
  opacity: 0.16;
}
.row-up .sector-bar {
  background: linear-gradient(90deg, var(--color-up), transparent);
}
.row-down .sector-bar {
  background: linear-gradient(90deg, var(--color-down), transparent);
}

.sector-rank {
  position: relative;
  width: 14px;
  text-align: right;
  flex-shrink: 0;
  font-size: var(--text-xs);
  color: var(--color-text-tertiary);
}
.sector-name {
  position: relative;
  flex: 0 1 auto;
  min-width: 0;
  color: var(--color-text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.sector-pct {
  position: relative;
  margin-left: auto;
  flex-shrink: 0;
  font-size: var(--text-xs);
  font-weight: var(--font-weight-semibold);
  padding: 1px 6px;
  border-radius: var(--radius-sm);
  white-space: nowrap;
}
.pct-up { background: var(--color-up-bg); color: var(--color-up); }
.pct-down { background: var(--color-down-bg); color: var(--color-down); }
.sector-leader {
  position: relative;
  flex-shrink: 0;
  min-width: 0;
  max-width: 45%;
  color: var(--color-text-tertiary);
  text-align: right;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.leader-label {
  margin-right: 4px;
  opacity: 0.7;
}

.sector-empty {
  margin: 0;
  padding: var(--space-2) 0;
  font-size: var(--text-xs);
  color: var(--color-text-tertiary);
}
</style>
