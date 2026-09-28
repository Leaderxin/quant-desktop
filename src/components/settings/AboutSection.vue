<script setup lang="ts">
// 关于：应用信息与版本、检查更新、最近几个版本的更新说明，以及
// 支持项目的引导（GitHub Star / 商店好评）。
//
// 更新说明来自仓库根目录的 CHANGELOG.md，构建时以 ?raw 内联 —— 离线可用、
// 与安装包版本严格一致（在线拉取会拿到比安装包新的日志，看着像撒谎）。
import { computed, onMounted, ref } from 'vue';
import { getVersion } from '@tauri-apps/api/app';
import { useSettingsStore } from '@/stores/settings';
import { useUpdaterStore } from '@/stores/updater';
import { useUpdateCheck } from '@/composables/useUpdateCheck';
import { parseChangelog } from '@/utils/changelog';
import { openExternal } from '@/utils/external';
import SettingsRow from './SettingsRow.vue';
import { ShoppingBag, Star } from '@lucide/vue';
// 应用图标直接取 tauri 打包用的那套（src-tauri/icons/），不再单独存一份 ——
// 两份拷贝迟早漂移，v1 的旧 logo.svg 就是这么漏在 src/assets 里的。
import iconUrl from '../../../src-tauri/icons/128x128.png';
import changelogRaw from '../../../CHANGELOG.md?raw';

const settings = useSettingsStore();
const updater = useUpdaterStore();
const { manualCheck } = useUpdateCheck();

const appVersion = ref('');

onMounted(async () => {
  try {
    appVersion.value = await getVersion();
  } catch {
    appVersion.value = '';
  }
});

// ── 外链 ──
const GITHUB_URL = 'https://github.com/Leaderxin/quant-desktop';
const RELEASES_URL = `${GITHUB_URL}/releases`;
const CHANGELOG_URL = `${GITHUB_URL}/blob/master/CHANGELOG.md`;
// 商店产品页 / 评分页走 ms-windows-store: 协议直开商店客户端；
// 网页版留作协议未注册时（精简版 Windows 等）的兜底。
const STORE_WEB_URL = 'https://apps.microsoft.com/detail/9P46MZZDTTZ0';
const STORE_PDP_URL = 'ms-windows-store://pdp/?ProductId=9P46MZZDTTZ0';
const STORE_REVIEW_URL = 'ms-windows-store://review/?ProductId=9P46MZZDTTZ0';

async function openStorePage() {
  try {
    await openExternal(STORE_PDP_URL);
  } catch {
    await openExternal(STORE_WEB_URL).catch(() => {});
  }
}

async function openStoreReview() {
  try {
    await openExternal(STORE_REVIEW_URL);
  } catch {
    await openExternal(STORE_WEB_URL).catch(() => {});
  }
}

// ── 检查更新 ──
// manualCheck 命中新版本时会自己弹出更新对话框，这里只负责按钮态。
const checkLabel = computed(() => {
  if (updater.updateStatus === 'checking') return '检查中...';
  if (updater.updateStatus === 'available') {
    return `发现新版本 v${updater.updateInfo?.latest_version ?? ''}`;
  }
  if (updater.isUpToDate) return '已是最新版本';
  return '检查更新';
});

// ── 更新说明 ──
/** 展示最近 N 个版本；再多就该去 GitHub 看完整日志了。 */
const RECENT_COUNT = 3;
const recentEntries = computed(() => parseChangelog(changelogRaw).slice(0, RECENT_COUNT));
</script>

