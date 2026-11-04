import { onMounted, onUnmounted } from 'vue';

/**
 * 详情面板的 Esc 关闭。
 *
 * 监听挂在 window 上而不是面板元素上：表格行点开后焦点仍在行上，元素级 keydown 得先
 * 把焦点移进面板，等于要求用户多点一次。监听只在面板挂载期间存在，关闭即摘除。
 */
export function useEscapeKey(handler: () => void) {
  function onKeydown(event: KeyboardEvent) {
    // 输入法组合过程中的 Esc 是取消候选，不是关闭面板
    if (event.key !== 'Escape' || event.isComposing) return;
    handler();
  }

  onMounted(() => window.addEventListener('keydown', onKeydown));
  onUnmounted(() => window.removeEventListener('keydown', onKeydown));
}
