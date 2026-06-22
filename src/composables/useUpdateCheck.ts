// src/composables/useUpdateCheck.ts
import { useUpdaterStore } from '@/stores/updater';

export function useUpdateCheck() {
  const updater = useUpdaterStore();

  /**
   * Startup check — called once on app mount. Shows dialog if update available.
   */
  async function performStartupCheck() {
    try {
      const info = await updater.checkForUpdate();
      if (!info) return;

      // Delay showing to avoid race with Naive UI setup during mount
      setTimeout(() => {
        updater.dialogVisible = true;
      }, 500);
    } catch (e) {
      console.error('[updater] performStartupCheck error:', e);
    }
  }

  /**
   * Manual check — always shows dialog if update available.
   * Used by settings panel button and tray menu trigger.
   */
  async function manualCheck(): Promise<boolean> {
    const info = await updater.checkForUpdate();
    if (info) {
      // Manual triggers always show the dialog (user explicitly asked)
      updater.dialogVisible = true;
      return true;
    }
    return false;
  }

  return { performStartupCheck, manualCheck };
}
