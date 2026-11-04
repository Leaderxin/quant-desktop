// @vitest-environment happy-dom
import { describe, expect, it, vi } from 'vitest';
import { createApp, defineComponent, h, nextTick } from 'vue';
import { useEscapeKey } from './useEscapeKey';

/**
 * 与 useTickerWindowHeight.spec.ts 同一思路：挂一个真的 Vue 组件，
 * 验证 composable 自己注册/摘除监听的生命周期。
 */

function mountWithEscape(handler: () => void) {
  const Comp = defineComponent({
    setup() {
      useEscapeKey(handler);
      return () => h('div');
    },
  });
  const host = document.createElement('div');
  document.body.appendChild(host);
  const app = createApp(Comp);
  app.mount(host);
  return { app, host };
}

function press(key: string, init: KeyboardEventInit = {}) {
  window.dispatchEvent(new KeyboardEvent('keydown', { key, ...init }));
}

describe('useEscapeKey', () => {
  it('只响应 Escape；输入法组合中的 Esc 不算', async () => {
    const handler = vi.fn();
    const { app, host } = mountWithEscape(handler);
    await nextTick();

    press('Enter');
    press('Escape', { isComposing: true });
    expect(handler).not.toHaveBeenCalled();

    press('Escape');
    expect(handler).toHaveBeenCalledTimes(1);

    app.unmount();
    host.remove();
  });

  it('卸载后摘掉监听，不再响应', async () => {
    const handler = vi.fn();
    const { app, host } = mountWithEscape(handler);
    await nextTick();

    app.unmount();
    press('Escape');
    expect(handler).not.toHaveBeenCalled();

    host.remove();
  });
});
