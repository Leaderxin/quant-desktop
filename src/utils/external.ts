// src/utils/external.ts
//
// 用系统默认应用打开外部链接（浏览器开 http(s)、Microsoft Store 开
// ms-windows-store:）。webview 内部跳转会丢掉原生外壳，商店这类协议
// webview 根本处理不了，所以外链一律经 plugin-opener 交给系统。
// 状态栏、更新弹窗与「关于」页共用 —— 各写一份迟早漏改一处。

export async function openExternal(url: string): Promise<void> {
  const { openUrl } = await import('@tauri-apps/plugin-opener');
  await openUrl(url);
}
