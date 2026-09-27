<template>
  <div class="dock-host-container">
    <!-- Tab Strip Header -->
    <div class="tabs-header">
      <div class="tabs-list">
        <div
          v-for="tab in openTabs"
          :key="tab.id"
          class="terminal-tab"
          :class="{ active: tab.id === activeTabId }"
          @click="selectTab(tab.id)"
        >
          <span
            class="tab-status-dot"
            :class="{ active: getProcessStatus(tab.commandId) === 'running' }"
          />
          <span class="tab-title">{{ tab.name }}</span>
          <span v-if="getProcessInfo(tab.commandId)?.pid" class="tab-pid">
            {{ getProcessInfo(tab.commandId)?.pid }}
          </span>

          <!-- Close tab button: chỉ ẩn tab, KHÔNG kill process -->
          <button
            class="tab-close-btn"
            title="Ẩn tab này (Tiến trình vẫn tiếp tục chạy ngầm)"
            @click.stop="closeTab(tab.id)"
          >
            <X :size="12" />
          </button>
        </div>

        <div v-if="openTabs.length === 0" class="no-tabs-msg">
          Chưa mở tab terminal nào
        </div>
      </div>

      <div class="tab-actions">
        <button
          class="btn btn-ghost btn-sm"
          title="Mở một terminal shell mới"
          :disabled="openingTerminal"
          @click="openEmptyTerminal"
        >
          <Plus :size="13" />
          <span>Terminal mới</span>
        </button>
        <button
          v-if="openTabs.length > 0"
          class="btn btn-ghost btn-sm"
          title="Đóng tất cả tab đang mở (Ẩn UI)"
          @click="closeAllTabs"
        >
          Ẩn tất cả tab
        </button>
      </div>
    </div>

    <!-- Active Terminal Viewport Area -->
    <div class="tab-content-area">
      <template v-for="tab in openTabs" :key="tab.id">
        <div v-show="tab.id === activeTabId" class="pane-wrapper">
          <XtermPane
            :command-id="tab.commandId"
            :command-name="tab.name"
            :run-event-id="tab.runEventId"
            :pid="getProcessInfo(tab.commandId)?.pid ?? undefined"
            :process-status="getProcessStatus(tab.commandId)"
            :shell-kind="tab.shellKind"
            :history-level="tab.historyLevel"
            :ghost-text-enabled="ghostTextEnabled"
            :font-family="terminalFontFamily"
            :font-size="terminalFontSize"
            @stop-process="handleStopProcess"
            @restart-process="handleRestartProcess"
          />
        </div>
      </template>

      <!-- Empty State khi không có tab nào mở -->
      <div v-if="openTabs.length === 0" class="empty-workspace">
        <div class="empty-card">
          <div class="empty-icon-wrap">
            <Terminal :size="32" />
          </div>
          <h4>Không gian Terminal sẵn sàng</h4>
          <p>
            Chọn một nhóm lệnh ở bảng điều khiển bên trái và bấm <strong>[▷ Chạy]</strong> hoặc chọn một lệnh để mở tab terminal PTY.
          </p>
          <button
            class="btn btn-primary btn-sm empty-terminal-btn"
            :disabled="openingTerminal"
            title="Mở một terminal shell mới"
            @click="openEmptyTerminal"
          >
            <Plus :size="14" />
            <span>Mở terminal mới</span>
          </button>
          <div class="empty-hint">
            💡 Lưu ý: Khi đóng thẻ tab, tiến trình vẫn chạy ngầm và Ring Buffer sẽ lưu lại log gần đây.
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { Plus, X, Terminal } from 'lucide-vue-next';
import XtermPane from './XtermPane.vue';
import { useRunSession } from '@/composables/useRunSession';
import { ipcClient } from '@/ipc/client';

export interface OpenTabItem {
  id: string; // e.g. `cmd-1`
  commandId: number;
  name: string;
  runEventId?: string;
  shellKind?: string;
  historyLevel?: number;
}

const props = defineProps<{
  initialTabs?: OpenTabItem[];
}>();

const emit = defineEmits<{
  (e: 'request-stop-process', commandId: number): void;
}>();

const { getProcessStatus, getProcessInfo, refreshProcesses } = useRunSession();

// Chỉ mở tab khi người dùng chọn/chạy command; không hiển thị dữ liệu demo mặc định.
const openTabs = ref<OpenTabItem[]>(props.initialTabs ? [...props.initialTabs] : []);

const activeTabId = ref<string>(openTabs.value[0]?.id || '');
const openingTerminal = ref(false);
const ghostTextEnabled = ref(true);
const terminalFontFamily = ref('JetBrains Mono');
const terminalFontSize = ref(13);

onMounted(async () => {
  try {
    const settings = await ipcClient.getSettings();
    ghostTextEnabled.value = settings.ghostTextEnabled;
    terminalFontFamily.value = settings.fontFamily || 'JetBrains Mono';
    terminalFontSize.value = settings.fontSize || 13;
  } catch {
    ghostTextEnabled.value = true;
  }
});

const selectTab = (tabId: string) => {
  activeTabId.value = tabId;
};

const closeTab = (tabId: string) => {
  const idx = openTabs.value.findIndex(t => t.id === tabId);
  if (idx !== -1) {
    openTabs.value.splice(idx, 1);
    if (activeTabId.value === tabId) {
      activeTabId.value = openTabs.value[0]?.id || '';
    }
  }
};

