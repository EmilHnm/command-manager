<template>
  <header class="titlebar" data-tauri-drag-region>
    <!-- Brand Title -->
    <div class="titlebar-brand" data-tauri-drag-region>
      <div class="brand-badge">CM</div>
      <span class="brand-text">Command Manager</span>
      <span class="version-tag">v0.1.0</span>
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
import { Search, Minus, Square, X } from 'lucide-vue-next';
import { useAppLifecycle } from '@/composables/useAppLifecycle';

defineEmits<{
  (e: 'open-palette'): void;
}>();

const { triggerGracefulShutdown } = useAppLifecycle();

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

const close = () => {
  // Thay vì đóng đột ngột, gọi quy trình dừng duyên dáng Graceful Shutdown
  triggerGracefulShutdown();
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

.brand-badge {
  background: linear-gradient(135deg, var(--primary), #9a66bf);
  color: #ffffff;
  font-size: 11px;
  font-weight: 700;
  padding: 2px 5px;
  border-radius: var(--radius-sm);
  letter-spacing: 0.5px;
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
