// 前端日志落盘。
//
// release 包里 webview 没有 devtools，`console.*` 只存在于内存 —— 用户报障时
// 我们拿不到任何界面侧的证据。这里把 error/warn 与全局未捕获异常经
// `log_event` 送进后端日志文件（带 `[web]` 前缀），同时保留原来的 console
// 输出，开发时照旧在控制台可见。
//
// 只接管 error / warn：info/log/debug 大多是开发期噪音，全量转发会把日志
// 文件变成另一份控制台。
import { invoke } from '@tauri-apps/api/core';

/** 同一条日志在这个窗口内不重复转发（毫秒）。轮询失败的同一句话不该刷满文件。 */
export const REPEAT_WINDOW_MS = 5000;
/** 单条日志最长字符数；后端还会再截一次，两边都对多字节字符安全。 */
export const MAX_MESSAGE_CHARS = 4000;
/** 去重表的上限；超过就整体清空，避免长期运行把内存攒成一张错误表。 */
const GATE_CAPACITY = 200;

export type WebLogLevel = 'error' | 'warn' | 'info' | 'debug';
export type WebLogSource = 'main' | 'ticker';

/** 两个窗口共用一份日志文件；从页面路径判断当前是哪一个。 */
export function windowSource(pathname: string): WebLogSource {
  return /(^|\/)ticker\.html$/.test(pathname) ? 'ticker' : 'main';
}

/** 按字符截断，避免把 emoji / 代理对切一半。 */
export function truncateMessage(
  message: string,
  max = MAX_MESSAGE_CHARS,
): string {
  if (message.length <= max) return message;
  return Array.from(message).slice(0, max).join('');
}

/** 把 console 参数拼成一段可读文本。Error 保留堆栈，对象尽量 JSON 化。 */
export function formatLogArgs(args: unknown[]): string {
  return args.map(formatArg).join(' ');
}

function formatArg(value: unknown): string {
  if (typeof value === 'string') return value;
  if (value instanceof Error) {
    return value.stack ?? `${value.name}: ${value.message}`;
  }
  if (typeof value === 'object' && value !== null) {
    try {
      // toJSON 返回 undefined 时 JSON.stringify 也返回 undefined，不能直接当字符串
      return JSON.stringify(value) ?? String(value);
    } catch {
      // 循环引用 / BigInt 等无法序列化的值：退回 String，不能因为日志本身抛错
      return String(value);
    }
  }
  return String(value);
}

/**
 * 建一个按 key 去重的放行器。
 *
 * `now` 由调用方传入，方便测试；同一 key 在 `windowMs` 内只放行第一次。
 */
export function createLogGate(windowMs = REPEAT_WINDOW_MS) {
  const seen = new Map<string, number>();
  return function allow(key: string, now: number): boolean {
    const prev = seen.get(key);
    if (prev !== undefined && now - prev < windowMs) return false;
    if (seen.size >= GATE_CAPACITY) seen.clear();
    seen.set(key, now);
    return true;
  };
}

/** 转发到后端的实现；测试里注入假的，避免依赖 Tauri 运行时。 */
export type LogForwarder = (level: WebLogLevel, message: string) => void;

/**
 * 建一个带前缀与去重的日志函数。
 *
 * `source` 区分主窗口与行情条（两份 webview 共用一个日志文件）；
 * 同一个 `level:message` 在窗口内只转发一次 —— 轮询失败时同一句话不该刷满文件。
 */
export function createWebLogger(
  source: string,
  forward: LogForwarder,
): (level: WebLogLevel, message: string) => void {
  const allow = createLogGate();
  return (level, message) => {
    const text = `[${source}] ${truncateMessage(message)}`;
    if (!allow(`${level}:${text}`, Date.now())) return;
    forward(level, text);
  };
}

/**
 * 安装日志桥。入口（主窗口 / 行情条）各调一次，`source` 用来区分两个窗口。
 */
export function installWebLogging(source: WebLogSource = 'main'): void {
  // HMR 会重新执行入口模块；重复接管会让每条日志转发两次（去重表也是各一份）。
  const host = window as unknown as Record<string, boolean>;
  if (host.__quantWebLogInstalled) return;
  host.__quantWebLogInstalled = true;

  // 失败只能吞掉 —— 再从 console 报错会绕回转发，形成递归。
  const log = createWebLogger(source, (level, message) => {
    void invoke('log_event', { level, message }).catch(() => {});
  });

  const originalError = console.error.bind(console);
  const originalWarn = console.warn.bind(console);

  console.error = (...args: unknown[]) => {
    log('error', formatLogArgs(args));
    originalError(...args);
  };
  console.warn = (...args: unknown[]) => {
    log('warn', formatLogArgs(args));
    originalWarn(...args);
  };

  // Vue 组件内的异常由 App.vue 的 onErrorCaptured 记 console.error；
  // 这两条管的是组件之外（事件回调、异步链、资源加载）的漏网之鱼。
  window.addEventListener('error', (e) => {
    log(
      'error',
      `uncaught: ${e.message} @ ${e.filename}:${e.lineno}:${e.colno}`,
    );
  });
  window.addEventListener('unhandledrejection', (e) => {
    log('error', `unhandled rejection: ${formatLogArgs([e.reason])}`);
  });
}
