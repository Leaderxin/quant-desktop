<script setup lang="ts">
// 行情条每行右侧的迷你分时线单元格。
//
// 每行右侧那块图：75×30，画这一只今天的分时。纵轴可以是价格轴、涨跌停轴，或两者
// 叠在一起（涨跌停那条 50% 透明）。尺寸写死而不是按内容量，否则数据到没到、请求成
// 没成功都会让窗口高度来回跳；宽度固定还保证同一屏各行的图右边缘对齐。
// 几何常量在 utils/tickerSparkline 里，只有 0% 虚线 + 两条曲线。
import { computed, ref, toRef } from 'vue';
import { useTickerSparkline } from '@/composables/useTickerSparkline';
import { SPARKLINE_CANVAS_HEIGHT, SPARKLINE_CANVAS_WIDTH } from '@/utils/tickerSparkline';
import type { TickerChartMode, TickerChartStyle } from '@/utils/prefs';

const props = defineProps<{
  code: string;
  market: string;
  name: string;
  /** off 由父级决定不挂载本组件 */
  mode: TickerChartMode;
  /** 画法：带 x 轴线与刻度 / 只有虚线与曲线。
   *  不叫 `style` —— 那是 Vue 特殊处理的保留名，声明成 prop 收不到值。 */
  chartStyle: TickerChartStyle;
  /** 实时行情给的兜底昨收（price - change） */
  prevClose?: number;
}>();

const canvasRef = ref<HTMLCanvasElement | null>(null);

// 尺寸写在 style 上而不是 scoped CSS 里：绘制坐标与它必须同一来源
// （utils/tickerSparkline 的几何常量），写两处数字迟早会漂。
const canvasStyle = computed(() => ({
  width: `${SPARKLINE_CANVAS_WIDTH}px`,
  height: `${SPARKLINE_CANVAS_HEIGHT}px`,
}));

useTickerSparkline({
  canvas: canvasRef,
  code: toRef(props, 'code'),
  market: toRef(props, 'market'),
  name: toRef(props, 'name'),
  mode: toRef(props, 'mode'),
  style: toRef(props, 'chartStyle'),
  prevClose: toRef(props, 'prevClose'),
});
</script>

<template>
  <canvas ref="canvasRef" :style="canvasStyle" class="ticker-sparkline" aria-hidden="true"></canvas>
</template>

<style scoped>
/* 实际尺寸来自 canvasStyle（与绘制几何同源）。这里只留不参与布局的部分。 */
.ticker-sparkline {
  display: block;
  flex: none;
  /* 不挡住行情条的拖动/点击 */
  pointer-events: none;
}
</style>
