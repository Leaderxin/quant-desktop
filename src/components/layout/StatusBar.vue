<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { NPopover } from 'naive-ui';
import { useSettingsStore } from '@/stores/settings';
import { useUpdaterStore } from '@/stores/updater';
import { useUpdateCheck } from '@/composables/useUpdateCheck';
import { getVersion } from '@tauri-apps/api/app';

const settings = useSettingsStore();
const updater = useUpdaterStore();
const { manualCheck } = useUpdateCheck();
const appVersion = ref('');

const props = withDefaults(defineProps<{
  copyright?: string;
  contactEmail?: string;
  qrcodeSrc?: string;
}>(), {
  copyright: '© 2026 Leaderxin',
  contactEmail: 'shazhoulen@outlook.com',
  qrcodeSrc: '/qrcode.jpg',
});

onMounted(async () => {
  try {
    appVersion.value = await getVersion();
  } catch {
    appVersion.value = '';
  }
  // Preload QR code image so popover has correct dimensions on first open
  if (props.qrcodeSrc) {
    const img = new Image();
    img.src = props.qrcodeSrc;
  }
});
</script>

<template>
  <footer class="status-bar">
    <!-- Zone 1: System info -->
    <div class="sb-zone sb-info">
      <span class="sb-version" v-if="appVersion">v{{ appVersion }}</span>
      <button
        v-if="!settings.isPortable"
        class="sb-check-btn"
        :class="{ 'sb-up-to-date': updater.isUpToDate }"
        :disabled="updater.updateStatus === 'checking'"
        @click="manualCheck"
      >
        {{ updater.updateStatus === 'checking' ? '检查中...' : updater.isUpToDate ? '已是最新版本' : '检查更新' }}
      </button>
      <span class="sb-sep">·</span>
      <span class="sb-copyright">{{ copyright }}</span>
    </div>

    <!-- Zone 2: Settings -->
    <div class="sb-zone sb-settings">
      <button
        class="sb-icon-btn"
        :aria-label="settings.theme === 'dark' ? '切换到浅色主题' : '切换到暗色主题'"
        :title="settings.theme === 'dark' ? '浅色主题' : '暗色主题'"
        @click="settings.toggleTheme()"
      >
        <svg v-if="settings.theme === 'dark'" viewBox="0 0 20 20" width="16" height="16" fill="currentColor">
          <path d="M10 2a.75.75 0 01.75.75v.5a.75.75 0 01-1.5 0v-.5A.75.75 0 0110 2zM10 16a.75.75 0 01.75.75v.5a.75.75 0 01-1.5 0v-.5A.75.75 0 0110 16zM4.46 4.46a.75.75 0 011.06 0l.354.354a.75.75 0 01-1.06 1.06l-.354-.353a.75.75 0 010-1.06zM14.126 14.126a.75.75 0 011.06 0l.354.354a.75.75 0 01-1.06 1.06l-.354-.353a.75.75 0 010-1.06zM2 10a.75.75 0 01.75-.75h.5a.75.75 0 010 1.5h-.5A.75.75 0 012 10zM16 9.25a.75.75 0 000 1.5h.5a.75.75 0 000-1.5H16zM4.813 14.126a.75.75 0 010 1.06l-.353.354a.75.75 0 01-1.06-1.06l.353-.354a.75.75 0 011.06 0zM14.126 4.46a.75.75 0 010 1.06l-.353.354a.75.75 0 11-1.06-1.06l.353-.354a.75.75 0 011.06 0zM10 6.5a3.5 3.5 0 100 7 3.5 3.5 0 000-7z"/>
        </svg>
        <svg v-else viewBox="0 0 20 20" width="16" height="16" fill="currentColor">
          <path fill-rule="evenodd" d="M7.455 2.004a.75.75 0 01.26.77 7 7 0 009.958 7.967.75.75 0 011.067.853A8.5 8.5 0 116.647 1.921a.75.75 0 01.808.083z" clip-rule="evenodd"/>
        </svg>
      </button>

      <span class="sb-sep">·</span>

      <span class="sb-autolaunch" @click.stop="settings.toggleAutoLaunch()">
        <span class="sb-autolaunch-label">开机自启</span>
        <span class="sb-toggle" :class="{ on: settings.autoLaunch }">
          <span class="sb-toggle-knob"></span>
        </span>
      </span>
    </div>

    <!-- Zone 3: Contact -->
    <div class="sb-zone sb-contact">
      <a class="sb-email" :href="`mailto:${contactEmail}`" title="商务合作">
        <svg viewBox="0 0 16 16" width="12" height="12" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" aria-hidden="true">
          <rect x="1.5" y="3.5" width="13" height="9" rx="1"/>
          <path d="M1.5 4l7 4.5 7-4.5"/>
        </svg>
        {{ contactEmail }}
      </a>

      <NPopover trigger="click" placement="top" :show-arrow="true">
        <template #trigger>
          <button class="sb-qr-btn" aria-label="点击入群">
            <svg viewBox="0 0 16 16" width="14" height="14" fill="currentColor" aria-hidden="true">
              <path d="M11.176 14.429c-2.665 0-4.826-1.8-4.826-4.018 0-2.22 2.159-4.02 4.824-4.02S16 8.191 16 10.411c0 1.21-.65 2.301-1.666 3.036a.324.324 0 00-.12.366l.218.81a.616.616 0 01.029.117.166.166 0 01-.162.162.177.177 0 01-.092-.03l-1.057-.61a.519.519 0 00-.256-.074.509.509 0 00-.142.021 5.668 5.668 0 01-1.576.22z"/>
              <path d="M9.064 9.542a.647.647 0 10.557-1 .645.645 0 00-.646.647.615.615 0 00.09.353zM12.296 9.543a.646.646 0 10.546-1 .645.645 0 00-.644.644.627.627 0 00.098.356z"/>
              <path d="M0 6.826c0 1.455.781 2.765 2.001 3.656a.385.385 0 01.143.439l-.161.6-.1.373a.499.499 0 00-.032.14.192.192 0 00.193.193c.039 0 .077-.01.111-.029l1.268-.733a.622.622 0 01.308-.088c.058 0 .116.009.171.025a6.83 6.83 0 001.625.26 4.45 4.45 0 01-.177-1.251c0-2.936 2.785-5.02 5.824-5.02.05 0 .1 0 .15.002C10.587 3.429 8.392 2 5.796 2 2.596 2 0 4.16 0 6.826z"/>
              <path d="M4.632 5.271a.77.77 0 11-1.54 0 .77.77 0 011.54 0zM8.507 5.271a.77.77 0 11-1.54 0 .77.77 0 011.54 0z"/>
            </svg>
            点击入群
          </button>
        </template>
        <div class="qr-popover">
          <img
            v-if="qrcodeSrc"
            :src="qrcodeSrc"
            alt="微信群二维码"
            class="qr-image"
          />
          <div v-else class="qr-placeholder">
            <svg viewBox="0 0 100 100" width="120" height="120" fill="none">
              <rect x="10" y="10" width="30" height="30" rx="2" stroke="currentColor" stroke-width="2"/>
              <rect x="10" y="10" width="14" height="14" fill="currentColor"/>
              <rect x="26" y="10" width="14" height="14" fill="currentColor"/>
              <rect x="10" y="26" width="14" height="14" fill="currentColor"/>
              <rect x="26" y="26" width="14" height="14" fill="currentColor"/>
              <rect x="60" y="10" width="30" height="30" rx="2" stroke="currentColor" stroke-width="2"/>
              <rect x="60" y="10" width="14" height="14" fill="currentColor"/>
              <rect x="76" y="10" width="14" height="14" fill="currentColor"/>
              <rect x="60" y="26" width="14" height="14" fill="currentColor"/>
              <rect x="76" y="26" width="14" height="14" fill="currentColor"/>
              <rect x="10" y="60" width="30" height="30" rx="2" stroke="currentColor" stroke-width="2"/>
              <rect x="10" y="60" width="14" height="14" fill="currentColor"/>
              <rect x="26" y="60" width="14" height="14" fill="currentColor"/>
              <rect x="10" y="76" width="14" height="14" fill="currentColor"/>
              <rect x="26" y="76" width="14" height="14" fill="currentColor"/>
              <rect x="44" y="44" width="12" height="12" fill="currentColor"/>
              <rect x="60" y="44" width="12" height="12" fill="currentColor"/>
              <rect x="44" y="60" width="12" height="12" fill="currentColor"/>
              <rect x="60" y="60" width="12" height="12" fill="currentColor"/>
            </svg>
            <p style="font-size: 10px; color: var(--color-text-tertiary); margin-top: 6px;">请替换为微信群二维码</p>
          </div>
        </div>
      </NPopover>
    </div>
  </footer>
