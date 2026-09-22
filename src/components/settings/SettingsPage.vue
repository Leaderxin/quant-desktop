<script setup lang="ts">
// 设置页外壳：顶栏（返回 + 标题 + 面包屑）+ 左侧分区导航 + 右侧内容。
//
// 用 KeepAlive 而不是 v-if 切换分区：各分区有本地 UI 状态（市场概览正在输入的
// 自定义条数、行情条的分组筛选），切走再切回应当保持原样，而不是被重置。
import { computed, ref } from 'vue';
import IndexSection from './IndexSection.vue';
import MarketSection from './MarketSection.vue';
import WatchlistSection from './WatchlistSection.vue';
import TickerSection from './TickerSection.vue';
import GeneralSection from './GeneralSection.vue';

const emit = defineEmits<{ (e: 'close'): void }>();

const sections = [
  { key: 'index', label: '指数区', component: IndexSection },
  { key: 'market', label: '市场概览', component: MarketSection },
  { key: 'watchlist', label: '自选列表', component: WatchlistSection },
  { key: 'ticker', label: '行情条', component: TickerSection },
  { key: 'general', label: '通用', component: GeneralSection },
] as const;

type SectionKey = (typeof sections)[number]['key'];

const active = ref<SectionKey>('index');
const activeSection = computed(
  () => sections.find((s) => s.key === active.value) ?? sections[0],
);

// 分区图标。每项带自己的 viewBox 与 stroke-width —— 齿轮取自 24 网格的通用图标，
// 按 14px 渲染时需要 2.25 的线宽才能和其余 16 网格 / 1.5 线宽的图标视觉等粗
// （2.25 × 14/24 ≈ 1.31 ≈ 1.5 × 14/16）。统一在这里声明，避免每处各调一次。
const icons: Record<SectionKey, { viewBox: string; strokeWidth: number; paths: string[] }> = {
  index: { viewBox: '0 0 16 16', strokeWidth: 1.5, paths: ['M2 12.5V9M6 12.5V4M10 12.5V7M14 12.5V2.5'] },
  market: { viewBox: '0 0 16 16', strokeWidth: 1.5, paths: ['M2 2.5h12v11H2zM2 6h12M6 6v7.5'] },
  watchlist: { viewBox: '0 0 16 16', strokeWidth: 1.5, paths: ['M2.5 4.5h11M2.5 8h11M2.5 11.5h11'] },
  ticker: { viewBox: '0 0 16 16', strokeWidth: 1.5, paths: ['M4 5h12v6H4zM6.5 8h2M10.5 8h2'] },
  // 齿轮。原先是「中心圆 + 放射线」，与状态栏主题切换的太阳图标同轮廓 ——
  // 两个图标在同一屏上出现时分不清谁是谁。齿轮的「粗环 + 外齿 + 中心孔」剪影可辨。
  general: {
    viewBox: '0 0 24 24',
    strokeWidth: 2.25,
    paths: [
      'M12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6z',
      'M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z',
    ],
  },
};
</script>

<template>
  <div class="settings-page">
    <header class="settings-header">
      <button class="back-btn" type="button" @click="emit('close')">
        <svg viewBox="0 0 16 16" width="13" height="13" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path d="M10 3 5 8l5 5"/>
        </svg>
        返回
      </button>
      <div class="title-group">
        <h1>设置</h1>
        <span class="path">/ {{ activeSection.label }}</span>
      </div>
    </header>

    <div class="settings-body">
      <nav class="side-nav" aria-label="设置分区">
        <button
          v-for="s in sections"
          :key="s.key"
          class="nav-item"
          type="button"
          :class="{ active: active === s.key }"
          :aria-current="active === s.key ? 'page' : undefined"
          @click="active = s.key"
        >
          <svg
            :viewBox="icons[s.key].viewBox"
            width="14"
            height="14"
            fill="none"
            stroke="currentColor"
            :stroke-width="icons[s.key].strokeWidth"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
          >
            <path v-for="(d, i) in icons[s.key].paths" :key="i" :d="d"/>
          </svg>
          {{ s.label }}
        </button>
      </nav>

      <div class="settings-content">
        <KeepAlive>
          <component :is="activeSection.component" :key="activeSection.key" />
        </KeepAlive>
      </div>
    </div>
  </div>
