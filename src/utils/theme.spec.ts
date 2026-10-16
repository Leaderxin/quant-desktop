// @vitest-environment happy-dom
//
// 「跟随系统」的窗口侧解析。
//
// 两件事在界面上都看不出来：
// 1. 显式模式**不看**系统深浅 —— 写成 `mode === 'system' ? sys : mode` 之外的花样
//    （例如先算系统值再覆盖）在本机测不出来，只会在别人机器上闪一下；
// 2. matchMedia 可能不存在（测试环境、老 webview）。读不到就按浅色，不能让主题
//    解析抛错 —— 它跑在 store 的建立路径上，抛了就是整个界面白屏。
import { describe, it, expect, vi, afterEach } from 'vitest';
import { DARK_SCHEME_QUERY, resolveTheme, systemPrefersDark, watchSystemTheme } from './theme';

type Listener = (e: { matches: boolean }) => void;

/** 假的 matchMedia。happy-dom 的实现只按宽度判定媒体查询，深色要自己造。 */
function stubMatchMedia(initialDark: boolean) {
  const listeners = new Set<Listener>();
  const mql = {
    matches: initialDark,
    media: DARK_SCHEME_QUERY,
    addEventListener: (_type: string, cb: Listener) => { listeners.add(cb); },
    removeEventListener: (_type: string, cb: Listener) => { listeners.delete(cb); },
  };
  const fn = vi.fn(() => mql);
  window.matchMedia = fn as unknown as typeof window.matchMedia;
  return {
    query: fn,
    /** 模拟系统主题变化：改值并通知所有订阅者。 */
    change(dark: boolean) {
      mql.matches = dark;
      for (const cb of listeners) cb({ matches: dark });
    },
    listenerCount: () => listeners.size,
  };
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe('resolveTheme', () => {
  it('显式模式直接返回模式本身', () => {
    expect(resolveTheme('dark', false)).toBe('dark');
    expect(resolveTheme('light', true)).toBe('light');
  });

  it('跟随系统时才看系统深浅', () => {
    expect(resolveTheme('system', true)).toBe('dark');
    expect(resolveTheme('system', false)).toBe('light');
  });

  /** 解析结果只该是这两种 —— 漏一个分支就会把 "system" 当成颜色用（= 亮色）。 */
  it('结果里永远不会出现 system', () => {
    for (const mode of ['system', 'dark', 'light'] as const) {
      for (const dark of [true, false]) {
        expect(['dark', 'light']).toContain(resolveTheme(mode, dark));
      }
    }
  });
});

describe('systemPrefersDark', () => {
  it('按媒体的判定取值', () => {
    stubMatchMedia(true);
    expect(systemPrefersDark()).toBe(true);
    stubMatchMedia(false);
    expect(systemPrefersDark()).toBe(false);
  });

  it('没有 matchMedia 时按浅色，不抛错', () => {
    // @ts-expect-error 故意抹掉，模拟老 webview / 无 DOM 环境
    window.matchMedia = undefined;
    expect(systemPrefersDark()).toBe(false);
  });
});

describe('watchSystemTheme', () => {
  it('系统主题变化时回调，用注册时那个查询', () => {
    const media = stubMatchMedia(false);
    const onChange = vi.fn();

    watchSystemTheme(onChange);
    expect(media.query).toHaveBeenCalledWith(DARK_SCHEME_QUERY);

    media.change(true);
    expect(onChange).toHaveBeenCalledWith(true);

    media.change(false);
    expect(onChange).toHaveBeenLastCalledWith(false);
  });

  it('返回的函数真的摘掉监听', () => {
    const media = stubMatchMedia(false);
    const onChange = vi.fn();

    const unwatch = watchSystemTheme(onChange);
    expect(media.listenerCount()).toBe(1);
    unwatch();
    expect(media.listenerCount()).toBe(0);

    media.change(true);
    expect(onChange).not.toHaveBeenCalled();
  });

  it('没有 matchMedia 时退化成空操作，取消订阅也能调', () => {
    // @ts-expect-error 见上
    window.matchMedia = undefined;
    const unwatch = watchSystemTheme(() => {});
    expect(() => unwatch()).not.toThrow();
  });
});
