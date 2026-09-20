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

// 微信群二维码：使用 GitHub 在线地址，二维码过期后只需替换仓库中的图片文件即可，用户端无需重新打包升级。
// 图片对应仓库路径为 public/qrcode.jpg，如你改存到其它路径，请同步修改下面的 URL。
const QRCODE_URL = 'https://raw.githubusercontent.com/Leaderxin/quant-desktop/master/public/qrcode.png';

// 项目主页：状态栏的 GitHub 图标指向这里
const GITHUB_URL = 'https://github.com/Leaderxin/quant-desktop';

// 用系统默认浏览器打开外链，避免 webview 内部跳转丢失原生外壳
async function openExternal(url: string) {
  const { openUrl } = await import('@tauri-apps/plugin-opener');
  await openUrl(url);
}

const props = withDefaults(defineProps<{
  copyright?: string;
  contactEmail?: string;
  qrcodeSrc?: string;
}>(), {
  copyright: '© 2026 Leaderxin',
  contactEmail: 'shazhoulen@outlook.com',
  qrcodeSrc: QRCODE_URL,
});

// 打包进安装包的本地兜底二维码（远程加载失败时回退用）
const QRCODE_FALLBACK_URL = '/qrcode.png';
// 当前实际展示的二维码地址：远程失败 → 回退本地旧图；本地也失败 → 显示占位提示
const qrSrc = ref(props.qrcodeSrc);
const qrFailed = ref(false);

