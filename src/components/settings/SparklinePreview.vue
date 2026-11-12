<script setup lang="ts">
// 设置页里的迷你分时线预览。
//
// 存在的理由就一条：这个选项的效果发生在另一个窗口（行情条），改完看不到任何
// 反馈，用户只能切回去确认。所以这里就地画一格 —— 但画的是**示意数据**：
// 预览要的是「这两种画法长什么样」，不是「这只股票今天怎么走」，为了它去发
// get_intraday 既打扰数据源又要挑一只真的自选（新装的用户还没有）。
//
// 画法与纵轴模式全部走真实链路：`paintTickerSparkline` / `resolveSparklineLines`
// 都是行情条用的那一份（含物理像素对齐与 alpha 规则），所以预览不会画出一个
// 实际看不到的样子 —— 这类「预览与实物不符」比没有预览更糟。
import { nextTick, onMounted, ref, watch } from 'vue';
import { useSettingsStore } from '@/stores/settings';
import {
  SPARKLINE_CANVAS_HEIGHT,
  SPARKLINE_CANVAS_WIDTH,
  paintTickerSparkline,
  resolveSparklineLines,
  sparklineBaselineY,
  sparklineCanvasPoints,
} from '@/utils/tickerSparkline';
import type { TickerChartMode, TickerChartStyle } from '@/utils/prefs';

const props = defineProps<{
  mode: TickerChartMode;
  /** 不叫 `style` —— Vue 对它有特殊处理，声明成 prop 收不到值 */
  chartStyle: TickerChartStyle;
}>();

const canvasRef = ref<HTMLCanvasElement | null>(null);

/** 昨收。示意数据以此为基准，涨跌都围着它走。 */
const PREV_CLOSE = 100;
/** 主板 10% —— 预览里涨跌停轴要有边界可算，给个最常见的比例 */
const LIMIT_RATE = 0.1;

/**
 * 一天的示意分时：早盘下探、午前拉起、午后回落到略高于昨收。
 *
 * 形状要同时喂饱两套纵轴 —— 价格轴看得到起伏（±0.7% 量级，与真实行情同数量级，
 * 不是随手画的余弦），涨跌停轴看得到「离两板都还远」。时间戳用交易日分钟铺到
 * 09:30–14:59，午休那段留空洞，午间断口才会出现在预览里。
 */
const SAMPLE_MINUTES = [2, 18, 34, 52, 68, 84, 118, 124, 142, 163, 182, 205, 228];
const SAMPLE_CLOSES = [99.92, 99.54, 99.68, 99.9, 100.24, 100.42, 100.55, 100.48, 100.6, 100.72, 100.44, 100.52, 100.36];

const sampleBars = SAMPLE_MINUTES.map((minute, i) => ({
  timestamp: minuteOfDay(minute),
  close: SAMPLE_CLOSES[i],
}));

/** 把「当日交易日分钟」还原成一个真实时间戳（09:30 起，跨过 11:30 补午休）。 */
function minuteOfDay(minute: number): number {
  const morning = 120;
  const wall = minute <= morning ? 9 * 60 + 30 + minute : 13 * 60 + (minute - morning);
  // 2026-01-05 是周一；预览数据不要求真实交易日，只要落在 09:30–15:00 之间。
  return new Date(2026, 0, 5, Math.floor(wall / 60), wall % 60).getTime();
}

function palette() {
  const css = getComputedStyle(document.documentElement);
  const hex = (name: string, fallback: string) => {
    const v = css.getPropertyValue(name).trim();
    return /^#[0-9a-f]{6}$/i.test(v) ? v : fallback;
  };
  return {
    accent: hex('--color-accent', '#58a6ff'),
    up: hex('--color-up', '#f85149'),
    down: hex('--color-down', '#3fb950'),
    baseline: css.getPropertyValue('--color-text-tertiary').trim() || '#8b949e',
  };
}

function paint() {
  const canvas = canvasRef.value;
  if (!canvas) return;
  const width = canvas.clientWidth;
  const height = canvas.clientHeight;
  if (width <= 0 || height <= 0) return;

  const dpr = Math.max(window.devicePixelRatio || 1, 1);
  canvas.width = Math.round(width * dpr);
  canvas.height = Math.round(height * dpr);
  const ctx = canvas.getContext('2d');
  if (!ctx) return;

  // 与行情条同一条纵轴判定：涨跌停规则在这里就是主板 10%。
  const { main, limit } = resolveSparklineLines(sampleBars, PREV_CLOSE, props.mode, LIMIT_RATE);
  const lastBar = sampleBars[sampleBars.length - 1];
  const primaryPoints = sparklineCanvasPoints(sampleBars, main ?? limit, width);
  const dot = primaryPoints[primaryPoints.length - 1];

  paintTickerSparkline(
    ctx,
    {
      style: props.chartStyle,
      baselineY: sparklineBaselineY(),
      under: sparklineCanvasPoints(sampleBars, limit, width),
      over: sparklineCanvasPoints(sampleBars, main, width),
      dot: dot && lastBar ? { x: dot.x, y: dot.y, rising: lastBar.close >= PREV_CLOSE } : null,
      width,
      height,
    },
    palette(),
    dpr,
  );
}

const settings = useSettingsStore();

onMounted(paint);
// 模式/画法变了立刻重画；主题、配色、透明切换也要重画 —— canvas 读不了 var()，
// 只能等样式落定后重取一次调色板。
watch(
  [
    () => props.mode,
    () => props.chartStyle,
    () => settings.theme,
    () => settings.colorScheme,
  ],
  () => {
    // mode 从 off 切回来时 canvas 这一刻还不存在（v-if 刚翻真），
    // 直接画会拿到 null 然后什么都不发生。
    void nextTick(paint);
  },
);
</script>

<template>
  <!-- 关闭时行情条整块不挂图，这里也就不画：画一个空的坐标格会让人以为
       「关了还留着框」。 -->
  <canvas
    v-if="mode !== 'off'"
    ref="canvasRef"
    class="spark-preview"
    :style="{
      width: `${SPARKLINE_CANVAS_WIDTH}px`,
      height: `${SPARKLINE_CANVAS_HEIGHT}px`,
    }"
    aria-hidden="true"
  ></canvas>
</template>

<style scoped>
.spark-preview {
  display: block;
  flex: none;
  /* 预览只是说明画法，不该抢走设置页的点击与滚动 */
  pointer-events: none;
}
</style>
