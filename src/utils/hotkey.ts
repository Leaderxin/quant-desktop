/**
 * 老板键的加速键字符串。
 *
 * 词表与 tauri-plugin-global-shortcut 的解析器一一对应：它认的是 global-hotkey
 * 那套 `Code` 名（KeyA / Digit1 / F5 / ArrowUp …），而这套名字与浏览器的
 * `KeyboardEvent.code` 几乎逐字相同 —— 所以录制时直接取 `e.code`，不做映射表。
 * 两端必须对齐：这里放行、Rust 那边解析不了的值，症状是「录进去了，按了没反应」，
 * 而设置页上显示得好好的。
 */

/** 修饰键的写法与固定顺序。顺序固定是因为「是不是同一个键」只能靠字符串比 ——
 *  同一组按键按下先后不同就会产出不同字符串的话，比对就成了肉眼活。 */
export const MODIFIERS = ['Ctrl', 'Alt', 'Shift', 'Super'] as const;

export type Modifier = (typeof MODIFIERS)[number];

const MODIFIER_FLAG: Record<Modifier, (e: KeyboardEvent) => boolean> = {
  Ctrl: (e) => e.ctrlKey,
  Alt: (e) => e.altKey,
  Shift: (e) => e.shiftKey,
  // 键盘上的 Win / Cmd 键。解析端的写法是 Super（别名 Command / Cmd）。
  Super: (e) => e.metaKey,
};

/** `KeyA`–`KeyZ`、`Digit0`–`Digit9`、`Numpad*`、`F1`–`F24`，加上这些零散的。 */
const NAMED_CODES = [
  'Backquote', 'Backslash', 'BracketLeft', 'BracketRight', 'Comma', 'Equal',
  'Minus', 'Period', 'Quote', 'Semicolon', 'Slash',
  'Backspace', 'CapsLock', 'Enter', 'Space', 'Tab',
  'Delete', 'End', 'Home', 'Insert', 'PageDown', 'PageUp',
  'PrintScreen', 'ScrollLock', 'Pause', 'NumLock', 'Escape',
  'ArrowDown', 'ArrowLeft', 'ArrowRight', 'ArrowUp',
  'AudioVolumeDown', 'AudioVolumeUp', 'AudioVolumeMute',
  'MediaPlay', 'MediaPause', 'MediaPlayPause', 'MediaStop',
  'MediaTrackNext', 'MediaTrackPrevious',
];

function buildSupportedCodes(): Set<string> {
  const codes = new Set(NAMED_CODES);
  for (let i = 0; i < 26; i++) codes.add(`Key${String.fromCharCode(65 + i)}`);
  for (let i = 0; i <= 9; i++) {
    codes.add(`Digit${i}`);
    codes.add(`Numpad${i}`);
  }
  for (const extra of ['Add', 'Decimal', 'Divide', 'Enter', 'Equal', 'Multiply', 'Subtract']) {
    codes.add(`Numpad${extra}`);
  }
  for (let i = 1; i <= 24; i++) codes.add(`F${i}`);
  return codes;
}

/** 解析端认得的全部主键。 */
export const SUPPORTED_CODES: ReadonlySet<string> = buildSupportedCodes();

/**
 * 允许**不带修饰键**单独使用的主键：功能键、PrintScreen、媒体/音量键。
 *
 * 这条限制不是洁癖：全局热键注册之后是全系统生效的。把裸的 `KeyH` 设成老板键，
 * 用户在任何输入框里都再也打不出 h —— 而带一个修饰键就撞不上正常打字。
 * 功能键与媒体键本来就是「单独按」的键，没有这个问题。
 */
function canStandAlone(code: string): boolean {
  return (
    /^F([1-9]|1\d|2[0-4])$/.test(code) ||
    code === 'PrintScreen' ||
    code.startsWith('Media') ||
    code.startsWith('AudioVolume')
  );
}

/**
 * 把一次按键翻译成加速键字符串；不是一组能用的组合就返回 `null`。
 *
 * 纯修饰键（ControlLeft 之类）、词表外的主键、以及不带修饰键的普通字符键都
 * 会被拒掉 —— 见 `canStandAlone`。
 */
export function acceleratorFromEvent(e: KeyboardEvent): string | null {
  const code = e.code;
  if (!SUPPORTED_CODES.has(code)) return null;

  const mods = MODIFIERS.filter((m) => MODIFIER_FLAG[m](e));
  if (mods.length === 0 && !canStandAlone(code)) return null;

  return [...mods, code].join('+');
}

/** 校验一个存下来的字符串是不是本应用认得的加速键（读旧库/坏值时用）。 */
export function isValidAccelerator(accelerator: string): boolean {
  return parseAccelerator(accelerator) !== null;
}

/** 拆开加速键；不认识就返回 `null`。 */
function parseAccelerator(accelerator: string): { mods: Modifier[]; code: string } | null {
  const tokens = accelerator.split('+').map((t) => t.trim());
  if (tokens.length === 0 || tokens.some((t) => t === '')) return null;

  const code = tokens[tokens.length - 1];
  const mods = tokens.slice(0, -1);

  if (!SUPPORTED_CODES.has(code)) return null;
  // 主键写在最后，修饰键不能重复 —— 解析端对 "Ctrl+A+Shift" 也是直接报错。
  if (new Set(mods).size !== mods.length) return null;
  if (!mods.every((m) => (MODIFIERS as readonly string[]).includes(m))) return null;
  if (mods.length === 0 && !canStandAlone(code)) return null;

  return { mods: mods as Modifier[], code };
}

/** 界面上的写法。认不出的字符串原样返回 —— 显示总比空着强。 */
const DISPLAY: Record<string, string> = {
  Ctrl: 'Ctrl',
  Alt: 'Alt',
  Shift: 'Shift',
  Super: 'Win',
  ArrowUp: '↑',
  ArrowDown: '↓',
  ArrowLeft: '←',
  ArrowRight: '→',
  Space: '空格',
  Enter: 'Enter',
  NumpadEnter: '小键盘 Enter',
  Escape: 'Esc',
  Backspace: '退格',
  Delete: 'Del',
  Insert: 'Ins',
  PageUp: 'PgUp',
  PageDown: 'PgDn',
  CapsLock: 'CapsLock',
  Backquote: '`',
  Minus: '-',
  Equal: '=',
  BracketLeft: '[',
  BracketRight: ']',
  Backslash: '\\',
  Semicolon: ';',
  Quote: "'",
  Comma: ',',
  Period: '.',
  Slash: '/',
};

function formatToken(token: string): string {
  if (DISPLAY[token]) return DISPLAY[token];
  if (token.startsWith('Key')) return token.slice(3);
  if (token.startsWith('Digit')) return token.slice(5);
  if (token.startsWith('Numpad')) return `小键盘 ${token.slice(6)}`;
  return token;
}

export function formatAccelerator(accelerator: string): string {
  if (!accelerator) return '';
  return accelerator.split('+').map(formatToken).join(' + ');
}
