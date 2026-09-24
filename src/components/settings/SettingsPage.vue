<script setup lang="ts">
// 设置页外壳：顶栏（返回 + 标题）+ 横向分区 Tab + 单列全宽滚动内容。
//
// 用 KeepAlive 而不是 v-if 切换分区：各分区有本地 UI 状态（市场概览正在输入的
// 自定义条数、行情条的分组筛选），切走再切回应当保持原样，而不是被重置。
//
// 布局参考 CC Switch 的设置页：Tab 条横向等宽铺满、固定在滚动区外不随内容滚动，
// 内容为居中限宽 1000px 的单列（见 settings.css 的 .panel）。原先的「左侧竖排
// 导航 + 720px 封顶内容」在 1100px 宽的主窗口里两侧各留一大块死白（侧栏下方
// 全是空背景，内容右侧常年空 200px）。
import { computed, ref } from 'vue';
import IndexSection from './IndexSection.vue';
import MarketSection from './MarketSection.vue';
import WatchlistSection from './WatchlistSection.vue';
import TickerSection from './TickerSection.vue';
import GeneralSection from './GeneralSection.vue';
import AboutSection from './AboutSection.vue';
import {
  ChartColumn,
  ChevronLeft,
  Info,
  LayoutDashboard,
  RectangleEllipsis,
  Settings,
  Star,
} from 'lucide-vue-next';

const emit = defineEmits<{ (e: 'close'): void }>();

// 分区图标取自 lucide（全应用统一的图标库，主界面与设置页同一套视觉语言）：
// 指数区=柱状图，市场概览=仪表盘，自选列表=星标（财经应用的「自选」通用符号），
// 行情条=药丸+省略号（滚动播报条的轮廓），通用=齿轮，关于=信息圆。
const sections = [
  { key: 'index', label: '指数区', component: IndexSection, icon: ChartColumn },
  { key: 'market', label: '市场概览', component: MarketSection, icon: LayoutDashboard },
  { key: 'watchlist', label: '自选列表', component: WatchlistSection, icon: Star },
  { key: 'ticker', label: '行情条', component: TickerSection, icon: RectangleEllipsis },
  { key: 'general', label: '通用', component: GeneralSection, icon: Settings },
  { key: 'about', label: '关于', component: AboutSection, icon: Info },
] as const;

type SectionKey = (typeof sections)[number]['key'];

const active = ref<SectionKey>('index');
const activeSection = computed(
  () => sections.find((s) => s.key === active.value) ?? sections[0],
);
</script>

<template>
  <div class="settings-page">
    <header class="settings-header">
      <button class="back-btn" type="button" @click="emit('close')">
        <ChevronLeft :size="13" aria-hidden="true" />
        返回
      </button>
      <h1>设置</h1>
    </header>

    <nav class="section-tabs" aria-label="设置分区">
      <button
        v-for="s in sections"
        :key="s.key"
        class="tab-item"
        type="button"
        :class="{ active: active === s.key }"
        :aria-current="active === s.key ? 'page' : undefined"
        @click="active = s.key"
      >
        <component :is="s.icon" :size="16" aria-hidden="true" />
        <span class="tab-label">{{ s.label }}</span>
      </button>
    </nav>

    <div class="settings-content">
      <KeepAlive>
        <component :is="activeSection.component" :key="activeSection.key" />
      </KeepAlive>
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
.settings-header h1 {
  margin: 0;
  font-size: var(--text-md);
  font-weight: var(--font-weight-semibold);
  letter-spacing: -0.01em;
  line-height: 1;
}

/* 分区 Tab：与 SegmentedControl 同一套「多选一」视觉（surface-2 轨道 + 选中
   accent-dim 底/accent 字），尺寸放大到页面级导航。横向等分铺满宽度；
   窗口拉得过窄时标签截断省略而不是把轨道撑破。
   限宽 1000px 且与内容列（settings.css 的 .panel）同一套外边距 —— 居中、
   右缘对齐：max(24px, (100% - 1000px)/2) 与 .panel 的 auto 外边距在
   任何窗口宽度下解出的左边距都相等（宽窗口平分、窄窗口回落 24px）。 */
.section-tabs {
  display: grid;
  grid-template-columns: repeat(6, 1fr);
  gap: 2px;
  max-width: 1000px;
  margin: var(--space-3) max(var(--space-6), calc((100% - 1000px) / 2)) 0;
  padding: 3px;
  border-radius: var(--radius-md);
  background: var(--color-surface-2);
  flex-shrink: 0;
}
.tab-item {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  height: 40px;
  min-width: 0;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--color-text-secondary);
  font-family: var(--font-sans);
  font-size: var(--text-md);
  /* 行高收为 1：16px 行盒与 16px 图标等高，align-items: center 才是真正的
     视觉居中。默认 1.5 行高的额外行距上下不对称，中文看起来比图标低一截。 */
  line-height: 1;
  white-space: nowrap;
  overflow: hidden;
  cursor: pointer;
  transition: background var(--transition-fast), color var(--transition-fast);
}
.tab-item .tab-label {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}
/* 选中态 = 底色 + 字色 + 字重三个通道。侧栏时代的竖条指示在横向轨道里没有
   位置，也不需要 —— 轨道本身圈定了选项范围，底色差异已足以定位。 */
.tab-item:hover {
  background: var(--color-bg-elevated);
  color: var(--color-text-primary);
}
.tab-item.active {
  background: var(--color-accent-dim);
  color: var(--color-accent);
  font-weight: var(--font-weight-medium);
}
.tab-item:focus-visible {
  outline: 2px solid var(--color-accent);
  outline-offset: -2px;
}
.tab-item svg {
  flex-shrink: 0;
}

.settings-content {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  /* 顶部 20px：Tab 条与内容区之间除了高度差，还需要一段呼吸距离才能「分层」，
     16px 时卡片几乎贴着轨道，像同一个控件的延续 */
  padding: var(--space-5) var(--space-6) var(--space-6);
}
</style>