</template>

<style scoped>
/* ── Status bar container ── */
.status-bar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  height: 28px;
  padding: 0 var(--space-4);
  background: var(--color-surface-1);
  border-top: 1px solid var(--color-border-0);
  flex-shrink: 0;
  font-size: var(--text-xs);
  line-height: 1;
  color: var(--color-text-tertiary);
}

/* ── Zones ── */
.sb-zone {
  display: flex;
  align-items: center;
  height: 100%;
  gap: var(--space-2);
}

/* ── Separator dot ── */
.sb-sep {
  color: var(--color-border-1);
  user-select: none;
  font-weight: var(--font-weight-bold);
  line-height: 1;
}

/* ── Zone 1: Info ── */
.sb-version {
  font-weight: var(--font-weight-medium);
  color: var(--color-accent);
  font-family: var(--font-mono);
  line-height: 1;
}
.sb-check-btn {
  display: inline-flex;
  align-items: center;
  padding: 1px 6px;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
  line-height: 1;
  font-family: var(--font-sans);
  cursor: pointer;
  transition: color var(--transition-fast), background var(--transition-fast);
}
.sb-check-btn:hover:not(:disabled) {
  color: var(--color-accent);
  background: var(--color-bg-elevated);
}
.sb-check-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}
.sb-check-btn.sb-up-to-date {
  color: #3fb950;
}
.sb-copyright {
  color: var(--color-text-tertiary);
  line-height: 1;
}