<template>
  <section class="panel">
    <p class="panel-hint">版本信息、更新说明，以及支持这个项目的方式。</p>

    <div class="card">
      <div class="app-hero">
        <img class="hero-logo" :src="iconUrl" alt="QuantDesktop 图标" />
        <div class="hero-text">
          <div class="hero-name">
            QuantDesktop
            <span v-if="appVersion" class="hero-version">v{{ appVersion }}</span>
          </div>
          <div class="hero-slogan">实时行情 · 多源切换 · 免费高效</div>
        </div>
      </div>

      <div class="card-body">
        <SettingsRow
          v-if="settings.updaterAvailable"
          title="检查更新"
          description="启动时会自动检查一次；交易时段不打扰。有新版本时在这里立即更新"
        >
          <button
            class="check-btn"
            :class="{ ok: updater.isUpToDate, found: updater.updateStatus === 'available' }"
            :disabled="updater.updateStatus === 'checking' || updater.updateStatus === 'downloading'"
            @click="manualCheck"
          >{{ checkLabel }}</button>
        </SettingsRow>

        <SettingsRow
          v-else-if="settings.isStoreBuild"
          title="检查更新"
          description="商店版的更新由 Microsoft Store 统一分发，这里只提供入口"
        >
          <button class="link-btn" type="button" @click="openStorePage">前往 Microsoft Store</button>
        </SettingsRow>

        <SettingsRow
          v-else-if="settings.isPortable"
          title="检查更新"
          description="便携版不自带自动更新，下载新版本解压覆盖即可，自选数据不会丢"
        >
          <button class="link-btn" type="button" @click="openExternal(RELEASES_URL)">前往 GitHub Releases</button>
        </SettingsRow>
      </div>
    </div>

    <div class="card">
      <div class="card-head">
        <h3>更新说明</h3>
        <span class="meta">最近 {{ recentEntries.length }} 个版本</span>
      </div>
      <div class="card-body log-body">
        <div v-for="e in recentEntries" :key="e.version" class="log-entry">
          <div class="log-head">
            <span class="log-version">v{{ e.version }}</span>
            <span v-if="e.date" class="log-date">{{ e.date }}</span>
          </div>
          <div v-for="s in e.sections" :key="s.title" class="log-section">
            <span class="log-section-title">{{ s.title }}</span>
            <ul class="log-list">
              <li v-for="(item, i) in s.items" :key="i">{{ item }}</li>
            </ul>
          </div>
        </div>
      </div>
      <p class="card-foot">
        完整更新日志在
        <a class="foot-link" :href="CHANGELOG_URL" @click.prevent="openExternal(CHANGELOG_URL)">GitHub</a>
        ，历史版本皆可查。
      </p>
    </div>

    <div class="card">
      <div class="card-head"><h3>支持这个项目</h3></div>
      <div class="card-body">
        <p class="support-copy">
          QuantDesktop 免费开源、无广告。如果它对你的日常看盘有帮助，欢迎去 GitHub
          点一个 Star，或在 Microsoft Store 留下五星好评 —— 这是对作者持续更新最实在的鼓励。
        </p>
        <div class="support-actions">
          <button class="support-btn primary" type="button" @click="openExternal(GITHUB_URL)">
            <Star :size="14" aria-hidden="true" />
            GitHub 点个 Star
          </button>
          <button class="support-btn" type="button" @click="openStoreReview">
            <ShoppingBag :size="14" aria-hidden="true" />
            商店五星好评
          </button>
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
/* ── 应用信息头 ── */
.app-hero {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-3) var(--space-4);
  border-bottom: 1px solid var(--color-border-0);
}
.hero-logo {
  width: 40px;
  height: 40px;
  flex-shrink: 0;
}
.hero-name {
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
  font-size: var(--text-md);
  font-weight: var(--font-weight-semibold);
  color: var(--color-text-primary);
  line-height: 1.2;
}
.hero-version {
  font-family: var(--font-mono);
  font-size: var(--text-sm);
  font-weight: var(--font-weight-medium);
  color: var(--color-accent);
}
.hero-slogan {
  margin-top: 2px;
  font-size: var(--text-xs);
  color: var(--color-text-tertiary);
}

/* ── 检查更新按钮 ──
   与右侧 link-btn 同高，三种反馈态：默认 / 已是最新（绿）/ 发现新版本（强调） */
