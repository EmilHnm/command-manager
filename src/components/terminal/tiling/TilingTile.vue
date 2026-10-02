<template>
  <div
    class="tiling-tile-container"
    :class="{
      'is-active': isActive,
      'is-zoomed': isZoomed,
      'is-sync-linked': isSyncLinked,
    }"
    @pointerdown="handlePointerDown"
  >
    <!-- Tilix 28px Mini Headerbar -->
    <div class="tile-headerbar" :class="{ 'header-active': isActive }">
      <!-- Left: Status dot, Title, PID, CWD, Sync badge -->
      <div class="header-left">
        <span
          class="status-dot"
          :class="[processStatus, { active: processStatus === 'running' }]"
          :title="`Trạng thái: ${processStatus}`"
        />

        <span
          class="tile-title"
          :class="{ 'is-manual': tab.isManual }"
          :title="tabTitleTooltip"
          @dblclick="$emit('rename', tab)"
        >
          {{ displayTitle }}
        </span>

        <span v-if="pid" class="tile-badge badge-pid font-mono">
          PID: {{ pid }}
        </span>

        <span v-if="tab.shellKind" class="tile-badge badge-shell font-mono">
          {{ tab.shellKind }}
        </span>

        <span v-if="currentCwd" class="tile-cwd" :title="currentCwd">
          {{ compactCwd }}
        </span>

        <!-- Synchronized Input Link Badge -->
        <span
          v-if="isSyncLinked"
          class="sync-pill"
          title="Terminal này đang tham gia phát sóng phím đồng bộ"
        >
          <Link2 :size="10" />
          <span>SYNC</span>
        </span>
      </div>

      <!-- Right: Action Buttons (Split Right, Split Down, Zoom, Reattach, Clear, Stop, Close) -->
      <div class="header-right">
        <!-- Split to Right -->
        <button
          class="tile-btn"
          title="Chia đôi sang phải (Ctrl+Alt+R)"
          @click.stop="$emit('split-right', tab.id)"
        >
          <Columns2 :size="12" />
        </button>

        <!-- Split to Down -->
        <button
          class="tile-btn"
          title="Chia đôi xuống dưới (Ctrl+Alt+D)"
          @click.stop="$emit('split-down', tab.id)"
        >
          <Rows2 :size="12" />
        </button>

        <!-- Zoom / Maximize Toggle -->
        <button
          class="tile-btn"
          :class="{ 'zoom-active': isZoomed }"
          :title="isZoomed ? 'Khôi phục bố cục lưới (Ctrl+Shift+Z)' : 'Phóng to toàn màn hình (Ctrl+Shift+Z)'"
          @click.stop="$emit('toggle-zoom', tab.id)"
        >
          <Minimize2 v-if="isZoomed" :size="12" />
          <Maximize2 v-else :size="12" />
        </button>

        <!-- Reattach Buffer -->
        <button
          class="tile-btn"
          title="Xả lại dữ liệu từ Ring Buffer in-memory"
          @click.stop="handleReattach"
        >
          <RefreshCw :size="12" />
        </button>

        <!-- Clear Screen -->
        <button
          class="tile-btn"
          title="Xóa màn hình terminal"
          @click.stop="handleClear"
        >
          <Eraser :size="12" />
        </button>

        <!-- Stop Process Button (if running) -->
        <button
          v-if="processStatus === 'running'"
          class="tile-btn btn-stop"
          title="Dừng tiến trình (SIGTERM)"
          @click.stop="$emit('stop', tab.commandId)"
        >
          <Square :size="11" />
        </button>

        <!-- Close / Detach Tile -->
        <button
          class="tile-btn btn-close"
          title="Đóng ô này (tiến trình vẫn chạy ngầm nếu là daemon)"
          @click.stop="$emit('close', tab.id)"
        >
          <X :size="12" />
        </button>
      </div>
    </div>

    <!-- Terminal Viewport Area -->
    <div class="tile-viewport">
      <XtermPane
        ref="xtermPaneRef"
        :command-id="tab.commandId"
        :command-name="tab.name"
        :run-event-id="tab.runEventId"
        :pid="pid"
        :process-status="processStatus"
        :shell-kind="tab.shellKind"
        :history-level="tab.historyLevel"
        :ghost-text-enabled="ghostTextEnabled"
        :font-family="fontFamily"
        :font-size="fontSize"
        :restarting="restarting"
        :hide-toolbar="true"
        @stop-process="$emit('stop', $event)"
        @restart-process="$emit('restart', $event)"
        @cwd-change="handleCwdChange"
        @title-change="handleTitleChange"
        @phase-change="handlePhaseChange"
        @data="$emit('data', tab.id, $event)"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue';
