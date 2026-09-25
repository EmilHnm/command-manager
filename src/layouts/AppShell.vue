<template>
  <div class="app-shell">
    <!-- Top Header: Titlebar with custom window controls & Command Palette trigger -->
    <Titlebar @open-palette="showPalette = true" />

    <!-- Main Workspace Body -->
    <div class="shell-body">
      <!-- 60px Activity Bar on Far Left -->
      <ActivityBar />

      <!-- Center Router Outlet -->
      <div class="shell-content">
        <router-view v-slot="{ Component }">
          <keep-alive include="WorkspaceView">
            <component :is="Component" />
          </keep-alive>
        </router-view>
      </div>
    </div>

    <!-- Bottom Status Bar -->
    <StatusBar />

    <!-- Global Command Palette Modal (Ctrl+K) -->
    <CommandPalette
      :visible="showPalette"
      @close="showPalette = false"
    />

    <!-- Global Graceful Shutdown Overlay (MOD-09) -->
    <ShutdownOverlay
      :visible="isShuttingDown"
      :countdown="shutdownCountdown"
      :active-processes="activeProcesses"
      @force-kill="forceExitApp"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue';
import { useRouter } from 'vue-router';
import Titlebar from '@/components/titlebar/Titlebar.vue';
import ActivityBar from '@/components/nav/ActivityBar.vue';
import StatusBar from '@/components/statusbar/StatusBar.vue';
import CommandPalette from '@/components/titlebar/CommandPalette.vue';
import ShutdownOverlay from '@/components/dialogs/ShutdownOverlay.vue';
import { useAppLifecycle } from '@/composables/useAppLifecycle';
import { useRunSession } from '@/composables/useRunSession';
import { usePtyStream } from '@/composables/usePtyStream';

const router = useRouter();
const showPalette = ref(false);

const { isShuttingDown, shutdownCountdown, forceExitApp } = useAppLifecycle();
const { activeProcesses } = useRunSession();
const { initIpcListener } = usePtyStream();

// Đăng ký phím tắt toàn cục theo ma trận Section 7.2 trong docs/screens.md
const handleGlobalKeydown = (e: KeyboardEvent) => {
  const isCtrlOrCmd = e.ctrlKey || e.metaKey;

  // Ctrl+K: Mở Command Palette
  if (isCtrlOrCmd && (e.key === 'k' || e.key === 'K')) {
    e.preventDefault();
    showPalette.value = !showPalette.value;
    return;
  }

  // Ctrl+1 .. Ctrl+5: Chuyển nhanh giữa 5 màn hình
  if (isCtrlOrCmd && !e.shiftKey) {
    if (e.key === '1') { e.preventDefault(); router.push('/workspace'); }
    else if (e.key === '2') { e.preventDefault(); router.push('/commands'); }
    else if (e.key === '3') { e.preventDefault(); router.push('/groups'); }
    else if (e.key === '4') { e.preventDefault(); router.push('/history'); }
    else if (e.key === '5') { e.preventDefault(); router.push('/settings'); }
  }

  // Ctrl+Shift+S: Mở nhanh Cài đặt
  if (isCtrlOrCmd && e.shiftKey && (e.key === 'S' || e.key === 's')) {
    e.preventDefault();
    router.push('/settings');
  }
};

onMounted(() => {
  window.addEventListener('keydown', handleGlobalKeydown);
  initIpcListener();
});

onUnmounted(() => {
  window.removeEventListener('keydown', handleGlobalKeydown);
});
</script>

<style scoped>
.app-shell {
  width: 100vw;
  height: 100vh;
  display: flex;
  flex-direction: column;
  background-color: var(--bg-app-base);
  color: var(--text-primary);
  overflow: hidden;
}

.shell-body {
  flex: 1;
  display: flex;
  height: calc(100vh - var(--titlebar-height) - var(--statusbar-height));
  overflow: hidden;
}

.shell-content {
  flex: 1;
  height: 100%;
  overflow: hidden;
  position: relative;
  background-color: var(--bg-app-base);
}
</style>