</template>

<style scoped>
.settings-page {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
  background: var(--color-surface-0);
}

.settings-header {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  height: var(--header-height);
  padding: 0 var(--space-4);
  flex-shrink: 0;
  background: var(--color-surface-1);
  border-bottom: 1px solid var(--color-border-0);
}
.back-btn {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  height: 24px;
  padding: 0 8px 0 6px;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--color-text-secondary);
  font-family: var(--font-sans);
  font-size: var(--text-xs);
  cursor: pointer;
  transition: background var(--transition-fast), color var(--transition-fast);
}
.back-btn:hover {
  background: var(--color-bg-elevated);
  color: var(--color-text-primary);
}
.back-btn:focus-visible {
  outline: 2px solid var(--color-accent);
  outline-offset: 1px;
}
/* 标题与面包屑是一行里的一个整体，所以包一层做基线对齐。
   层级靠**字重与颜色**拉开，不靠字号 —— 两者同字号、同基线，读起来是一句话
   「设置 / 通用」，而不是两个尺寸不一的碎片挤在一起。
   `align-items: baseline` 配 `line-height: 1` 才能让两个 14px 文本严格对齐；
   居中会在中文与拉丁混排时产生 1~2px 的错位。 */
.title-group {
  display: flex;
  align-items: baseline;
  gap: 6px;
  min-width: 0;
}
.settings-header h1 {
  margin: 0;
  font-size: var(--text-md);
  font-weight: var(--font-weight-semibold);
  letter-spacing: -0.01em;
  line-height: 1;
}
.path {
  font-size: var(--text-md);
  font-weight: var(--font-weight-normal);
  color: var(--color-text-tertiary);
  line-height: 1;
  white-space: nowrap;
}

.settings-body {
  display: flex;
  flex: 1;
  min-height: 0;
}

.side-nav {
  display: flex;
  flex-direction: column;
  gap: 2px;
  width: 176px;
  flex-shrink: 0;
  padding: var(--space-3) var(--space-2);
  overflow-y: auto;
  background: var(--color-surface-1);
  border-right: 1px solid var(--color-border-0);
}
.nav-item {
  position: relative;
  display: flex;
  align-items: center;
  gap: var(--space-2);
  width: 100%;
  height: 32px;
  padding: 0 10px;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--color-text-secondary);
  font-family: var(--font-sans);
  font-size: var(--text-sm);
  text-align: left;
  cursor: pointer;
  transition: background var(--transition-fast), color var(--transition-fast);
}
/* 当前分区用「竖条 + 底色 + 文字色」三重指示，不只靠颜色 —— 色觉障碍下
   底色差异可能不可辨，竖条提供了不依赖颜色的位置线索。 */
.nav-item::before {
  content: '';
  position: absolute;
  left: 0;
  top: 6px;
  bottom: 6px;
  width: 2px;
  border-radius: 0 2px 2px 0;
  background: transparent;
}
.nav-item:hover {
  background: var(--color-bg-elevated);
  color: var(--color-text-primary);
}
.nav-item.active {
  background: var(--color-accent-dim);
  color: var(--color-accent);
  font-weight: var(--font-weight-medium);
}
.nav-item.active::before {
  background: var(--color-accent);
}
.nav-item:focus-visible {
  outline: 2px solid var(--color-accent);
  outline-offset: -2px;
}
.nav-item svg {
  flex-shrink: 0;
}

.settings-content {
  flex: 1;
  min-width: 0;
  overflow-y: auto;
  padding: var(--space-4) var(--space-6) 48px;
}
</style>
