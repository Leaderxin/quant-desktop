/**
 * 空闲时预取异步 chunk。
 *
 * 详情/设置页改成 `defineAsyncComponent` 后，chunk 不再进首屏，但首次渲染要等它
 * 加载 + 解析。在首屏画完之后用空闲时间把它取回来，两个目标都能拿到：
 * 首屏不被 klinecharts 拖慢，用户点开时又几乎不用等。
 *
 * 预取失败不处理 —— 真正渲染时的 import() 会再试一次，并由 async 组件报错。
 */
export function prefetchWhenIdle(loader: () => Promise<unknown>): void {
  const run = () => {
    void loader().catch(() => {});
  };
  // WebKitGTK / 部分 WKWebView 版本没有 requestIdleCallback，退化为延时预取
  const idle = (window as Window & {
    requestIdleCallback?: (cb: () => void, opts?: { timeout: number }) => number;
  }).requestIdleCallback;
  if (typeof idle === 'function') {
    idle(run, { timeout: 3000 });
  } else {
    window.setTimeout(run, 1500);
  }
}
