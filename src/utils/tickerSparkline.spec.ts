// src/utils/tickerSparkline.spec.ts
//
// 两种纵轴的边界都在这里钉住：price 模式的 ±1% 下限、limit 模式的固定半幅、
// 数据越界/规则未知时的回退，以及 09:30–15:00 的横向铺满。
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
  resolveSparklineLines,
  sparklineBaselineY,
  sparklineCanvasPoints,
  sparklinePoints,
  resolveSparklineRange,
} from './tickerSparkline';

function bar(close: number, hour = 9, minute = 30) {
  return { timestamp: new Date(2026, 8, 24, hour, minute).getTime(), close };
}

describe('resolveSparklineRange — price 模式', () => {
  it('数据波动小于 1% 时按昨收 ±1% 兜底', () => {
    const range = resolveSparklineRange([bar(100)], 100, 'price', null);
    expect(range.from).toBeCloseTo(99);
    expect(range.to).toBeCloseTo(101);
    expect(range.fellBack).toBe(false);
    expect(range.limitUp).toBeNull();
    expect(range.limitDown).toBeNull();
  });

  it('按数据最大偏离取半幅', () => {
    const range = resolveSparklineRange([bar(100), bar(103), bar(97)], 100, 'price', null);
    expect(range.from).toBeCloseTo(97);
    expect(range.to).toBeCloseTo(103);
  });

  it('prevClose 无效时用第一根有效 close 兜底', () => {
    const range = resolveSparklineRange([bar(50)], 0, 'price', null);
    expect(range.from).toBeCloseTo(49.5);
    expect(range.to).toBeCloseTo(50.5);
  });

  it('没有任何有效数据时返回空区间', () => {
    const range = resolveSparklineRange([], 0, 'price', null);
    expect(range).toEqual({ from: 0, to: 0, limitUp: null, limitDown: null, fellBack: false });
  });
});

