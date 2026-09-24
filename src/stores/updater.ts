// src/stores/updater.ts
import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { UpdateInfo } from '@/types';
import { useSettingsStore } from '@/stores/settings';
import { openExternal } from '@/utils/external';

export const useUpdaterStore = defineStore('updater', () => {
  const updateStatus = ref<'idle' | 'checking' | 'available' | 'downloading' | 'ready' | 'error'>('idle');
  const updateInfo = ref<UpdateInfo | null>(null);
  const downloadProgress = ref(0);
  const downloadedBytes = ref(0);
  const totalBytes = ref(0);
  const lastCheckTime = ref('');
  const nextReminderTime = ref('');
  const errorMessage = ref('');
  const dialogVisible = ref(false);

  const hasUpdate = computed(() => updateStatus.value === 'available');
  const isDownloading = computed(() => updateStatus.value === 'downloading');
  // Whether the last check found no update (for UI feedback)
  const isUpToDate = ref(false);

  // Store unlisten function for cleanup (prevents listener leak on HMR)
  let unlistenUpdateAvailable: (() => void) | null = null;
  let unlistenCheckComplete: (() => void) | null = null;
  let listenersInitialized = false;

  function initListeners() {
    if (listenersInitialized) return;
    listenersInitialized = true;

    // Listen for manual update triggers (tray menu / settings button).
    // Startup auto-check goes through useUpdateCheck composable instead,
    // which gates on is_trading_session() before showing the dialog.
    listen<UpdateInfo>('update-available', (event) => {
      updateStatus.value = 'available';
      updateInfo.value = event.payload;
      // Manual triggers always show dialog (user explicitly asked)
      dialogVisible.value = true;
    }).then((fn) => {
      unlistenUpdateAvailable = fn;
    }).catch((e) => console.error('[updater] Failed to listen update-available:', e));

    // Listen for tray menu "no update" results
    listen<string>('update-check-complete', (event) => {
      if (event.payload === 'up-to-date') {
        isUpToDate.value = true;
        setTimeout(() => { isUpToDate.value = false; }, 5000);
      }
    }).then((fn) => {
      unlistenCheckComplete = fn;
    }).catch((e) => console.error('[updater] Failed to listen update-check-complete:', e));
  }

  async function checkForUpdate(): Promise<UpdateInfo | null> {
    // 自更新不可用的构建在此统一拦截（前端唯一的行为判定点；UI 可见性见
    // settings store 的 updaterAvailable）：
    //   - 商店版：更新由 Microsoft Store 分发；
    //   - 便携版：更新由用户自行下载替换。
    const settings = useSettingsStore();
    if (settings.isStoreBuild || settings.isPortable) {
      console.log('[updater] Skipping update check — store or portable build');
      return null;
    }

    updateStatus.value = 'checking';
    errorMessage.value = '';
    try {
      const result = await invoke<UpdateInfo | null>('check_update');
      if (result) {
        updateStatus.value = 'available';
        updateInfo.value = result;
        lastCheckTime.value = new Date().toISOString();
        return result;
      } else {
        updateStatus.value = 'idle';
        isUpToDate.value = true;
        lastCheckTime.value = new Date().toISOString();
        // Auto-clear "up to date" after 5 seconds
        setTimeout(() => { isUpToDate.value = false; }, 5000);
        return null;
      }
    } catch (e) {
      updateStatus.value = 'error';
      errorMessage.value = String(e).slice(0, 200);
      isUpToDate.value = false;
      console.error('[updater] checkForUpdate failed:', e);
      return null;
    }
  }

  async function downloadAndInstall() {
    // 无需 store/portable 门控：安装对话框只能在 checkForUpdate 成功（已在
    // 上方统一拦截）或托盘「检查更新」事件（商店构建中编译剔除、便携构建中
    // Rust 侧已拦截）之后打开，此处必然是可自更新的构建。
    if (!updateInfo.value) return;
    updateStatus.value = 'downloading';
    downloadProgress.value = 0;
    errorMessage.value = '';

    // Listen for download progress events from Rust backend
    const unlisten = await listen<{ downloaded: number; total: number; percent: number }>(
      'update-download-progress',
      (event) => {
        downloadProgress.value = event.payload.percent;
        downloadedBytes.value = event.payload.downloaded;
        totalBytes.value = event.payload.total;
      }
    );

    try {
      await invoke('install_update');
      updateStatus.value = 'ready';
      downloadProgress.value = 100;
    } catch (e) {
      updateStatus.value = 'error';
      errorMessage.value = String(e).slice(0, 200);
      console.error('[updater] downloadAndInstall failed:', e);
    } finally {
      unlisten();
    }
  }

  function dismissUpdate() {
    // Set 24-hour cooldown
    const next = new Date();
    next.setHours(next.getHours() + 24);
    nextReminderTime.value = next.toISOString();
    dialogVisible.value = false;
  }

  function canRemind(): boolean {
    if (!nextReminderTime.value) return true;
    return new Date() >= new Date(nextReminderTime.value);
  }

  async function openReleasePage() {
    if (!updateInfo.value?.release_url) return;
    await openExternal(updateInfo.value.release_url);
  }

  function showDialog() {
    if (updateStatus.value === 'available' && canRemind()) {
      dialogVisible.value = true;
    }
  }

  function cleanup() {
    if (unlistenUpdateAvailable) {
      unlistenUpdateAvailable();
      unlistenUpdateAvailable = null;
    }
    if (unlistenCheckComplete) {
      unlistenCheckComplete();
      unlistenCheckComplete = null;
    }
    listenersInitialized = false;
  }

  function reset() {
    updateStatus.value = 'idle';
    updateInfo.value = null;
    errorMessage.value = '';
    downloadProgress.value = 0;
  }

  return {
    updateStatus,
    updateInfo,
    downloadProgress,
    downloadedBytes,
    totalBytes,
    lastCheckTime,
    nextReminderTime,
    errorMessage,
    dialogVisible,
    hasUpdate,
    isDownloading,
    isUpToDate,
    checkForUpdate,
    downloadAndInstall,
    dismissUpdate,
    canRemind,
    openReleasePage,
    showDialog,
    reset,
    cleanup,
    initListeners,
  };
});
