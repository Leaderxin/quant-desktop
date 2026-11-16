// src/utils/tickerSparkline.spec.ts
//
// 价格轴的边界都在这里钉住：±1% 下限、按最大偏离取半幅、无昨收时用第一根
// 有效 close 兜底，以及 09:30–15:00 的横向铺满。
import { describe, expect, it } from 'vitest';
import {
  SPARKLINE_AXIS_Y,
  SPARKLINE_CANVAS_HEIGHT,
  SPARKLINE_CANVAS_WIDTH,
  SPARKLINE_CURVE_PADDING_Y,
  SPARKLINE_PAD_TOP,
  SPARKLINE_DOT_RADIUS,
  SPARKLINE_PLOT_HEIGHT,
  SPARKLINE_TICK_HEIGHT,
  axisTickPositions,
  paintTickerSparkline,
  sparklineBaselineY,
  sparklineCanvasPoints,
  sparklinePoints,
  sparklineSpanMinutes,
  resolveSparklineRange,
} from './tickerSparkline';
import { SESSION_MINUTES } from './minuteAxis';

function bar(close: number, hour = 9, minute = 30) {
  return { timestamp: new Date(2026, 8, 24, hour, minute).getTime(), close };
}

describe('resolveSparklineRange', () => {
  it('数据波动小于 1% 时按昨收 ±1% 兜底', () => {
    const range = resolveSparklineRange([bar(100)], 100);
    expect(range.from).toBeCloseTo(99);
    expect(range.to).toBeCloseTo(101);
  });

  it('按数据最大偏离取半幅', () => {
    const range = resolveSparklineRange([bar(100), bar(103), bar(97)], 100);
    expect(range.from).toBeCloseTo(97);
    expect(range.to).toBeCloseTo(103);
  });

  it('prevClose 无效时用第一根有效 close 兜底', () => {
    const range = resolveSparklineRange([bar(50)], 0);
    expect(range.from).toBeCloseTo(49.5);
    expect(range.to).toBeCloseTo(50.5);
  });

  it('没有任何有效数据时返回空区间', () => {
    expect(resolveSparklineRange([], 0)).toEqual({ from: 0, to: 0 });
  });
});

describe('sparklineSpanMinutes — 早盘横轴自适应', () => {
  const today = [bar(100, 9, 30), bar(101, 10, 30)];
  const at = (hour: number, minute = 0) => new Date(2026, 8, 24, hour, minute).getTime();

  it('11:30 之前只铺早盘（120 分钟）', () => {
    expect(sparklineSpanMinutes(today, at(9, 30))).toBe(SESSION_MINUTES / 2);
    expect(sparklineSpanMinutes(today, at(10, 0))).toBe(SESSION_MINUTES / 2);
    expect(sparklineSpanMinutes(today, at(11, 29))).toBe(SESSION_MINUTES / 2);
  });

  it('11:30 及之后切回全天', () => {
    expect(sparklineSpanMinutes(today, at(11, 30))).toBe(SESSION_MINUTES);
    expect(sparklineSpanMinutes(today, at(12, 0))).toBe(SESSION_MINUTES);
    expect(sparklineSpanMinutes(today, at(15, 30))).toBe(SESSION_MINUTES);
  });

  it('画的是上一交易日（盘前/周末）时按全天，不把昨天塞进半天', () => {
    const nextDay = new Date(2026, 8, 25, 9, 0).getTime();
    expect(sparklineSpanMinutes(today, nextDay)).toBe(SESSION_MINUTES);
  });

  it('没有数据时按全天', () => {
    expect(sparklineSpanMinutes([], at(10, 0))).toBe(SESSION_MINUTES);
  });
});