describe('resolveSparklineRange — limit 模式', () => {
  it('10% 主板：半幅固定为涨停价到昨收的距离', () => {
    const range = resolveSparklineRange([bar(101)], 100, 'limit', 0.10);
    expect(range.from).toBeCloseTo(90);
    expect(range.to).toBeCloseTo(110);
    expect(range.limitUp).toBeCloseTo(110);
    expect(range.limitDown).toBeCloseTo(90);
    expect(range.fellBack).toBe(false);
  });

  it('5% ST / 20% 创业板 / 30% 北交所', () => {
    expect(resolveSparklineRange([bar(10)], 10, 'limit', 0.05).limitUp).toBeCloseTo(10.5);
    expect(resolveSparklineRange([bar(10)], 10, 'limit', 0.05).limitDown).toBeCloseTo(9.5);
    expect(resolveSparklineRange([bar(100)], 100, 'limit', 0.20).limitUp).toBeCloseTo(120);
    expect(resolveSparklineRange([bar(100)], 100, 'limit', 0.30).limitDown).toBeCloseTo(70);
  });

  it('数据超出涨跌停区间（新股/规则判错）时回退 price，不画假边界', () => {
    const range = resolveSparklineRange([bar(125)], 100, 'limit', 0.10);
    expect(range.fellBack).toBe(true);
    expect(range.limitUp).toBeNull();
    expect(range.limitDown).toBeNull();
    expect(range.from).toBeCloseTo(75);
    expect(range.to).toBeCloseTo(125);
  });

  it('比例未知或昨收无效时回退 price', () => {
    const unknown = resolveSparklineRange([bar(100)], 100, 'limit', null);
    expect(unknown.fellBack).toBe(true);
    expect(unknown.limitUp).toBeNull();

    const noPrevClose = resolveSparklineRange([bar(100)], 0, 'limit', 0.10);
    expect(noPrevClose.fellBack).toBe(true);
    expect(noPrevClose.limitUp).toBeNull();
  });

  it('涨跌停价四舍五入后两侧距离不等时，半幅取较大值保持 0% 居中', () => {
    // 10.05 × 1.1 = 11.06（+1.01）、× 0.9 = 9.05（-1.00）
    const range = resolveSparklineRange([bar(10.05)], 10.05, 'limit', 0.10);
    expect(range.limitUp).toBeCloseTo(11.06);
    expect(range.limitDown).toBeCloseTo(9.05);
    expect(range.from).toBeCloseTo(9.04);
    expect(range.to).toBeCloseTo(11.06);
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

describe('resolveSparklineLines — 一格要画哪些线', () => {
  const bars = [bar(101, 10, 0), bar(103, 11, 0), bar(99, 14, 0)];

  it('off 与无数据都不画', () => {
    expect(resolveSparklineLines(bars, 100, 'off', 0.1)).toEqual({
      main: null,
      limit: null,
    });
    expect(resolveSparklineLines([], 100, 'both', 0.1)).toEqual({
      main: null,
      limit: null,
    });
  });

  it('price 只有主线，没有涨跌停线', () => {
    const { main, limit } = resolveSparklineLines(bars, 100, 'price', 0.1);
    expect(main).not.toBeNull();
    expect(main!.from).toBeCloseTo(97);
    expect(main!.to).toBeCloseTo(103);
    expect(limit).toBeNull();
  });

  it('limit 单轴：涨跌停投影就是主线（满不透明），不再叠第二条', () => {
    const { main, limit } = resolveSparklineLines(bars, 100, 'limit', 0.1);
    expect(limit).toBeNull();
    expect(main!.from).toBeCloseTo(90);
    expect(main!.to).toBeCloseTo(110);
  });

  it('both：主线是价格轴，叠一条涨跌停轴投影', () => {
    const { main, limit } = resolveSparklineLines(bars, 100, 'both', 0.1);
    expect(main!.to).toBeCloseTo(103);
    expect(limit!.to).toBeCloseTo(110);
  });

  it('both 且涨跌停轴回退（规则未知 / 数据越界）时只画主线', () => {
    const unknownRate = resolveSparklineLines(bars, 100, 'both', null);
    expect(unknownRate.limit).toBeNull();
    expect(unknownRate.main).not.toBeNull();

    const outOfRange = resolveSparklineLines([bar(130)], 100, 'both', 0.1);
    expect(outOfRange.limit).toBeNull();
  });

  it('昨收无效时 limit 轴不可用，主线仍按数据画', () => {
    const { main, limit } = resolveSparklineLines(bars, 0, 'both', 0.1);
    expect(limit).toBeNull();
    expect(main).not.toBeNull();
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

  /**
   * 两套纵轴叠在同一格里能读的前提：昨收在两个坐标系里落在**同一个 y**。
   * 涨跌停价对称（半幅取较宽一侧），价格轴区间也以昨收为中心。
   */
  it('price 与 limit 两套纵轴下昨收映射到同一个 y', () => {
    const bars = [bar(101, 10, 0), bar(97, 14, 0)];
    const width = 88;
    for (const kind of ['price', 'limit', 'both'] as const) {
      const { main, limit } = resolveSparklineLines(bars, 100, kind, 0.1);
      for (const range of [main, limit]) {
        if (!range) continue;
        const points = sparklineCanvasPoints(bars, range, width);
        // 昨收落在 y = baseline：用一根正好等于昨收的 bar 验证
        const atPrev = sparklineCanvasPoints([bar(100, 10, 0)], range, width)[0];
        expect(atPrev.y).toBeCloseTo(sparklineBaselineY());
        expect(points.length).toBe(2);
      }
    }
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
    const state = { alpha: 1, lineWidth: 1, dash: [] as number[] };
    const ctx = {
      rec,
      strokeStyle: '',
      fillStyle: '',
      setTransform: () => rec.push({ op: 'setTransform' }),
      clearRect: () => rec.push({ op: 'clearRect' }),
      save: () => rec.push({ op: 'save' }),
      restore: () => rec.push({ op: 'restore' }),
      beginPath: () => rec.push({ op: 'beginPath' }),
      moveTo: (x: number, y: number) => rec.push({ op: 'moveTo', detail: [x, y] }),
      lineTo: (x: number, y: number) => rec.push({ op: 'lineTo', detail: [x, y] }),
      stroke: () =>
        rec.push({
          op: 'stroke',
          detail: { alpha: state.alpha, lineWidth: state.lineWidth, dash: [...state.dash] },
        }),
      arc: (x: number, y: number, r: number) => rec.push({ op: 'arc', detail: [x, y, r] }),
      fill: () => rec.push({ op: 'fill' }),
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
      under: [],
      over: [],
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

  it('0% 线是虚线，涨跌停那条是 50% 透明，主线满不透明', () => {
    const ctx = fakeCtx();
    paintTickerSparkline(
      ctx,
      {
        style: 'plain',
        baselineY: sparklineBaselineY(),
        under: points,
        over: points,
        dot: null,
        width: SPARKLINE_CANVAS_WIDTH,
        height: SPARKLINE_CANVAS_HEIGHT,
      },
      palette,
      1,
    );

    const [baselineStroke, underStroke, overStroke] = strokes(ctx);
    expect(baselineStroke).toMatchObject({ alpha: 0.3 });
    expect((baselineStroke as { dash: number[] }).dash.length).toBeGreaterThan(0);
    expect(underStroke).toMatchObject({ alpha: 0.5, lineWidth: 1 });
    expect(overStroke).toMatchObject({ alpha: 1, lineWidth: 1.25 });
  });

  it('单独选涨跌停时那条线不再半透明（它就是主线）', () => {
    const ctx = fakeCtx();
    paintTickerSparkline(
      ctx,
      {
        style: 'plain',
        baselineY: null,
        under: points,
        over: [],
        dot: null,
        width: SPARKLINE_CANVAS_WIDTH,
        height: SPARKLINE_CANVAS_HEIGHT,
      },
      palette,
      1,
    );
    // 没有 0% 线（baselineY=null），只剩一条曲线，满不透明
    expect(strokes(ctx)).toEqual([expect.objectContaining({ alpha: 1 })]);
  });

  it('末端圆点被夹进画布内，收盘时不缺半颗', () => {
    const ctx = fakeCtx();
    paintTickerSparkline(
      ctx,
      {
        style: 'plain',
        baselineY: sparklineBaselineY(),
        under: [],
        over: [{ x: SPARKLINE_CANVAS_WIDTH, y: 12 }],
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
  });
});
