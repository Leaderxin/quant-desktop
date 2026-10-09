// src/utils/hotkey.spec.ts
//
// 老板键加速键字符串的两端约定。这里钉住的是「录制出来的字符串，Rust 那边
// tauri-plugin-global-shortcut 必须认」—— 词表是两边各写一份，错了不会报错，
// 只会表现为「录进去了，按了没反应」，而设置页上显示得好好的。
//
// 顺带钉住两条不是洁癖的规矩：修饰键顺序固定（否则「同一个键」没法按字符串比），
// 以及不在打字路径上的组合才许当热键 —— 裸键挡小写、只带 Shift 的组合挡大写
// 与上档标点（KeyH 是小写 h、Shift+KeyH 是大写 H、Shift+Digit1 是「!」），全局
// 热键都会把它从全系统抢走。
import { describe, expect, it } from 'vitest';
import { acceleratorFromEvent, formatAccelerator, isValidAccelerator } from './hotkey';

/** 只喂录制函数真正读的那几个字段，省掉整个 DOM 环境。 */
function key(init: Partial<KeyboardEvent> & { code: string }): KeyboardEvent {
  return { altKey: false, ctrlKey: false, shiftKey: false, metaKey: false, ...init } as KeyboardEvent;
}

describe('acceleratorFromEvent', () => {
  it('拼出修饰键 + 主键', () => {
    expect(acceleratorFromEvent(key({ code: 'KeyH', ctrlKey: true }))).toBe('Ctrl+KeyH');
    expect(acceleratorFromEvent(key({ code: 'F5', altKey: true }))).toBe('Alt+F5');
    expect(acceleratorFromEvent(key({ code: 'Digit1', ctrlKey: true, shiftKey: true })))
      .toBe('Ctrl+Shift+Digit1');
  });

  it('修饰键顺序固定为 Ctrl/Alt/Shift/Super，与按下的先后无关', () => {
    const both = { code: 'KeyB', ctrlKey: true, shiftKey: true };
    expect(acceleratorFromEvent(key(both))).toBe('Ctrl+Shift+KeyB');
    expect(acceleratorFromEvent(key({ code: 'KeyB', shiftKey: true, altKey: true, ctrlKey: true })))
      .toBe('Ctrl+Alt+Shift+KeyB');
  });

  it('Win/Cmd 键记作 Super', () => {
    expect(acceleratorFromEvent(key({ code: 'KeyB', metaKey: true }))).toBe('Super+KeyB');
  });

  it('纯修饰键不是一组组合键', () => {
    expect(acceleratorFromEvent(key({ code: 'ControlLeft', ctrlKey: true }))).toBeNull();
    expect(acceleratorFromEvent(key({ code: 'MetaLeft', metaKey: true }))).toBeNull();
    expect(acceleratorFromEvent(key({ code: 'ShiftRight', shiftKey: true }))).toBeNull();
  });

  it('裸的普通字符键会被拒掉 —— 注册上去就等于全系统抢走这个键', () => {
    expect(acceleratorFromEvent(key({ code: 'KeyH' }))).toBeNull();
    expect(acceleratorFromEvent(key({ code: 'Digit1' }))).toBeNull();
    expect(acceleratorFromEvent(key({ code: 'Space' }))).toBeNull();
    expect(acceleratorFromEvent(key({ code: 'ArrowUp' }))).toBeNull();
  });

  it('只带 Shift 的组合同样拒掉 —— 那是大写字母与上档标点的输入路径', () => {
    // Shift+KeyH 就是输入大写 H，Shift+Digit1 就是「!」。RegisterHotKey(MOD_SHIFT)
    // 全系统生效，注册后这些字符在任何应用的输入框里都打不出来了。
    expect(acceleratorFromEvent(key({ code: 'KeyH', shiftKey: true }))).toBeNull();
    expect(acceleratorFromEvent(key({ code: 'Digit1', shiftKey: true }))).toBeNull();
    expect(acceleratorFromEvent(key({ code: 'Space', shiftKey: true }))).toBeNull();
  });

  it('Shift 与 Ctrl/Alt/Super 同用没问题；Shift+功能键不在打字路径上，放行', () => {
    expect(acceleratorFromEvent(key({ code: 'KeyH', ctrlKey: true, shiftKey: true }))).toBe('Ctrl+Shift+KeyH');
    expect(acceleratorFromEvent(key({ code: 'F5', shiftKey: true }))).toBe('Shift+F5');
  });

  it('功能键与媒体键可以单独用', () => {
    expect(acceleratorFromEvent(key({ code: 'F5' }))).toBe('F5');
    expect(acceleratorFromEvent(key({ code: 'F24' }))).toBe('F24');
    expect(acceleratorFromEvent(key({ code: 'PrintScreen' }))).toBe('PrintScreen');
    expect(acceleratorFromEvent(key({ code: 'AudioVolumeMute' }))).toBe('AudioVolumeMute');
  });

  it('解析端不认的主键返回 null', () => {
    // IntlBackslash / ContextMenu / NumpadComma 不在 global-hotkey 的词表里
    expect(acceleratorFromEvent(key({ code: 'IntlBackslash', ctrlKey: true }))).toBeNull();
    expect(acceleratorFromEvent(key({ code: 'ContextMenu', ctrlKey: true }))).toBeNull();
    expect(acceleratorFromEvent(key({ code: 'NumpadComma', ctrlKey: true }))).toBeNull();
  });
});

