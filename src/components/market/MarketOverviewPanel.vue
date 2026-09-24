<script setup lang="ts">
import { computed, onMounted, onUnmounted } from 'vue';
import { useMarketStore, type MarketDirection } from '@/stores/market';
import type { SectorItem } from '@/types';
import { formatAmount } from '@/utils/format';
import { ChevronDown } from '@lucide/vue';

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

/** 涨跌配色:A股惯例红涨绿跌,持平为中性灰 —— 平盘不是涨,不能标红。 */
function rowClass(pct: number) {
  return pct > 0 ? 'row-up' : pct < 0 ? 'row-down' : 'row-flat';
}
function pctClass(pct: number) {
  return pct > 0 ? 'pct-up' : pct < 0 ? 'pct-down' : 'pct-flat';
}

/** 榜单方向决定看哪只成分股:涨幅榜看领涨股,跌幅榜看领跌股(不是同一个字段)。 */
const leaderLabel = computed(() => (market.direction === 'up' ? '领涨' : '领跌'));
function leaderOf(s: SectorItem) {
  return market.direction === 'up' ? s.leader_name : s.laggard_name;
}

function pctText(pct: number) {
  if (pct === 0) return '0.00%';
  return `${pct > 0 ? '+' : ''}${pct.toFixed(2)}%`;
}

/** 背景色条宽度:涨跌幅绝对值相对 10%(主板涨停)归一化,封顶 100%。
 *  下限 3% —— 微涨(如 +0.05%)时若按比例只有几 px,会被误读成渲染瑕疵。 */
function barWidth(pct: number) {
  const capped = Math.min(Math.abs(pct) / 10, 1);
  return `${Math.max(capped * 100, 3).toFixed(1)}%`;
}

function toggleExpand() {
  market.setExpanded(!market.expanded);
}

