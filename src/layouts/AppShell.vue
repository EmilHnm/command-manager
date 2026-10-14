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
import { ref, onMounted, onUnmounted, watch } from 'vue';
import { useRouter, useRoute } from 'vue-router';
import Titlebar from '@/components/titlebar/Titlebar.vue';
import ActivityBar from '@/components/nav/ActivityBar.vue';
import StatusBar from '@/components/statusbar/StatusBar.vue';
import CommandPalette from '@/components/titlebar/CommandPalette.vue';
import ShutdownOverlay from '@/components/dialogs/ShutdownOverlay.vue';
import CloseConfirmModal from '@/components/dialogs/CloseConfirmModal.vue';
import { useAppLifecycle } from '@/composables/useAppLifecycle';
import { useRunSession } from '@/composables/useRunSession';
import { usePtyStream } from '@/composables/usePtyStream';
import { ipcClient } from '@/ipc/client';
import { useKeybindings } from '@/composables/useKeybindings';
import {
  initGlobalKeybindingsListener,
  disposeGlobalKeybindingsListener,
  setDispatcherRoute,
  useKeyCommand,
} from '@/keybindings/dispatcher';

const { fetchKeybindings } = useKeybindings();

const router = useRouter();
const route = useRoute();
const showPalette = ref(false);

watch(
  () => route.path,
  (path) => {
    setDispatcherRoute(path);
  },
  { immediate: true }
);

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

// Unified Keybinding Registrations
useKeyCommand('app.commandPalette', () => {
  showPalette.value = !showPalette.value;
});

useKeyCommand('app.quit', () => {
  requestClose();
});

useKeyCommand('nav.workspace', () => {
  router.push('/workspace');
});

useKeyCommand('nav.commands', () => {
  router.push('/commands');
});

useKeyCommand('nav.groups', () => {
  router.push('/groups');
});

useKeyCommand('nav.history', () => {
  router.push('/history');
});

useKeyCommand('nav.settings', () => {
  router.push('/settings');
});

useKeyCommand('nav.templates', () => {
  router.push('/templates');
});

useKeyCommand('nav.keybindings', () => {
  router.push('/settings?tab=keybindings');
});

if (import.meta.env.DEV) {
  useKeyCommand('app.toggleDevtools', () => {
    void ipcClient.toggleDevtools().catch(console.error);
  });
}

// The webview's own menu (Reload, Inspect Element, ...) does not belong in
// the app; copy, paste and inspect have keyboard shortcuts instead.
const blockContextMenu = (event: MouseEvent) => event.preventDefault();

onMounted(async () => {
  window.addEventListener('contextmenu', blockContextMenu);
  initGlobalKeybindingsListener();
  void fetchKeybindings().catch(console.error);
  void initLifecycleListener().catch(console.error);
  initIpcListener();
  await refreshProcesses();
});

onUnmounted(() => {
  disposeLifecycleListener();
  disposeGlobalKeybindingsListener();
  window.removeEventListener('contextmenu', blockContextMenu);
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
  border: 1px solid var(--secondary-alpha-45, #4edea373);
  border-radius: var(--radius-sm);
  background: var(--bg-surface);
  color: var(--text-primary);
  font-size: 12px;
  box-shadow: 0 8px 24px #00000047;
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
