// src/composables/useUpdateCheck.ts
import { useUpdaterStore } from '@/stores/updater';
import { useSettingsStore } from '@/stores/settings';

export function useUpdateCheck() {
  const updater = useUpdaterStore();
  const settings = useSettingsStore();

  /**
   * Startup check — called once on app mount. Shows dialog if update available.
   * Skipped in portable mode (updates are managed by the user).
   */
  async function performStartupCheck() {
    if (settings.isPortable) {
      console.log('[updater] Skipping startup update check — portable mode');
      return;
    }
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
