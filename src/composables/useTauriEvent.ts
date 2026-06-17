// src/composables/useTauriEvent.ts
import { onMounted, onUnmounted } from 'vue';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export function useTauriEvent<T>(event: string, handler: (data: T) => void) {
  let unlisten: UnlistenFn | null = null;

  onMounted(async () => {
    unlisten = await listen<T>(event, (e) => {
      handler(e.payload);
    });
  });

  onUnmounted(() => {
    unlisten?.();
  });
}