describe('sparklinePoints', () => {
  const range = { from: 90, to: 110 };

  it('横轴按 09:30–15:00 铺满，午休不计入', () => {
    const points = sparklinePoints(
      [bar(100, 9, 30), bar(100, 11, 30), bar(100, 15, 0)],
      range,
      240,
      56,
    );
    expect(points[0].x).toBeCloseTo(0);
    expect(points[1].x).toBeCloseTo(120);
    expect(points[2].x).toBeCloseTo(240);
  });

  it('早盘跨度 120 分钟时只铺 09:30–11:30，午后 bar 不进图', () => {
    const points = sparklinePoints(
      [bar(100, 9, 30), bar(101, 10, 30), bar(102, 11, 30), bar(103, 13, 30)],
      range,
      240,
      56,
      3,
      120,
    );
    expect(points).toHaveLength(3);
    expect(points[0].x).toBeCloseTo(0);
    expect(points[1].x).toBeCloseTo(120);
    expect(points[2].x).toBeCloseTo(240);
  });

  it('纵轴线性映射并留出 padding', () => {
    const points = sparklinePoints(
      [bar(110), bar(100), bar(90)],
      range,
      240,
      56,
      3,
    );
    expect(points[0].y).toBeCloseTo(3);
    expect(points[1].y).toBeCloseTo(28);
    expect(points[2].y).toBeCloseTo(53);
  });

  it('越界数据夹在画布内，不画到 canvas 外面', () => {
    const points = sparklinePoints([bar(130), bar(70)], range, 240, 56, 3);
    expect(points[0].y).toBeCloseTo(3);
    expect(points[1].y).toBeCloseTo(53);
  });

  it('空数据 / 非法尺寸 / 零区间返回空数组', () => {
    expect(sparklinePoints([], range, 240, 56)).toEqual([]);
    expect(sparklinePoints([bar(100)], range, 0, 56)).toEqual([]);
    expect(sparklinePoints([bar(100)], range, 240, 0)).toEqual([]);
    expect(sparklinePoints([bar(100)], { from: 100, to: 100 }, 240, 56)).toEqual([]);
  });
});

describe('单元格几何', () => {
  /**
   * 两种画法共用同一个高度，且 `axis` 那条轴 + 刻度画得下。
   *
   * 高度**不**由画法推导：画法变了窗口就该跟着长高变矮，行情条会在屏幕上跳
   * 一下 —— 而它贴在右下角，跳的那一下正对着视线。刻度位置也钉在带内，
   * 免得以后有人把 PAD_TOP 改大时把轴挤到画布外。
   */
  it('画布高度固定，且装得下曲线区 + 轴线 + 向下刻度', () => {
    // 30 是硬约定，不由几何推出：两种画法同一个值，切换样式时窗口不跳。
    expect(SPARKLINE_CANVAS_HEIGHT).toBe(30);
    // 轴线紧贴曲线带下沿；刻度向下伸出后再留 3px 底部留白，不会被裁掉。
    expect(SPARKLINE_AXIS_Y).toBe(SPARKLINE_PAD_TOP + SPARKLINE_PLOT_HEIGHT + 1);
    expect(SPARKLINE_CANVAS_HEIGHT - SPARKLINE_AXIS_Y - SPARKLINE_TICK_HEIGHT).toBe(3);
  });

  it('0% 线在曲线区正中间', () => {
    expect(sparklineBaselineY()).toBeCloseTo(
      SPARKLINE_PAD_TOP + SPARKLINE_PLOT_HEIGHT / 2,
    );
  });

  it('昨收映射到 0% 线', () => {
    const bars = [bar(101, 10, 0), bar(97, 14, 0)];
    const range = resolveSparklineRange(bars, 100);
    const atPrev = sparklineCanvasPoints([bar(100, 10, 0)], range, 88)[0];
    expect(atPrev.y).toBeCloseTo(sparklineBaselineY());
    expect(sparklineCanvasPoints(bars, range, 88)).toHaveLength(2);
  });

  it('x 轴三个刻度：起点 / 中点（午间）/ 右端内缩一像素', () => {
    expect(axisTickPositions(75)).toEqual([0, 37.5, 74]);
    expect(axisTickPositions(0)).toEqual([]);
    expect(axisTickPositions(-5)).toEqual([]);
  });

  it('sparklineCanvasPoints 把曲线区整体下移 PAD_TOP，null 区间返回空', () => {
    const range = { from: 90, to: 110 };
    const points = sparklineCanvasPoints([bar(110, 15, 0)], range, 88);
    expect(points[0].x).toBeCloseTo(88);
    // 顶到区间上沿 = 顶部留白 + 曲线区内边距（都取自常量，别写死数字）
    expect(points[0].y).toBeCloseTo(SPARKLINE_PAD_TOP + SPARKLINE_CURVE_PADDING_Y);
    expect(sparklineCanvasPoints([bar(100)], null, 88)).toEqual([]);
  });
});

