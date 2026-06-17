<script setup lang="ts">
import TopBar from './TopBar.vue';
import IndexBar from '@/components/index/IndexBar.vue';
import WatchlistTable from '@/components/watchlist/WatchlistTable.vue';
</script>

<template>
  <div class="app-layout">
    <TopBar />
    <IndexBar />
    <WatchlistTable />
  </div>
</template>

<style scoped>
.app-layout {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background-color: var(--color-bg);
}
</style>