describe('isValidAccelerator', () => {
  it('接受录制端能产出的写法', () => {
    expect(isValidAccelerator('Ctrl+Shift+KeyH')).toBe(true);
    expect(isValidAccelerator('Ctrl+Digit1')).toBe(true);
    expect(isValidAccelerator('Alt+NumpadAdd')).toBe(true);
    expect(isValidAccelerator('F5')).toBe(true);
  });

  it('拒掉空串、缺主键、词表外的键、重复修饰键', () => {
    expect(isValidAccelerator('')).toBe(false);
    expect(isValidAccelerator('Ctrl+Shift')).toBe(false); // 只有修饰键
    expect(isValidAccelerator('Ctrl+')).toBe(false);
    expect(isValidAccelerator('Ctrl+Whatever')).toBe(false);
    expect(isValidAccelerator('Ctrl+Ctrl+KeyA')).toBe(false);
    expect(isValidAccelerator('KeyH')).toBe(false); // 裸字符键，与录制端同一条规矩
    expect(isValidAccelerator('Meta+KeyH')).toBe(false); // 写法是 Super，不是 Meta
  });

  it('只带 Shift 的组合拒掉、Shift+功能键放行 —— 与录制端同一条规矩', () => {
    expect(isValidAccelerator('Shift+KeyH')).toBe(false);
    expect(isValidAccelerator('Shift+Digit1')).toBe(false);
    expect(isValidAccelerator('Shift+F5')).toBe(true);
  });
});

describe('formatAccelerator', () => {
  it('按界面写法拼接', () => {
    expect(formatAccelerator('Ctrl+Shift+KeyH')).toBe('Ctrl + Shift + H');
    expect(formatAccelerator('Ctrl+Digit1')).toBe('Ctrl + 1');
    expect(formatAccelerator('Super+KeyB')).toBe('Win + B');
  });

  it('方向键与功能键用符号/原名', () => {
    expect(formatAccelerator('Ctrl+ArrowUp')).toBe('Ctrl + ↑');
    expect(formatAccelerator('Alt+F4')).toBe('Alt + F4');
    expect(formatAccelerator('Ctrl+Numpad1')).toBe('Ctrl + 小键盘 1');
  });

  it('空串是空串（界面据此显示「未设置」）', () => {
    expect(formatAccelerator('')).toBe('');
  });
});
