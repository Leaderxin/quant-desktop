// src/utils/webLog.spec.ts
//
// 前端日志桥的纯逻辑：参数怎么拼、多长算过长、重复的同一句话怎么抑制。
// 真正把日志写进文件的是后端 `logging::log_from_web`，由 Rust 侧测试覆盖。
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  MAX_MESSAGE_CHARS,
  createLogGate,
  createWebLogger,
  formatLogArgs,
  truncateMessage,
  windowSource,
} from './webLog';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue(undefined),
}));

afterEach(() => {
  vi.useRealTimers();
});

describe('formatLogArgs', () => {
  it('把多个参数拼成一行', () => {
    expect(formatLogArgs(['a', 1, true])).toBe('a 1 true');
  });

  it('Error 带上堆栈（至少带消息）', () => {
    const text = formatLogArgs([new Error('boom')]);
    expect(text).toContain('boom');
  });

  it('对象尽量 JSON 化', () => {
    expect(formatLogArgs([{ code: '600519', name: '贵州茅台' }]))
      .toBe('{"code":"600519","name":"贵州茅台"}');
  });

  it('循环引用不抛错', () => {
    const cyc: Record<string, unknown> = {};
    cyc.self = cyc;
    expect(() => formatLogArgs([cyc])).not.toThrow();
    expect(formatLogArgs([cyc])).toBe('[object Object]');
  });

  it('JSON.stringify 返回 undefined 时仍给出可读文本', () => {
    // toJSON 主动返回 undefined 的对象：直接拿 JSON.stringify 的结果会拼出空串
    expect(formatLogArgs([{ toJSON: () => undefined }])).toBe('[object Object]');
  });

  it('null / undefined 也能落成文字', () => {
    expect(formatLogArgs([null, undefined])).toBe('null undefined');
  });
});

describe('truncateMessage', () => {
  it('短消息原样保留', () => {
    expect(truncateMessage('hello', 10)).toBe('hello');
  });

  it('默认上限与后端对齐', () => {
    const long = 'x'.repeat(MAX_MESSAGE_CHARS + 10);
    expect(truncateMessage(long)).toHaveLength(MAX_MESSAGE_CHARS);
  });

  it('按字符截断，不切坏多字节字符', () => {
    expect(truncateMessage('日志记录', 2)).toBe('日志');
    // emoji 在 UTF-16 里占两个单元；按单元切会留下半个代理对
    const emoji = '😀'.repeat(5);
    const cut = truncateMessage(emoji, 3);
    expect(cut).toBe('😀😀😀');
    expect(cut.length).toBe(6);
  });
});

describe('createLogGate', () => {
  it('同一 key 在窗口内只放行一次，窗口过后再次放行', () => {
    const allow = createLogGate(5000);
    const start = 1_000_000;
    expect(allow('error:boom', start)).toBe(true);
    expect(allow('error:boom', start + 4999)).toBe(false);
    expect(allow('error:boom', start + 5000)).toBe(true);
  });

  it('不同 key 互不影响', () => {
    const allow = createLogGate(5000);
    expect(allow('error:a', 0)).toBe(true);
    expect(allow('error:b', 0)).toBe(true);
  });

  it('窗口长度可配', () => {
    const allow = createLogGate(1000);
    expect(allow('warn:x', 0)).toBe(true);
    expect(allow('warn:x', 999)).toBe(false);
    expect(allow('warn:x', 1000)).toBe(true);
  });
});

describe('windowSource', () => {
  it('按页面路径区分主窗口与行情条', () => {
    expect(windowSource('/ticker.html')).toBe('ticker');
    expect(windowSource('/index.html')).toBe('main');
    expect(windowSource('/')).toBe('main');
    // 只是名字里带 ticker 的其它页面不能误判
    expect(windowSource('/guide/ticker-notes.html')).toBe('main');
  });
});

describe('createWebLogger', () => {
  it('加窗口前缀后转发级别与文本', () => {
    const forward = vi.fn();
    const log = createWebLogger('ticker', forward);
    log('warn', 'height sync skipped');
    expect(forward).toHaveBeenCalledWith('warn', '[ticker] height sync skipped');
  });

  it('同一句话在去重窗口内只转发一次', () => {
    vi.useFakeTimers();
    vi.setSystemTime(0);
    const forward = vi.fn();
    const log = createWebLogger('main', forward);

    log('error', 'boom');
    log('error', 'boom');
    expect(forward).toHaveBeenCalledTimes(1);

    vi.setSystemTime(5000);
    log('error', 'boom');
    expect(forward).toHaveBeenCalledTimes(2);
  });

  it('不同级别与不同文本互不影响', () => {
    vi.useFakeTimers();
    vi.setSystemTime(0);
    const forward = vi.fn();
    const log = createWebLogger('main', forward);

    log('error', 'boom');
    log('warn', 'boom');
    log('error', 'other');
    expect(forward).toHaveBeenCalledTimes(3);
  });

  it('转发前按字符截断', () => {
    const forward = vi.fn();
    const log = createWebLogger('main', forward);
    log('error', 'x'.repeat(MAX_MESSAGE_CHARS + 5));
    const [, text] = forward.mock.calls[0];
    expect(text).toHaveLength('[main] '.length + MAX_MESSAGE_CHARS);
  });
});
