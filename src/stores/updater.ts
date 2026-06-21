// src/stores/updater.ts
import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { UpdateInfo } from '@/types';

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

  // Store unlisten function for cleanup (prevents listener leak on HMR)
  let unlistenUpdateAvailable: (() => void) | null = null;

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

  async function checkForUpdate(): Promise<UpdateInfo | null> {
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
        lastCheckTime.value = new Date().toISOString();
        return null;
      }
    } catch (e) {
      updateStatus.value = 'error';
      errorMessage.value = String(e).slice(0, 200);
      console.error('[updater] checkForUpdate failed:', e);
      return null;
    }
  }

  async function downloadAndInstall() {
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
    const { openUrl } = await import('@tauri-apps/plugin-opener');
    await openUrl(updateInfo.value.release_url);
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
    checkForUpdate,
    downloadAndInstall,
    dismissUpdate,
    canRemind,
    openReleasePage,
    showDialog,
    reset,
    cleanup,
  };
});
