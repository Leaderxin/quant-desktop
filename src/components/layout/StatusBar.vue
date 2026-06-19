<script setup lang="ts">
import { NPopover } from 'naive-ui';

withDefaults(defineProps<{
  version?: string;
  copyright?: string;
  contactEmail?: string;
  qrcodeSrc?: string;
}>(), {
  version: '0.5.3',
  copyright: '© 2026 QuantDesktop',
  contactEmail: 'shazhoulen@outlook.com',
  qrcodeSrc: '',
});
</script>

<template>
  <footer class="status-bar">
    <div class="status-left">
      <span class="status-version">v{{ version }}</span>
      <span class="status-divider">|</span>
      <span class="status-copyright">{{ copyright }}</span>
    </div>
    <div class="status-right">
      <a class="status-contact" :href="`mailto:${contactEmail}`" title="商务合作">
        <svg viewBox="0 0 16 16" width="12" height="12" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" style="vertical-align: middle; margin-right: 4px;">
          <rect x="1.5" y="3.5" width="13" height="9" rx="1"/>
          <path d="M1.5 4l7 4.5 7-4.5"/>
        </svg>
        {{ contactEmail }}
      </a>
      <NPopover trigger="click" placement="top" :show-arrow="true">
        <template #trigger>
          <button class="status-qr-btn" aria-label="点击入群">
            <svg viewBox="0 0 16 16" width="12" height="12" fill="currentColor" style="vertical-align: middle; margin-right: 3px;">
              <path d="M8 2a5 5 0 0 0-5 5v1a2 2 0 0 0-2 2v2a2 2 0 0 0 2 2h1a1 1 0 0 0 1-1V8a1 1 0 0 0-1-1h-.5A4 4 0 0 1 8 3h.5a4 4 0 0 1 4 4v1h-.5a1 1 0 0 0-1 1v5a1 1 0 0 0 1 1h1a2 2 0 0 0 2-2v-2a2 2 0 0 0-2-2v-1a5 5 0 0 0-5-5z"/>
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
  color: var(--color-text-tertiary);
}
.status-left,
.status-right {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}
.status-version {
  font-weight: var(--font-weight-medium);
  color: var(--color-accent);
  font-family: var(--font-mono);
}
.status-divider {
  color: var(--color-border-1);
  user-select: none;
}
.status-copyright {
  color: var(--color-text-tertiary);
}
.status-contact {
  display: inline-flex;
  align-items: center;
  color: var(--color-text-tertiary);
  text-decoration: none;
  transition: color var(--transition-fast);
  cursor: pointer;
}
.status-contact:hover {
  color: var(--color-accent);
}
.status-qr-btn {
  display: inline-flex;
  align-items: center;
  padding: 2px 10px;
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
.status-qr-btn:hover {
  filter: brightness(1.2);
}
.qr-popover {
  padding: 8px;
  text-align: center;
}
.qr-image {
  width: 140px;
  height: 140px;
  border-radius: var(--radius-sm);
}
.qr-placeholder {
  display: flex;
  flex-direction: column;
  align-items: center;
  color: var(--color-text-tertiary);
  padding: 8px;
}
</style>
