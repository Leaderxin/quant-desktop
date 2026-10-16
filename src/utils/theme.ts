// src/utils/theme.ts
//
// 「跟随系统」的窗口侧：读 `prefers-color-scheme`、订阅它的变化。
//
// ⚠ 这个媒体查询**只在跟随系统时可信**。显式主题会把窗口主题钉住，tao 收到钉住的
// 主题后连 WebView 的 `prefers-color-scheme` 一起冻在应用自己的主题上 —— 也就是
// 说，显式亮色时读它会得到「系统=浅色」，哪怕系统是深色。完整链条见
// src-tauri/src/lib.rs 的 set_app_theme；调用方（stores/settings.ts）据此只在
// 跟随系统时采纳它的值。

import type { ThemeMode } from './prefs';

/** 解析出来的实际主题 —— 只可能是这两种，不会再出现 "system"。 */
export type ResolvedTheme = 'dark' | 'light';

/** 深浅判定用的媒体查询。读写两端只此一处。 */
export const DARK_SCHEME_QUERY = '(prefers-color-scheme: dark)';

/** 系统当前是不是深色。没有 matchMedia（测试环境、老 webview）时按浅色。 */
export function systemPrefersDark(): boolean {
  if (typeof window === 'undefined') return false;
  return window.matchMedia?.(DARK_SCHEME_QUERY)?.matches ?? false;
}

/**
 * 模式 + 系统深浅 → 实际主题。
 *
 * 显式模式**不看**系统值，所以调用方不需要自己判断「现在是不是跟随系统」——
 * 少一个判断就少一处「忘记判断」。
 */
export function resolveTheme(mode: ThemeMode, systemDark: boolean): ResolvedTheme {
  if (mode !== 'system') return mode;
  return systemDark ? 'dark' : 'light';
}

/**
 * 订阅系统主题变化，返回取消订阅。
 *
 * 事件本身不做过滤：显式模式下它也会来（那是应用自己钉出来的变化，切模式那一下
 * 就会触发一次），而「要不要采纳」只有 store 知道 —— 见 stores/settings.ts。
 */
export function watchSystemTheme(onChange: (dark: boolean) => void): () => void {
  if (typeof window === 'undefined') return () => {};
  const mql = window.matchMedia?.(DARK_SCHEME_QUERY);
  if (!mql) return () => {};
  const handler = (e: MediaQueryListEvent) => onChange(e.matches);
  mql.addEventListener('change', handler);
  return () => mql.removeEventListener('change', handler);
}
