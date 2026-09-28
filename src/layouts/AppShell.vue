<template>
  <div class="app-shell">
    <div v-if="showSingleInstanceAlert" class="instance-notice" role="status">
      Command Manager đã được mở sẵn; cửa sổ hiện tại đã được đưa lên trước.
    </div>
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

    <!-- Global Close Confirmation Modal -->
    <CloseConfirmModal
      :visible="showCloseConfirm"
      :active-processes="activeProcesses"
      :loading="isShuttingDown"
      @confirm="confirmClose"
      @cancel="cancelClose"
      @hide-tray="handleHideToTray"
    />

    <!-- Global Graceful Shutdown Overlay (MOD-09) -->
    <ShutdownOverlay
      :visible="isShuttingDown"
      :countdown="shutdownCountdown"
      :timeout="shutdownTimeout"
      :phase="shutdownPhase"
      :error="shutdownError"
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
import CloseConfirmModal from '@/components/dialogs/CloseConfirmModal.vue';
import { useAppLifecycle } from '@/composables/useAppLifecycle';
import { useRunSession } from '@/composables/useRunSession';
import { usePtyStream } from '@/composables/usePtyStream';

const router = useRouter();
const showPalette = ref(false);

const {
  isShuttingDown,
  showCloseConfirm,
  shutdownCountdown,
  shutdownTimeout,
  shutdownPhase,
  shutdownError,
  requestClose,
  cancelClose,
  confirmClose,
  forceExitApp,
  showSingleInstanceAlert,
  initLifecycleListener,
  disposeLifecycleListener,
} = useAppLifecycle();
const { activeProcesses, refreshProcesses } = useRunSession();
const { initIpcListener } = usePtyStream();

const handleHideToTray = () => {
  cancelClose();
  if (typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window) {
    void import('@tauri-apps/api/core').then(({ invoke }) => invoke('app_hide'));
  }
};

// Đăng ký phím tắt toàn cục theo ma trận Section 7.2 trong docs/screens.md
const handleGlobalKeydown = (e: KeyboardEvent) => {
  const isCtrlOrCmd = e.ctrlKey || e.metaKey;

  // Ctrl+Q: Đóng ứng dụng (mở modal xác nhận)
  if (isCtrlOrCmd && (e.key === 'q' || e.key === 'Q')) {
    e.preventDefault();
    requestClose();
    return;
  }

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
    else if (e.key === '6') { e.preventDefault(); router.push('/templates'); }
  }

  // Ctrl+Shift+S: Mở nhanh Cài đặt
  if (isCtrlOrCmd && e.shiftKey && (e.key === 'S' || e.key === 's')) {
    e.preventDefault();
    router.push('/settings');
  }
};

onMounted(async () => {
  window.addEventListener('keydown', handleGlobalKeydown);
  void initLifecycleListener().catch(console.error);
  initIpcListener();
  await refreshProcesses();
});

onUnmounted(() => {
  disposeLifecycleListener();
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

.instance-notice {
  position: fixed;
  top: 8px;
  right: 16px;
  z-index: 1000;
  padding: 8px 12px;
  border: 1px solid rgba(78, 222, 163, 0.45);
  border-radius: var(--radius-sm);
  background: var(--bg-surface);
  color: var(--text-primary);
  font-size: 12px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.28);
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