const closeAllTabs = () => {
  openTabs.value = [];
  activeTabId.value = '';
};

const openEmptyTerminal = async () => {
  if (openingTerminal.value) return;
  openingTerminal.value = true;
  try {
    // Đăng ký listener trước khi backend phát event trạng thái terminal mới.
    await refreshProcesses();
    const terminal = await ipcClient.openTerminal();
    await refreshProcesses();
    const newTab: OpenTabItem = {
      id: `terminal-${terminal.commandId}-${Date.now()}`,
      commandId: terminal.commandId,
      name: 'Terminal',
      runEventId: terminal.runEventId,
      shellKind: terminal.shellKind,
      historyLevel: terminal.historyLevel,
    };
    openTabs.value.push(newTab);
    activeTabId.value = newTab.id;
  } catch (error) {
    console.error('[DockHost] Không thể mở terminal mới:', error);
  } finally {
    openingTerminal.value = false;
  }
};

const openCommandTab = (commandId: number, commandName: string, runEventId?: string) => {
  const existing = openTabs.value.find(t => t.commandId === commandId);
  if (existing) {
    if (runEventId) existing.runEventId = runEventId;
    activeTabId.value = existing.id;
  } else {
    const newTab: OpenTabItem = {
      id: `cmd-${commandId}-${Date.now()}`,
      commandId,
      name: commandName,
      runEventId,
      shellKind: 'command',
      historyLevel: 0,
    };
    openTabs.value.push(newTab);
    activeTabId.value = newTab.id;
  }
};

const handleStopProcess = (commandId: number) => {
  emit('request-stop-process', commandId);
};

const handleRestartProcess = (commandId: number) => {
  console.log('Restart process ID:', commandId);
};

const openTabCommandIds = computed(() => openTabs.value.map(t => t.commandId));

defineExpose({
  openEmptyTerminal,
  openCommandTab,
  openTabCommandIds,
  openTabs,
});
</script>

<style scoped>
.dock-host-container {
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  background-color: var(--bg-app-base);
  overflow: hidden;
}

.tabs-header {
  height: 38px;
  background-color: #12151f;
  border-bottom: 1px solid var(--border-subtle);
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 8px;
  flex-shrink: 0;
  user-select: none;
}

.tabs-list {
  display: flex;
  align-items: center;
  gap: 4px;
  overflow-x: auto;
  flex: 1;
}

.terminal-tab {
  height: 30px;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0 12px;
  background-color: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm) var(--radius-sm) 0 0;
  color: var(--text-secondary);
  font-size: 12px;
  cursor: pointer;
  transition: all 0.1s ease;
  white-space: nowrap;
}

.terminal-tab:hover {
  background-color: var(--bg-surface-hover);
  color: var(--text-primary);
}

.terminal-tab.active {
  background-color: var(--bg-terminal);
  border-color: var(--border-subtle);
  border-top: 2px solid var(--primary);
  color: var(--text-primary);
  font-weight: 500;
}

.tab-status-dot {
  width: 7px;
  height: 7px;
  border-radius: 9999px;
  background-color: var(--status-idle);
}

.tab-status-dot.active {
  background-color: var(--status-running);
  box-shadow: 0 0 5px var(--status-running);
}

.tab-title {
  max-width: 140px;
  overflow: hidden;
  text-overflow: ellipsis;
}

.tab-pid {
  font-size: 9.5px;
  font-family: var(--font-mono);
  color: var(--text-muted);
  background: var(--bg-surface);
  padding: 1px 4px;
  border-radius: 2px;
}

.tab-close-btn {
  background: transparent;
  border: none;
  color: var(--text-muted);
  width: 16px;
  height: 16px;
  border-radius: 2px;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}

.tab-close-btn:hover {
  background-color: rgba(239, 68, 68, 0.2);
  color: #ef4444;
}

.no-tabs-msg {
  font-size: 11.5px;
  color: var(--text-muted);
  padding: 0 8px;
}

.tab-content-area {
  flex: 1;
  position: relative;
  overflow: hidden;
  background-color: var(--bg-terminal);
}

.pane-wrapper {
  width: 100%;
  height: 100%;
}

.empty-workspace {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 20px;
}

.empty-card {
  max-width: 440px;
  text-align: center;
  background-color: var(--bg-surface);
  border: 1px solid var(--border-medium);
  border-radius: var(--radius-lg);
  padding: 32px 24px;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
}

.empty-icon-wrap {
  width: 56px;
  height: 56px;
  border-radius: 50%;
  background: var(--primary-subtle);
  color: #cda8ee;
  display: flex;
  align-items: center;
  justify-content: center;
  margin-bottom: 4px;
}

.empty-card h4 {
  font-size: 16px;
  font-weight: 600;
  color: var(--text-primary);
}

.empty-card p {
  font-size: 12.5px;
  color: var(--text-secondary);
  line-height: 1.6;
}

.empty-terminal-btn {
  margin-top: 4px;
}

.empty-hint {
  margin-top: 8px;
  padding: 8px 12px;
  border-radius: var(--radius-md);
  background-color: var(--bg-app-base);
  border: 1px solid var(--border-subtle);
  font-size: 11px;
  color: var(--text-muted);
}
</style>