/* ── Zone 2: Settings (icon buttons + toggles) ── */
.sb-icon-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--color-text-tertiary);
  cursor: pointer;
  transition: color var(--transition-fast), background var(--transition-fast);
}
.sb-icon-btn:hover {
  color: var(--color-text-primary);
  background: var(--color-bg-elevated);
}

/* Auto-launch toggle with label */
.sb-autolaunch {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  cursor: pointer;
  user-select: none;
}
.sb-autolaunch-label {
  color: var(--color-text-tertiary);
  line-height: 1;
  transition: color var(--transition-fast);
}
.sb-autolaunch:hover .sb-autolaunch-label {
  color: var(--color-text-secondary);
}

/* Toggle switch pill */
.sb-toggle {
  position: relative;
  width: 26px;
  height: 15px;
  border-radius: var(--radius-full);
  background: var(--color-border-1);
  transition: background var(--transition-fast);
  flex-shrink: 0;
}
.sb-toggle.on {
  background: var(--color-accent);
}
.sb-toggle-knob {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 11px;
  height: 11px;
  border-radius: 50%;
  background: #fff;
  transition: transform var(--transition-fast);
  box-shadow: 0 1px 2px rgba(0,0,0,0.2);
}
.sb-toggle.on .sb-toggle-knob {
  transform: translateX(11px);
}

/* ── Zone 3: Contact ── */
.sb-email {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  color: var(--color-text-tertiary);
  text-decoration: none;
  transition: color var(--transition-fast);
  cursor: pointer;
}
.sb-email:hover {
  color: var(--color-accent);
}

.sb-qr-btn {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  padding: 1px 8px;
  border: none;
  border-radius: var(--radius-sm);
  background: var(--color-accent);
  color: #fff;
  font-size: 10px;
  font-family: var(--font-sans);
  font-weight: var(--font-weight-medium);
  cursor: pointer;
  transition: filter var(--transition-fast);
}
.sb-qr-btn:hover {
  filter: brightness(1.2);
}

/* ── QR popover ── */
.qr-popover {
  padding: 8px;
  text-align: center;
}
.qr-image {
  display: block;
  width: min(200px, calc(100vw - 80px));
  height: 200px;
  border-radius: var(--radius-sm);
  object-fit: contain;
}
.qr-placeholder {
  display: flex;
  flex-direction: column;
  align-items: center;
  color: var(--color-text-tertiary);
  padding: 8px;
}
</style>
