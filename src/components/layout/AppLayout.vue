<script setup lang="ts">
import TopBar from './TopBar.vue';
import IndexBar from '@/components/index/IndexBar.vue';
import WatchlistTable from '@/components/watchlist/WatchlistTable.vue';
</script>

<template>
  <div class="app-layout">
    <TopBar />
    <IndexBar />
    <main class="main-content">
      <WatchlistTable />
    </main>
  </div>
</template>

<style scoped>
.app-layout {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background: var(--color-surface-0);
}
.main-content {
  flex: 1;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}
</style>