onMounted(() => {
  // 无论展开与否都要轮询:折叠时标题栏的成交额/涨跌家数依赖它。
  // startRefresh 内部会先立即拉一次,再按后端 market_clock 的时段间隔排期。
  void market.startRefresh();
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
        <ChevronDown
          class="chevron"
          :class="{ 'chevron-expanded': market.expanded }"
          :size="10"
          aria-hidden="true"
        />
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
          @click="market.setDirection(opt.key)"
        >
          {{ opt.label }}
        </button>
      </div>

      <!-- 命令整体失败时错误落地展示;部分字段失败由后端降级,不会走到这里 -->
      <p v-if="market.error" class="sector-error">{{ market.error }}</p>
      <div v-else class="sector-columns">
        <div class="sector-column">
          <h4 class="sector-column-title">行业板块</h4>
          <ul v-if="market.overview && market.overview.industry.length > 0" class="sector-list">
            <li
              v-for="(s, i) in market.overview.industry"
              :key="s.code"
              class="sector-row"
              :class="rowClass(s.change_pct)"
            >
              <span v-if="s.change_pct !== 0" class="sector-bar" aria-hidden="true" :style="{ width: barWidth(s.change_pct) }"></span>
              <span class="sector-rank tabular-nums" :class="{ 'rank-top': i < 3 }">{{ i + 1 }}</span>
              <span class="sector-name">{{ s.name }}</span>
              <span class="sector-pct tabular-nums" :class="pctClass(s.change_pct)">
                {{ pctText(s.change_pct) }}
              </span>
              <span class="sector-leader">
                <template v-if="leaderOf(s)">
                  <span class="leader-label">{{ leaderLabel }}</span>{{ leaderOf(s) }}
                </template>
                <template v-else>--</template>
              </span>
            </li>
          </ul>
          <p v-else-if="market.loading" class="sector-empty">加载中…</p>
          <p v-else class="sector-empty">行业板块暂不可用</p>
        </div>

        <div class="sector-column">
          <h4 class="sector-column-title">概念板块</h4>
          <ul v-if="market.overview && market.overview.concept.length > 0" class="sector-list">
            <li
              v-for="(s, i) in market.overview.concept"
              :key="s.code"
              class="sector-row"
              :class="rowClass(s.change_pct)"
            >
              <span v-if="s.change_pct !== 0" class="sector-bar" aria-hidden="true" :style="{ width: barWidth(s.change_pct) }"></span>
              <span class="sector-rank tabular-nums" :class="{ 'rank-top': i < 3 }">{{ i + 1 }}</span>
              <span class="sector-name">{{ s.name }}</span>
              <span class="sector-pct tabular-nums" :class="pctClass(s.change_pct)">
                {{ pctText(s.change_pct) }}
              </span>
              <span class="sector-leader">
                <template v-if="leaderOf(s)">
                  <span class="leader-label">{{ leaderLabel }}</span>{{ leaderOf(s) }}
                </template>
                <template v-else>--</template>
              </span>
            </li>
          </ul>
          <p v-else-if="market.loading" class="sector-empty">加载中…</p>
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
/* 折叠 ▶ / 展开 ▼ —— 常见展开指示方向。原先基础态是 ▼、展开转 90° 成 ◀,
   两个方向都不符合习惯,默认折叠后尤其容易误读。 */
.chevron {
  color: var(--color-text-tertiary);
  transition: transform var(--transition-fast);
  transform: rotate(-90deg);
}
.chevron-expanded {
  transform: rotate(0deg);
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
  padding: var(--space-2) var(--space-4);
}

/* 分段控件 —— 与股票详情里的周期切换(ChartSwitcher)保持同一套样式 */
.direction-toggle {
  display: flex;
  gap: 2px;
  margin-bottom: var(--space-2);
  padding: 2px;
  border-radius: var(--radius-md);
  background: var(--color-surface-2);
  width: fit-content;
}
.direction-btn {
  padding: 3px 12px;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
  font-family: var(--font-sans);
  line-height: 1.4;
  cursor: pointer;
  transition: all var(--transition-fast);
}
.direction-btn:hover {
  color: var(--color-text-secondary);
}
.direction-btn-active {
  background: var(--color-accent-dim);
  color: var(--color-accent);
  font-weight: var(--font-weight-medium);
}

.sector-columns {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: var(--space-6);
}

.sector-column-title {
  margin: 0 0 var(--space-3);
  font-size: var(--text-xs);
  font-weight: var(--font-weight-medium);
  color: var(--color-text-secondary);
}

/* 行间距 > 行内 padding,标题间距 > 行间距 —— 三级留白让分组关系自明 */
.sector-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  list-style: none;
  margin: 0;
  padding: 0;
}

/* 行 —— grid 定宽列:序号 / 名称 / 涨跌幅 / 领涨股。
   涨跌幅与领涨股都是确定宽度,标签位置才不会被领涨股名称长度推着左右跑;
   名称列独占剩余空间(1fr),空间不足时靠省略号截断而不是挤压其它列。 */
.sector-row {
  position: relative;
  display: grid;
  grid-template-columns: 14px minmax(0, 1fr) auto 104px;
  align-items: center;
  gap: var(--space-3);
  padding: 4px 8px;
  border-radius: var(--radius-md);
}

/* 涨幅背景色条:渐变实色 → 透明,宽度随涨幅强度,不遮挡文字 */
.sector-bar {
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  border-radius: var(--radius-md);
  pointer-events: none;
}
.row-up .sector-bar {
  background: linear-gradient(90deg, var(--color-up-bar), var(--color-up-bar-end));
}
.row-down .sector-bar {
  background: linear-gradient(90deg, var(--color-down-bar), var(--color-down-bar-end));
}

.sector-rank {
  position: relative;
  text-align: right;
  font-size: var(--text-xs);
  color: var(--color-text-tertiary);
}
/* 前三名加重,榜单扫读时的视觉锚点 */
.rank-top {
  color: var(--color-text-secondary);
  font-weight: var(--font-weight-medium);
}
.sector-name {
  position: relative;
  min-width: 0;
  color: var(--color-text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
/* 定宽 + 右对齐:6 位(+6.82%)与 7 位(+20.00%)的标签同宽(56px = 7 位实测宽),
   数字成列;若出现更长的数值则向左生长,不会撞到领涨股列 */
.sector-pct {
  position: relative;
  justify-self: end;
  min-width: 56px;
  text-align: right;
  font-size: var(--text-xs);
  font-weight: var(--font-weight-semibold);
  padding: 1px 6px;
  border-radius: var(--radius-sm);
  white-space: nowrap;
}
.pct-up { background: var(--color-up-bg); color: var(--color-up); }
.pct-down { background: var(--color-down-bg); color: var(--color-down); }
/* 持平:中性底 + 次要文字色,不参与红绿语义 */
.pct-flat { background: var(--color-surface-2); color: var(--color-text-secondary); }
/* 104px ≈ 领涨(26px) + 间距 + 5 个汉字 / XD·*ST 前缀,是 A 股简称的实际上限。
   左对齐:各行的「领涨」二字对齐成一条竖线,长短不一的是右侧股名,右侧本是空白 */
.sector-leader {
  position: relative;
  color: var(--color-text-secondary);
  text-align: left;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
/* 用色阶而非 opacity 区分主次:0.7 透明度会把 tertiary 压到 ~2.9:1,低于可读阈值 */
.leader-label {
  margin-right: 4px;
  color: var(--color-text-tertiary);
}

.sector-empty {
  margin: 0;
  padding: var(--space-2) 0;
  font-size: var(--text-xs);
  color: var(--color-text-tertiary);
}

/* 拉取失败 —— 用次要色而非红/绿:A股语境里红绿有涨跌含义,不该挪作错误提示 */
.sector-error {
  margin: 0;
  padding: var(--space-2) 0;
  font-size: var(--text-xs);
  color: var(--color-text-secondary);
}
</style>