import {
  Columns2,
  Rows2,
  Maximize2,
  Minimize2,
  RefreshCw,
  Eraser,
  Square,
  X,
  Link2,
} from 'lucide-vue-next';
import XtermPane from '../XtermPane.vue';
import type { OpenTabItem } from '@/composables/useSplitLayout';
import type { SyncInputMode } from '@/types/tiling';
import { useRunSession } from '@/composables/useRunSession';

const props = withDefaults(
  defineProps<{
    tab: OpenTabItem;
    isActive: boolean;
    isZoomed: boolean;
    syncMode: SyncInputMode;
    ghostTextEnabled?: boolean;
    fontFamily?: string;
    fontSize?: number;
    restarting?: boolean;
  }>(),
  {
    ghostTextEnabled: true,
    fontFamily: 'JetBrains Mono',
    fontSize: 13,
    restarting: false,
  }
);

const emit = defineEmits<{
  (e: 'focus', tabId: string): void;
  (e: 'split-right', tabId: string): void;
  (e: 'split-down', tabId: string): void;
  (e: 'toggle-zoom', tabId: string): void;
  (e: 'close', tabId: string): void;
  (e: 'stop', commandId: number): void;
  (e: 'restart', commandId: number): void;
  (e: 'rename', tab: OpenTabItem): void;
  (e: 'register-xterm', tabId: string, instance: unknown): void;
  (e: 'data', tabId: string, data: string): void;
}>();

const { getProcessStatus, getProcessInfo } = useRunSession();

const xtermPaneRef = ref<InstanceType<typeof XtermPane> | null>(null);
const currentCwd = ref(props.tab.cwd || '');
const currentTitle = ref(props.tab.title || '');

const processStatus = computed(() => getProcessStatus(props.tab.commandId));
const pid = computed(() => getProcessInfo(props.tab.commandId)?.pid);

const isSyncLinked = computed(() => {
  return props.syncMode === 'session' && processStatus.value === 'running';
});

const displayTitle = computed(() => {
  if (currentTitle.value && props.tab.isManual) {
    return currentTitle.value;
  }
  return props.tab.name;
});

const tabTitleTooltip = computed(() => {
  let s = props.tab.name;
  if (props.tab.commandId > 0) s += ` (#${props.tab.commandId})`;
  if (currentTitle.value && currentTitle.value !== props.tab.name) {
    s += ` — Chương trình: ${currentTitle.value}`;
  }
  if (currentCwd.value) s += `\nCWD: ${currentCwd.value}`;
  return s;
});

const compactCwd = computed(() => {
  if (!currentCwd.value) return '';
  const parts = currentCwd.value.replace(/\\/g, '/').split('/').filter(Boolean);
  if (parts.length <= 2) return currentCwd.value;
  return `~/${parts.slice(-2).join('/')}`;
});

const handlePointerDown = () => {
  emit('focus', props.tab.id);
};

const handleCwdChange = (cwd: string) => {
  currentCwd.value = cwd;
};

const handleTitleChange = (title: string) => {
  currentTitle.value = title;
};

const handlePhaseChange = (phase: 'prompt' | 'input' | 'running') => {
  props.tab.phase = phase;
};

const handleReattach = () => {
  xtermPaneRef.value?.handleReattach?.();
};

const handleClear = () => {
  xtermPaneRef.value?.handleClear?.();
};