/**
 * `paintTickerSparkline` 的绘制指令序列。
 *
 * 用一支记录所有调用的假 ctx：这里要钉的不是像素（像素归肉眼），而是
 * 「两种画法各自画了什么」—— 预览与行情条共用这份实现，它要是把
 * `plain` 也画上线，设置里就看不到差别了。
 */
describe('paintTickerSparkline', () => {
  type Rec = { op: string; detail?: unknown };

  function fakeCtx() {
    const rec: Rec[] = [];
    const state = {
      alpha: 1,
      lineWidth: 1,
      dash: [] as number[],
      strokeStyle: '',
      fillStyle: '',
    };
    const ctx = {
      rec,
      setTransform: () => rec.push({ op: 'setTransform' }),
      clearRect: () => rec.push({ op: 'clearRect' }),
      save: () => rec.push({ op: 'save' }),
      restore: () => rec.push({ op: 'restore' }),
      beginPath: () => rec.push({ op: 'beginPath' }),
      moveTo: (x: number, y: number) => rec.push({ op: 'moveTo', detail: [x, y] }),
      lineTo: (x: number, y: number) => rec.push({ op: 'lineTo', detail: [x, y] }),
      closePath: () => rec.push({ op: 'closePath' }),
      createLinearGradient: (x0: number, y0: number, x1: number, y1: number) => {
        rec.push({ op: 'gradient', detail: [x0, y0, x1, y1] });
        return {
          addColorStop: (offset: number, color: string) =>
            rec.push({ op: 'colorStop', detail: [offset, color] }),
        };
      },
      stroke: () =>
        rec.push({
          op: 'stroke',
          detail: {
            alpha: state.alpha,
            lineWidth: state.lineWidth,
            dash: [...state.dash],
            color: state.strokeStyle,
          },
        }),
      arc: (x: number, y: number, r: number) => rec.push({ op: 'arc', detail: [x, y, r] }),
      fill: () => rec.push({ op: 'fill', detail: state.fillStyle }),
      setLineDash: (d: number[]) => {
        state.dash = d;
      },
    };
    Object.defineProperty(ctx, 'globalAlpha', {
      get: () => state.alpha,
      set: (v: number) => {
        state.alpha = v;
      },
    });
    Object.defineProperty(ctx, 'lineWidth', {
      get: () => state.lineWidth,
      set: (v: number) => {
        state.lineWidth = v;
      },
    });
    Object.defineProperty(ctx, 'strokeStyle', {
      get: () => state.strokeStyle,
      set: (v: string) => {
        state.strokeStyle = v;
      },
    });
    Object.defineProperty(ctx, 'fillStyle', {
      get: () => state.fillStyle,
      set: (v: string) => {
        state.fillStyle = v;
      },
    });
    // 调色板断言就靠这两个字段被真实赋值
    return ctx as unknown as CanvasRenderingContext2D & { rec: Rec[] };
  }

  const palette = { accent: '#58a6ff', baseline: '#8b949e', up: '#f85149', down: '#3fb950' };
  const points = [
    { x: 0, y: 10 },
    { x: 40, y: 14 },
  ];

  function strokes(ctx: ReturnType<typeof fakeCtx>) {
    return ctx.rec.filter((r) => r.op === 'stroke').map((r) => r.detail);
  }

  it('axis 画 x 轴线与三个刻度，plain 一条都不画', () => {
    const base = {
      baselineY: null,
      points: [],
      dot: null,
      width: SPARKLINE_CANVAS_WIDTH,
      height: SPARKLINE_CANVAS_HEIGHT,
    };

    const axisCtx = fakeCtx();
    paintTickerSparkline(axisCtx, { ...base, style: 'axis' }, palette, 1);
    const axisLineMoves = axisCtx.rec.filter((r) => r.op === 'moveTo');
    // 轴线 1 段 + 刻度 3 段
    expect(axisLineMoves).toHaveLength(4);

    const plainCtx = fakeCtx();
    paintTickerSparkline(plainCtx, { ...base, style: 'plain' }, palette, 1);
    expect(plainCtx.rec.filter((r) => r.op === 'moveTo')).toHaveLength(0);
    expect(plainCtx.rec.filter((r) => r.op === 'stroke')).toHaveLength(0);
  });

  it('0% 线是虚线，价格线满不透明', () => {
    const ctx = fakeCtx();
    paintTickerSparkline(
      ctx,
      {
        style: 'plain',
        baselineY: sparklineBaselineY(),
        points,
        dot: null,
        width: SPARKLINE_CANVAS_WIDTH,
        height: SPARKLINE_CANVAS_HEIGHT,
      },
      palette,
      1,
    );

    const [baselineStroke, lineStroke] = strokes(ctx);
    expect(baselineStroke).toMatchObject({ alpha: 0.3 });
    expect((baselineStroke as { dash: number[] }).dash.length).toBeGreaterThan(0);
    expect(lineStroke).toMatchObject({ alpha: 1, lineWidth: 1.5 });
  });

  it('主线与末端圆点跟随当日方向着色，下方铺同色渐变', () => {
    const base = {
      style: 'plain' as const,
      baselineY: sparklineBaselineY(),
      width: SPARKLINE_CANVAS_WIDTH,
      height: SPARKLINE_CANVAS_HEIGHT,
    };

    const rising = fakeCtx();
    paintTickerSparkline(
      rising,
      {
        ...base,
        points,
        dot: { x: 40, y: 14, rising: true },
      },
      palette,
      1,
    );
    // 最后一条 stroke 是价格线（圆点是 fill）
    expect(strokes(rising).at(-1)).toMatchObject({
      color: palette.up,
      alpha: 1,
      lineWidth: 1.5,
    });
    const stops = rising.rec
      .filter((r) => r.op === 'colorStop')
      .map((r) => r.detail as [number, string]);
    expect(rising.rec.some((r) => r.op === 'gradient')).toBe(true);
    expect(stops).toHaveLength(2);
    expect(stops[0][1]).toContain('rgba(248, 81, 73');
    expect(rising.fillStyle).toBe(palette.up);

    const falling = fakeCtx();
    paintTickerSparkline(
      falling,
      {
        ...base,
        points,
        dot: { x: 40, y: 14, rising: false },
      },
      palette,
      1,
    );
    expect(strokes(falling).at(-1)).toMatchObject({ color: palette.down });
    expect(falling.fillStyle).toBe(palette.down);
  });

  it('末端圆点被夹进画布内，收盘时不缺半颗', () => {
    const ctx = fakeCtx();
    paintTickerSparkline(
      ctx,
      {
        style: 'plain',
        baselineY: sparklineBaselineY(),
        points: [{ x: SPARKLINE_CANVAS_WIDTH, y: 12 }],
        dot: { x: SPARKLINE_CANVAS_WIDTH, y: 12, rising: true },
        width: SPARKLINE_CANVAS_WIDTH,
        height: SPARKLINE_CANVAS_HEIGHT,
      },
      palette,
      1,
    );
    const arc = ctx.rec.find((r) => r.op === 'arc');
    expect((arc!.detail as number[])[0]).toBe(SPARKLINE_CANVAS_WIDTH - SPARKLINE_DOT_RADIUS);
    // 涨：用 --color-up
    expect(ctx.fillStyle).toBe(palette.up);
    // 只有一个点撑不出面积，不铺渐变
    expect(ctx.rec.some((r) => r.op === 'gradient')).toBe(false);
  });
});