function onQrError() {
  if (qrSrc.value !== QRCODE_FALLBACK_URL) {
    qrSrc.value = QRCODE_FALLBACK_URL;
  } else {
    qrFailed.value = true;
  }
}

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
        v-if="settings.updaterAvailable"
        class="sb-check-btn"
        :class="{ 'sb-up-to-date': updater.isUpToDate }"
        :disabled="updater.updateStatus === 'checking'"
        @click="manualCheck"
      >
        {{ updater.updateStatus === 'checking' ? '检查中...' : updater.isUpToDate ? '已是最新版本' : '检查更新' }}
      </button>
      <span class="sb-sep">·</span>
      <a
        class="sb-copyright"
        :href="GITHUB_URL"
        target="_blank"
        rel="noopener noreferrer"
        title="访问 GitHub 项目主页"
        @click.prevent="openExternal(GITHUB_URL)"
      >{{ copyright }}</a>
      <a
        class="sb-github"
        :href="GITHUB_URL"
        target="_blank"
        rel="noopener noreferrer"
        title="访问 GitHub 项目主页"
        aria-label="访问 GitHub 项目主页"
        @click.prevent="openExternal(GITHUB_URL)"
      >
        <!-- GitHub 官方 mark（Octicons mark-github-24） -->
        <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor" aria-hidden="true">
          <path d="M10.226 17.284c-2.965-.36-5.054-2.493-5.054-5.256 0-1.123.404-2.336 1.078-3.144-.292-.741-.247-2.314.09-2.965.898-.112 2.111.36 2.83 1.01.853-.269 1.752-.404 2.853-.404 1.1 0 1.999.135 2.807.382.696-.629 1.932-1.1 2.83-.988.315.606.36 2.179.067 2.942.72.854 1.101 2 1.101 3.167 0 2.763-2.089 4.852-5.098 5.234.763.494 1.28 1.572 1.28 2.807v2.336c0 .674.561 1.056 1.235.786 4.066-1.55 7.255-5.615 7.255-10.646C23.5 6.188 18.334 1 11.978 1 5.62 1 .5 6.188.5 12.545c0 4.986 3.167 9.12 7.435 10.669.606.225 1.19-.18 1.19-.786V20.63a2.9 2.9 0 0 1-1.078.224c-1.483 0-2.359-.808-2.987-2.313-.247-.607-.517-.966-1.034-1.033-.27-.023-.359-.135-.359-.27 0-.27.45-.471.898-.471.652 0 1.213.404 1.797 1.235.45.651.921.943 1.483.943.561 0 .92-.202 1.437-.719.382-.381.674-.718.944-.943"/>
        </svg>
      </a>
    </div>

    <!-- Zone 2: Settings -->
    <div class="sb-zone sb-settings">
      <button
        class="sb-theme"
        :aria-label="settings.theme === 'dark' ? '切换到浅色主题' : '切换到暗色主题'"
        :title="settings.theme === 'dark' ? '切换到浅色主题' : '切换到暗色主题'"
        @click="settings.toggleTheme()"
      >
        <span class="sb-theme-label">主题</span>
        <svg v-if="settings.theme === 'dark'" class="sb-icon" viewBox="0 0 20 20" width="16" height="16" fill="currentColor" aria-hidden="true">
          <path d="M10 2a.75.75 0 01.75.75v.5a.75.75 0 01-1.5 0v-.5A.75.75 0 0110 2zM10 16a.75.75 0 01.75.75v.5a.75.75 0 01-1.5 0v-.5A.75.75 0 0110 16zM4.46 4.46a.75.75 0 011.06 0l.354.354a.75.75 0 01-1.06 1.06l-.354-.353a.75.75 0 010-1.06zM14.126 14.126a.75.75 0 011.06 0l.354.354a.75.75 0 01-1.06 1.06l-.354-.353a.75.75 0 010-1.06zM2 10a.75.75 0 01.75-.75h.5a.75.75 0 010 1.5h-.5A.75.75 0 012 10zM16 9.25a.75.75 0 000 1.5h.5a.75.75 0 000-1.5H16zM4.813 14.126a.75.75 0 010 1.06l-.353.354a.75.75 0 01-1.06-1.06l.353-.354a.75.75 0 011.06 0zM14.126 4.46a.75.75 0 010 1.06l-.353.354a.75.75 0 11-1.06-1.06l.353-.354a.75.75 0 011.06 0zM10 6.5a3.5 3.5 0 100 7 3.5 3.5 0 000-7z"/>
        </svg>
        <svg v-else class="sb-icon" viewBox="0 0 20 20" width="16" height="16" fill="currentColor" aria-hidden="true">
          <path fill-rule="evenodd" d="M7.455 2.004a.75.75 0 01.26.77 7 7 0 009.958 7.967.75.75 0 011.067.853A8.5 8.5 0 116.647 1.921a.75.75 0 01.808.083z" clip-rule="evenodd"/>
        </svg>
      </button>

      <span class="sb-sep">·</span>

      <button
        class="sb-autolaunch"
        role="switch"
        :aria-checked="settings.autoLaunch"
        :aria-label="`开机自启：${settings.autoLaunch ? '已开启' : '已关闭'}`"
        @click.stop="settings.toggleAutoLaunch()"
      >
        <span class="sb-autolaunch-label">开机自启</span>
        <span class="sb-toggle" :class="{ on: settings.autoLaunch }">
          <span class="sb-toggle-knob"></span>
        </span>
      </button>
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
          <template v-if="!qrFailed">
            <img
              :src="qrSrc"
              alt="微信群二维码"
              class="qr-image"
              @error="onQrError"
            />
            <p v-if="qrSrc === QRCODE_FALLBACK_URL" style="font-size: 10px; color: var(--color-text-tertiary); margin-top: 6px;">在线二维码加载失败，已显示本地版本</p>
          </template>
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
            <p style="font-size: 10px; color: var(--color-text-tertiary); margin-top: 6px;">二维码加载失败，请稍后重试</p>
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
  text-decoration: none;
  transition: color var(--transition-fast);
  cursor: pointer;
}
.sb-copyright:hover {
  color: var(--color-text-primary);
}

.sb-github {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: var(--color-text-tertiary);
  line-height: 1;
  transition: color var(--transition-fast);
}
.sb-github:hover {
  color: var(--color-text-primary);
}

/* ── Zone 2: Settings (icon buttons + toggles) ── */
/* 主题切换：图标 + 文字标签，让用户看得出这个图标是干嘛的 */
.sb-theme {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 0 6px;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--color-text-tertiary);
  font: inherit;
  line-height: 1;
  cursor: pointer;
  user-select: none;
  transition: color var(--transition-fast), background var(--transition-fast);
}
.sb-theme:hover {
  color: var(--color-text-primary);
  background: var(--color-bg-elevated);
}
.sb-theme-label {
  color: inherit;
  line-height: 1;
}
.sb-icon {
  display: block;
  flex-shrink: 0;
}

/* Auto-launch toggle with label */
.sb-autolaunch {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  border: none;
  padding: 0;
  background: none;
  color: inherit;
  font: inherit;
  line-height: 1;
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