onMounted(() => {
  if (xtermPaneRef.value) {
    emit('register-xterm', props.tab.id, xtermPaneRef.value);
  }
});

watch(xtermPaneRef, (val) => {
  if (val) {
    emit('register-xterm', props.tab.id, val);
  }
});
</script>

<style scoped>
.tiling-tile-container {
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  background-color: var(--bg-terminal);
  border: 1px solid var(--border-subtle);
  overflow: hidden;
  position: relative;
  transition: border-color 0.15s ease, box-shadow 0.15s ease;
}

.tiling-tile-container.is-active {
  border-color: var(--primary);
  box-shadow: inset 0 0 0 1px var(--primary), 0 0 12px -2px rgba(116, 71, 145, 0.4);
  z-index: 10;
}

.tiling-tile-container.is-sync-linked {
  border-color: rgba(116, 71, 145, 0.8);
}

/* 28px Mini Headerbar */
.tile-headerbar {
  height: 28px;
  background-color: #12151f;
  border-bottom: 1px solid var(--border-subtle);
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 8px;
  user-select: none;
  flex-shrink: 0;
  transition: background-color 0.15s ease;
}

.tile-headerbar.header-active {
  background-color: #181c2b;
  border-bottom-color: rgba(116, 71, 145, 0.4);
}

.header-left {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
  overflow: hidden;
}

.status-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background-color: var(--status-idle);
  flex-shrink: 0;
}

.status-dot.active {
  background-color: var(--status-running);
  box-shadow: 0 0 6px rgba(16, 185, 129, 0.6);
  animation: pulse-dot 2s infinite ease-in-out;
}

.status-dot.starting {
  background-color: var(--status-starting);
}

.status-dot.failed {
  background-color: var(--status-failed);
}

.tile-title {
  font-size: 11px;
  font-weight: 600;
  color: var(--text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 160px;
  cursor: pointer;
}

.tile-title:hover {
  text-decoration: underline;
  text-underline-offset: 2px;
}

.tile-badge {
  font-size: 9px;
  padding: 1px 4px;
  border-radius: 3px;
  background-color: #1e2333;
  color: var(--text-secondary);
  border: 1px solid var(--border-subtle);
  flex-shrink: 0;
}

.badge-pid {
  color: var(--primary-light, #e4b5ff);
}

.tile-cwd {
  font-size: 10px;
  font-family: monospace;
  color: var(--text-muted);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 140px;
}

.sync-pill {
  display: flex;
  align-items: center;
  gap: 3px;
  font-size: 9px;
  font-weight: 700;
  padding: 1px 5px;
  border-radius: 3px;
  background-color: rgba(116, 71, 145, 0.25);
  border: 1px solid rgba(116, 71, 145, 0.6);
  color: #e4b5ff;
  box-shadow: 0 0 8px rgba(116, 71, 145, 0.4);
  flex-shrink: 0;
}

.header-right {
  display: flex;
  align-items: center;
  gap: 2px;
  flex-shrink: 0;
}

.tile-btn {
  width: 20px;
  height: 20px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: none;
  background: transparent;
  color: var(--text-muted);
  border-radius: 3px;
  cursor: pointer;
  padding: 0;
  transition: all 0.15s ease;
}

.tile-btn:hover {
  color: var(--text-primary);
  background-color: var(--bg-surface-hover);
}

.tile-btn.zoom-active {
  color: #e4b5ff;
  background-color: rgba(116, 71, 145, 0.3);
}

.tile-btn.btn-stop:hover {
  color: var(--status-failed);
  background-color: rgba(239, 68, 68, 0.15);
}

.tile-btn.btn-close:hover {
  color: #f1f5f9;
  background-color: rgba(239, 68, 68, 0.3);
}

.tile-viewport {
  flex: 1;
  min-height: 0;
  position: relative;
  overflow: hidden;
}

@keyframes pulse-dot {
  0%, 100% {
    opacity: 1;
    transform: scale(1);
  }
  50% {
    opacity: 0.6;
    transform: scale(0.9);
  }
}
</style>
