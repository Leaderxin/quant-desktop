// src/composables/useUpdateCheck.ts
import { invoke } from '@tauri-apps/api/core';
import { useUpdaterStore } from '@/stores/updater';

export function useUpdateCheck() {
  const updater = useUpdaterStore();

  /**
   * Startup check — gated by trading session.
   * Called once on app mount. Does NOT show dialog during trading hours.
   */
  async function performStartupCheck() {
    const info = await updater.checkForUpdate();
    if (!info) return; // Already up to date

    // Check if we should suppress the dialog (trading session)
    try {
      const isTrading = await invoke<boolean>('is_trading_session');
      if (isTrading) {
        console.log(
          `[updater] Update ${info.latest_version} available but suppressing during trading hours`
        );
        return;
      }
    } catch {
      // If is_trading_session call fails, show dialog anyway (safe default)
      console.warn('[updater] is_trading_session check failed, showing dialog');
    }

    // Show the update dialog
    updater.showDialog();
  }

  /**
   * Manual check — always shows dialog if update available.
   * Used by settings panel button and tray menu trigger.
   */
  async function manualCheck(): Promise<boolean> {
    const info = await updater.checkForUpdate();
    if (info) {
      updater.showDialog();
      return true;
    } else if (updater.updateStatus === 'idle') {
      return false; // No update
    }
    return true;
  }

  return { performStartupCheck, manualCheck };
}
