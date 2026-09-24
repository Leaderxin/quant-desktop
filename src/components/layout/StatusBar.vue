<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { NPopover, NSwitch } from 'naive-ui';
import { useSettingsStore } from '@/stores/settings';
import { useUpdaterStore } from '@/stores/updater';
import { useUpdateCheck } from '@/composables/useUpdateCheck';
import { openExternal } from '@/utils/external';
import { getVersion } from '@tauri-apps/api/app';
import { Mail, MessageCircle, Moon, Settings, Sun } from 'lucide-vue-next';

const settings = useSettingsStore();
const updater = useUpdaterStore();
const { manualCheck } = useUpdateCheck();
const appVersion = ref('');

// 微信群二维码：使用 GitHub 在线地址，二维码过期后只需替换仓库中的图片文件即可，用户端无需重新打包升级。
// 图片对应仓库路径为 public/qrcode.jpg，如你改存到其它路径，请同步修改下面的 URL。
const QRCODE_URL = 'https://raw.githubusercontent.com/Leaderxin/quant-desktop/master/public/qrcode.png';

// 项目主页：状态栏的 GitHub 图标指向这里
const GITHUB_URL = 'https://github.com/Leaderxin/quant-desktop';

const props = withDefaults(defineProps<{
  copyright?: string;
  contactEmail?: string;
  qrcodeSrc?: string;
  /** 设置页当前是否打开。仅用于按钮的选中态。 */
  settingsOpen?: boolean;
}>(), {
  copyright: '© 2026 Leaderxin',
  contactEmail: 'shazhoulen@outlook.com',
  qrcodeSrc: QRCODE_URL,
  settingsOpen: false,
});

const emit = defineEmits<{ (e: 'open-settings'): void }>();

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
        <!-- GitHub 官方 mark（Octicons mark-github-24）—— 品牌图标用官方原版，
             不混用 lucide 的描线版（同一形状两种笔触，放在一起会打架） -->
        <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor" aria-hidden="true">
          <path d="M10.226 17.284c-2.965-.36-5.054-2.493-5.054-5.256 0-1.123.404-2.336 1.078-3.144-.292-.741-.247-2.314.09-2.965.898-.112 2.111.36 2.83 1.01.853-.269 1.752-.404 2.853-.404 1.1 0 1.999.135 2.807.382.696-.629 1.932-1.1 2.83-.988.315.606.36 2.179.067 2.942.72.854 1.101 2 1.101 3.167 0 2.763-2.089 4.852-5.098 5.234.763.494 1.28 1.572 1.28 2.807v2.336c0 .674.561 1.056 1.235.786 4.066-1.55 7.255-5.615 7.255-10.646C23.5 6.188 18.334 1 11.978 1 5.62 1 .5 6.188.5 12.545c0 4.986 3.167 9.12 7.435 10.669.606.225 1.19-.18 1.19-.786V20.63a2.9 2.9 0 0 1-1.078.224c-1.483 0-2.359-.808-2.987-2.313-.247-.607-.517-.966-1.034-1.033-.27-.023-.359-.135-.359-.27 0-.27.45-.471.898-.471.652 0 1.213.404 1.797 1.235.45.651.921.943 1.483.943.561 0 .92-.202 1.437-.719.382-.381.674-.718.944-.943"/>
        </svg>
      </a>
    </div>

    <!-- Zone 2: Settings -->
    <div class="sb-zone sb-settings">
      <!-- 设置入口。看盘相关的可配置项已全部收进设置页，这里是唯一入口，
           因此图标旁边保留文字标签，不做成纯图标按钮（发现性优先）。 -->
      <button
        class="sb-settings-btn"
        :class="{ active: settingsOpen }"
        :aria-pressed="settingsOpen"
        title="设置"
        @click.stop="emit('open-settings')"
      >
        <Settings class="sb-icon" :size="14" aria-hidden="true" />
        <span class="sb-settings-label">设置</span>
      </button>

      <span class="sb-sep">·</span>

      <button
        class="sb-theme"
        :aria-label="settings.theme === 'dark' ? '切换到浅色主题' : '切换到暗色主题'"
        :title="settings.theme === 'dark' ? '切换到浅色主题' : '切换到暗色主题'"
        @click="settings.toggleTheme()"
      >
        <span class="sb-theme-label">主题</span>
        <Sun v-if="settings.theme === 'dark'" class="sb-icon" :size="14" aria-hidden="true" />
        <Moon v-else class="sb-icon" :size="14" aria-hidden="true" />
      </button>

      <span class="sb-sep">·</span>

      <!-- 开机自启：开关本体用 naive-ui 的 NSwitch（主题色随 NConfigProvider
           走应用的强调色，无障碍/键盘支持内置），外层只留文字标签 -->
      <div class="sb-autolaunch">
        <span class="sb-autolaunch-label">开机自启</span>
        <NSwitch
          size="small"
          :value="settings.autoLaunch"
          aria-label="开机自启"
          @update:value="settings.toggleAutoLaunch()"
        />
      </div>
    </div>

    <!-- Zone 3: Contact -->
    <div class="sb-zone sb-contact">
      <a class="sb-email" :href="`mailto:${contactEmail}`" title="商务合作">
        <Mail :size="14" aria-hidden="true" />
        {{ contactEmail }}
      </a>

      <NPopover trigger="click" placement="top" :show-arrow="true">
        <template #trigger>
          <button class="sb-qr-btn" aria-label="点击入群">
            <MessageCircle :size="14" aria-hidden="true" />
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
            <svg viewBox="0 0 100 100" width="160" height="160" fill="none">
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
.sb-settings-btn {
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
.sb-settings-btn:hover {
  color: var(--color-text-primary);
  background: var(--color-bg-elevated);
}
/* 设置页打开时入口保持高亮，让用户知道当前在哪、从哪儿回去 */
.sb-settings-btn.active {
  color: var(--color-accent);
  background: var(--color-accent-dim);
}
.sb-settings-label {
  color: inherit;
  line-height: 1;
}

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

/* 开机自启：文字标签 + NSwitch（开关本体样式由 naive-ui 提供，
   主题色经 NConfigProvider 的 themeOverrides 跟随应用强调色） */
.sb-autolaunch {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  line-height: 1;
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
  width: min(280px, calc(100vw - 80px));
  height: 280px;
  border-radius: var(--radius-sm);
  object-fit: contain;
  /* 悬停放大：离屏幕远扫不上码时，鼠标移上去凑近看。
     transform 不占布局空间，弹窗本身不会跟着变大。 */
  cursor: zoom-in;
  transition: transform var(--transition-fast);
}
.qr-image:hover {
  transform: scale(1.25);
}
.qr-placeholder {
  display: flex;
  flex-direction: column;
  align-items: center;
  color: var(--color-text-tertiary);
  padding: 8px;
}
</style>
