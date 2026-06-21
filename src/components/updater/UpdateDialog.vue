<script setup lang="ts">
import { computed } from 'vue';
import { NModal, NCard, NButton, NProgress, NSpace, NScrollbar, NDivider } from 'naive-ui';
import { useUpdaterStore } from '@/stores/updater';

const updater = useUpdaterStore();

interface ChangelogSection {
  title: string;
  items: string[];
}

const formattedNotes = computed(() => {
  if (!updater.updateInfo?.notes) return [];
  return renderMarkdownLines(updater.updateInfo.notes);
});

const progressStatus = computed(() => {
  if (updater.errorMessage) return 'error';
  if (updater.updateStatus === 'ready') return 'success';
  return undefined;
});

const downloadLabel = computed(() => {
  if (updater.updateStatus === 'downloading') {
    const dl = formatBytes(updater.downloadedBytes);
    const tot = formatBytes(updater.totalBytes);
    return tot ? `正在下载 ${dl} / ${tot}` : '正在下载...';
  }
  if (updater.updateStatus === 'ready') return '下载完成';
  return '';
});

function formatBytes(bytes: number): string {
  if (!bytes || bytes <= 0) return '';
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function handleUpdate() {
  updater.downloadAndInstall();
}

function handleLater() {
  updater.dismissUpdate();
}

function handleViewOnGitHub() {
  updater.openReleasePage();
}

function handleRetry() {
  updater.reset();
  updater.checkForUpdate().then((info) => {
    if (info) updater.showDialog();
  });
}

/**
 * Lightweight Markdown renderer — zero dependencies.
 * Converts Keep a Changelog style markdown to structured sections.
 */
function renderMarkdownLines(raw: string): ChangelogSection[] {
  const sections: ChangelogSection[] = [];
  const lines = raw.split('\n');
  let currentSection: ChangelogSection | null = null;
  let passedHeader = false;

  for (const line of lines) {
    // Skip everything until we've passed the version header
    if (!passedHeader) {
      if (/^##\s+v?\d+\.\d+\.\d+/.test(line)) {
        passedHeader = true;
      }
      continue;
    }

    // Skip date line that immediately follows the version header
    if (passedHeader && /^\d{4}-\d{2}-\d{2}/.test(line.trim())) {
      continue;
    }

    // Section header: "### ..."
    const h3Match = line.match(/^###\s+(.+)/);
    if (h3Match) {
      currentSection = { title: h3Match[1].trim(), items: [] };
      sections.push(currentSection);
      continue;
    }

    // List item: "- ..." or "* ..."
    const liMatch = line.match(/^[-*]\s+(.+)/);
    if (liMatch && currentSection) {
      currentSection.items.push(liMatch[1].trim());
    }
  }

  return sections;
}
</script>

<template>
  <NModal
    :show="updater.dialogVisible"
    :mask-closable="updater.updateStatus !== 'downloading'"
    @update:show="(v: boolean) => { if (!v) updater.dismissUpdate(); }"
  >
    <NCard
      style="width: 480px; max-width: 90vw;"
      :bordered="false"
      closable
      @close="updater.dismissUpdate()"
    >
      <template #header>
        <div class="dialog-header">
          <span class="version-badge">
            <svg viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" aria-hidden="true">
              <circle cx="8" cy="8" r="6"/>
              <path d="M8 4v4l2.5 2"/>
            </svg>
            发现新版本
          </span>
        </div>
      </template>

      <NSpace vertical :size="16">
        <!-- Version comparison -->
        <div class="version-compare">
          <span class="ver-current">{{ updater.updateInfo?.current_version }}</span>
          <svg viewBox="0 0 16 16" width="16" height="16" fill="none" aria-hidden="true" class="ver-arrow">
            <path d="M3 8h10M11 4l4 4-4 4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
          <span class="ver-latest">{{ updater.updateInfo?.latest_version }}</span>
          <span class="ver-date tabular-nums">
            <svg viewBox="0 0 16 16" width="12" height="12" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" aria-hidden="true" class="date-icon">
              <rect x="2" y="3" width="12" height="11" rx="1"/>
              <path d="M5 1v3M11 1v3M2 6h12"/>
            </svg>
            {{ updater.updateInfo?.release_date || '--' }}
          </span>
        </div>

        <!-- Changelog -->
        <div class="changelog-section">
          <div class="changelog-title">更新内容</div>
          <div class="changelog-box">
            <NScrollbar style="max-height: 220px">
              <div v-if="formattedNotes.length === 0" class="changelog-empty">
                暂无更新说明
              </div>
              <div v-for="(section, si) in formattedNotes" :key="si" class="changelog-section-item">
                <h4 class="changelog-h4">{{ section.title }}</h4>
                <ul class="changelog-list">
                  <li v-for="(item, ii) in section.items" :key="ii" class="changelog-li">
                    {{ item }}
                  </li>
                </ul>
              </div>
            </NScrollbar>
          </div>
        </div>

        <!-- Download progress -->
        <div v-if="updater.updateStatus === 'downloading' || updater.updateStatus === 'error' || updater.updateStatus === 'ready'" class="progress-section">
          <NProgress
            :percentage="updater.downloadProgress"
            :status="progressStatus"
            :show-indicator="false"
            :height="6"
            :border-radius="3"
          />
          <div class="progress-info" :class="{ 'progress-error': updater.updateStatus === 'error' }">
            <template v-if="updater.updateStatus === 'error'">
              <span class="error-text">{{ updater.errorMessage || '下载失败' }}</span>
            </template>
            <template v-else>
              <span>{{ downloadLabel }}</span>
              <span class="tabular-nums">{{ updater.downloadProgress }}%</span>
            </template>
          </div>
        </div>

        <NDivider style="margin: 0" />

        <!-- Footer actions -->
        <div class="dialog-footer">
          <NButton
            text
            size="small"
            @click="handleViewOnGitHub"
          >
            在 GitHub 查看详情
          </NButton>

          <NSpace :size="8">
            <NButton
              v-if="updater.updateStatus !== 'error' && updater.updateStatus !== 'ready'"
              size="medium"
              :disabled="updater.updateStatus === 'downloading'"
              @click="handleLater"
            >
              稍后提醒
            </NButton>

            <NButton
              v-if="updater.updateStatus === 'available' || updater.updateStatus === 'downloading'"
              type="primary"
              size="medium"
              :loading="updater.updateStatus === 'downloading'"
              @click="handleUpdate"
            >
              立即更新 {{ updater.updateInfo?.latest_version }}
            </NButton>

            <NButton
              v-else-if="updater.updateStatus === 'error'"
              type="warning"
              size="medium"
              @click="handleRetry"
            >
              重试
            </NButton>

            <NButton
              v-else-if="updater.updateStatus === 'ready'"
              type="success"
              size="medium"
            >
              即将安装...
            </NButton>
          </NSpace>
        </div>
      </NSpace>
    </NCard>
  </NModal>
</template>

<style scoped>
/* ── Header ── */
.dialog-header {
  display: flex;
  align-items: center;
}

.version-badge {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 2px 10px;
  border-radius: var(--radius-sm);
  background: var(--color-accent-dim);
  color: var(--color-accent);
  font-size: var(--text-xs);
  font-weight: var(--font-weight-medium);
}

/* ── Version comparison ── */
.version-compare {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-3);
  background: var(--color-surface-2);
  border-radius: var(--radius-md);
  border: 1px solid var(--color-border-0);
}

.ver-current {
  font-family: var(--font-mono);
  font-size: var(--text-sm);
  color: var(--color-text-tertiary);
}

.ver-arrow {
  color: var(--color-accent);
  flex-shrink: 0;
}

.ver-latest {
  font-family: var(--font-mono);
  font-size: var(--text-lg);
  font-weight: var(--font-weight-semibold);
  color: var(--color-accent);
}

.ver-date {
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: var(--text-xs);
  color: var(--color-text-tertiary);
}

.date-icon {
  color: var(--color-text-tertiary);
}

/* ── Changelog ── */
.changelog-section {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.changelog-title {
  font-size: var(--text-sm);
  font-weight: var(--font-weight-medium);
  color: var(--color-text-secondary);
}

.changelog-box {
  background: var(--color-surface-2);
  border: 1px solid var(--color-border-0);
  border-radius: var(--radius-md);
  padding: var(--space-3);
}

.changelog-empty {
  color: var(--color-text-tertiary);
  font-size: var(--text-sm);
  text-align: center;
  padding: var(--space-4);
}

.changelog-section-item {
  margin-bottom: var(--space-3);
}

.changelog-section-item:last-child {
  margin-bottom: 0;
}

.changelog-h4 {
  font-size: var(--text-sm);
  font-weight: var(--font-weight-semibold);
  color: var(--color-text-primary);
  margin: 0 0 var(--space-1) 0;
  padding-left: var(--space-2);
  border-left: 2px solid var(--color-accent);
  line-height: 1.4;
}

.changelog-list {
  list-style: none;
  padding: 0;
  margin: 0;
}

.changelog-li {
  position: relative;
  padding-left: var(--space-4);
  font-size: var(--text-sm);
  color: var(--color-text-secondary);
  line-height: 1.6;
}

.changelog-li::before {
  content: '';
  position: absolute;
  left: 6px;
  top: 10px;
  width: 4px;
  height: 4px;
  border-radius: 50%;
  background: var(--color-text-tertiary);
}

/* ── Progress ── */
.progress-section {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.progress-info {
  display: flex;
  justify-content: space-between;
  font-size: var(--text-xs);
  color: var(--color-text-tertiary);
}

.progress-error {
  color: var(--color-error, #f85149);
}

.error-text {
  color: var(--color-error, #f85149);
}

/* ── Footer ── */
.dialog-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
</style>