.check-btn {
  height: 26px;
  padding: 0 14px;
  border: 1px solid var(--color-accent);
  border-radius: var(--radius-sm);
  background: var(--color-accent-dim);
  color: var(--color-accent);
  font-family: var(--font-sans);
  font-size: var(--text-xs);
  cursor: pointer;
  transition: background var(--transition-fast), color var(--transition-fast), border-color var(--transition-fast);
}
.check-btn:hover:not(:disabled) {
  background: var(--color-accent);
  color: #fff;
}
.check-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}
.check-btn.ok {
  border-color: var(--color-up);
  background: transparent;
  color: var(--color-up);
}
.check-btn.found {
  border-color: var(--color-accent);
  font-weight: var(--font-weight-medium);
}
.check-btn:focus-visible {
  outline: 2px solid var(--color-accent);
  outline-offset: 1px;
}
.link-btn {
  height: 26px;
  padding: 0 10px;
  border: 1px solid var(--color-border-1);
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--color-text-secondary);
  font-family: var(--font-sans);
  font-size: var(--text-xs);
  cursor: pointer;
  transition: border-color var(--transition-fast), color var(--transition-fast);
}
.link-btn:hover {
  border-color: var(--color-accent);
  color: var(--color-accent);
}
.link-btn:focus-visible {
  outline: 2px solid var(--color-accent);
  outline-offset: 1px;
}

/* ── 更新说明 ── */
.log-body {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}
.log-entry {
  padding-left: var(--space-2);
  border-left: 2px solid var(--color-border-1);
}
.log-head {
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
}
.log-version {
  font-family: var(--font-mono);
  font-size: var(--text-sm);
  font-weight: var(--font-weight-medium);
  color: var(--color-accent);
}
.log-date {
  font-family: var(--font-mono);
  font-size: var(--text-xs);
  color: var(--color-text-tertiary);
}
.log-section {
  margin-top: var(--space-1);
}
.log-section-title {
  font-size: var(--text-xs);
  color: var(--color-text-tertiary);
}
.log-list {
  margin: 2px 0 0;
  padding: 0;
  list-style: none;
}
.log-list li {
  position: relative;
  padding-left: var(--space-3);
  font-size: var(--text-sm);
  line-height: 1.6;
  color: var(--color-text-secondary);
}
.log-list li::before {
  content: '';
  position: absolute;
  left: 4px;
  top: 9px;
  width: 3px;
  height: 3px;
  border-radius: 50%;
  background: var(--color-text-tertiary);
}
.foot-link {
  color: var(--color-accent);
  text-decoration: none;
  cursor: pointer;
}
.foot-link:hover {
  text-decoration: underline;
}

/* ── 支持项目 ── */
.support-copy {
  margin: 0 0 var(--space-3);
  font-size: var(--text-sm);
  line-height: 1.7;
  color: var(--color-text-secondary);
}
.support-actions {
  display: flex;
  gap: var(--space-3);
  flex-wrap: wrap;
}
.support-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 30px;
  padding: 0 16px;
  border: 1px solid var(--color-border-1);
  border-radius: var(--radius-md);
  background: transparent;
  color: var(--color-text-secondary);
  font-family: var(--font-sans);
  font-size: var(--text-sm);
  cursor: pointer;
  transition: border-color var(--transition-fast), color var(--transition-fast), background var(--transition-fast);
}
.support-btn:hover {
  border-color: var(--color-accent);
  color: var(--color-accent);
}
.support-btn:focus-visible {
  outline: 2px solid var(--color-accent);
  outline-offset: 1px;
}
/* Star 是主推动作：实底强调；商店好评次之，描边即可 */
.support-btn.primary {
  border-color: var(--color-accent);
  background: var(--color-accent);
  color: #fff;
}
.support-btn.primary:hover {
  filter: brightness(1.1);
  color: #fff;
}
</style>
