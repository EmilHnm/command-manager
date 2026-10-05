<template>
  <header class="titlebar" data-tauri-drag-region>
    <!-- Brand Title & Engine Status -->
    <div class="titlebar-brand" data-tauri-drag-region>
      <img src="/logo.svg" class="brand-logo" alt="Command Manager Logo" />
      <span class="brand-text">Command Manager</span>
      <span class="version-tag">{{ APP_VERSION_TAG }}</span>
      
      <!-- Daemon Running Status Indicator Pill (from Stitch) -->
      <div
        class="daemon-pill"
        :class="{ active: runningCount > 0 }"
        title="Bấm để chuyển tới Terminal Workspace"
        @click="goToWorkspace"
      >
        <span class="daemon-dot" :class="{ pulse: runningCount > 0 }" />
        <span class="daemon-text">
          {{ runningCount > 0 ? `${runningCount} Daemon Active` : '0 Active' }}
        </span>
      </div>
    </div>

    <!-- Quick Search / Command Palette Trigger -->
    <div class="titlebar-center" data-tauri-drag-region>
      <button class="palette-trigger" @click="$emit('open-palette')">
        <Search class="icon" :size="13" />
        <span class="trigger-label">Tìm kiếm lệnh, nhóm hoặc thao tác...</span>
        <kbd class="shortcut-tag">Ctrl+K</kbd>
      </button>
    </div>

    <!-- Window Controls -->
    <div class="window-controls">
      <button class="ctrl-btn" title="Thu nhỏ" @click="minimize">
        <Minus :size="14" />
      </button>
      <button class="ctrl-btn tray-btn" title="Ẩn xuống khay hệ thống" @click="hideToTray">
        <PanelTopClose :size="14" />
      </button>
      <button class="ctrl-btn" title="Phóng to" @click="toggleMaximize">
        <Square :size="12" />
      </button>
      <button class="ctrl-btn close-btn" title="Đóng & Thoát Duyên Dáng" @click="close">
        <X :size="15" />
      </button>
    </div>
  </header>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { useRouter } from 'vue-router';
import { Search, Minus, PanelTopClose, Square, X } from 'lucide-vue-next';
import { useAppLifecycle } from '@/composables/useAppLifecycle';
import { useRunSession } from '@/composables/useRunSession';
import { APP_VERSION_TAG } from '@/config/version';

defineEmits<{
  (e: 'open-palette'): void;
}>();

const router = useRouter();
const { requestClose } = useAppLifecycle();
const { activeProcesses } = useRunSession();

const runningCount = computed(() => {
  let count = 0;
  activeProcesses.value.forEach(p => {
    if (p.status === 'running') count++;
  });
  return count;
});

const goToWorkspace = () => {
  router.push('/workspace');
};

const minimize = async () => {
  if (typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window) {
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    await getCurrentWindow().minimize();
  }
};

const toggleMaximize = async () => {
  if (typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window) {
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    await getCurrentWindow().toggleMaximize();
  }
};

const hideToTray = () => {
  if (typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window) {
    void import('@tauri-apps/api/core').then(({ invoke }) => invoke('app_hide'));
  }
};

const close = () => {
  requestClose();
};
</script>

<style scoped>
.titlebar {
  height: var(--titlebar-height);
  background-color: #0b0d13;
  border-bottom: 1px solid var(--border-subtle);
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 8px 0 14px;
  user-select: none;
  z-index: 100;
}

.titlebar-brand {
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: default;
}

.brand-logo {
  width: 22px;
  height: 22px;
  object-fit: contain;
  border-radius: var(--radius-sm);
}

.brand-text {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-primary);
  letter-spacing: 0.2px;
}

.version-tag {
  font-size: 10px;
  color: var(--text-muted);
  background: var(--bg-surface);
  padding: 1px 4px;
  border-radius: var(--radius-sm);
  border: 1px solid var(--border-subtle);
  font-family: var(--font-mono);
}

.daemon-pill {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 2px 8px;
  background-color: var(--status-idle-bg, #64748b1f);
  border: 1px solid #64748b40;
  border-radius: 9999px;
  font-size: 10.5px;
  font-weight: 500;
  color: var(--text-muted);
  cursor: pointer;
  transition: all 0.15s ease;
  margin-left: 4px;
}

.daemon-pill.active {
  background-color: var(--status-running-bg);
  border-color: #10b98159;
  color: #34d399;
}

.daemon-pill:hover {
  filter: brightness(1.15);
}

.daemon-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background-color: var(--text-muted);
}

.daemon-pill.active .daemon-dot {
  background-color: var(--status-running);
  box-shadow: 0 0 6px var(--status-running);
}

.daemon-dot.pulse {
  animation: pulseDot 2s infinite ease-in-out;
}

.titlebar-center {
  flex: 1;
  max-width: 440px;
  margin: 0 16px;
}

.palette-trigger {
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: space-between;
  background-color: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  padding: 4px 10px;
  color: var(--text-secondary);
  font-size: 11.5px;
  cursor: pointer;
  transition: all 0.15s ease;
}

.palette-trigger:hover {
  background-color: var(--bg-surface-hover);
  border-color: var(--border-focus);
  color: var(--text-primary);
}

.icon {
  margin-right: 6px;
  color: var(--text-muted);
}

.trigger-label {
  flex: 1;
  text-align: left;
}

.shortcut-tag {
  background: var(--bg-app-base);
  border: 1px solid var(--border-medium);
  border-radius: var(--radius-sm);
  padding: 1px 5px;
  font-size: 10px;
  font-family: var(--font-mono);
  color: var(--text-muted);
}

.window-controls {
  display: flex;
  align-items: center;
}

.ctrl-btn {
  background: transparent;
  border: none;
  color: var(--text-secondary);
  width: 32px;
  height: 28px;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  border-radius: var(--radius-sm);
  transition: all 0.1s ease;
}

.ctrl-btn:hover {
  background-color: var(--bg-surface-hover);
  color: var(--text-primary);
}

.close-btn:hover {
  background-color: #ef4444;
  color: #ffffff;
}
</style>
