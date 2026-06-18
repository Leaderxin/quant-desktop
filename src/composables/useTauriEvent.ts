// src/composables/useTauriEvent.ts
import { onMounted, onUnmounted, getCurrentInstance } from 'vue';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export function useTauriEvent<T>(event: string, handler: (data: T) => void) {
  let unlisten: UnlistenFn | null = null;
  const instance = getCurrentInstance();

  onMounted(async () => {
    const unlistenFn = await listen<T>(event, (e) => {
      handler(e.payload);
    });
    // Guard: if the component unmounted before listen() resolved, clean up immediately
    if (instance?.isUnmounted) {
      unlistenFn();
    } else {
      unlisten = unlistenFn;
    }
  });

  onUnmounted(() => {
    unlisten?.();
  });
}
