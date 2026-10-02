<template>
  <div class="dock-host-container">
    <!-- Top Workspace Tilix Toolbar (36px) -->
    <div class="workspace-split-toolbar">
      <div class="toolbar-left-controls">
        <!-- Sessions Drawer Toggle -->
        <button
          class="btn btn-ghost btn-sm toolbar-action-btn"
          :class="{ active: showSessionsDrawer }"
          title="Bật/Tắt Ngăn Kéo Phiên Làm Việc (Sessions Drawer)"
          @click="showSessionsDrawer = !showSessionsDrawer"
        >
          <FolderKanban :size="13" />
          <span>Phiên ({{ sessions.length }})</span>
        </button>

        <div class="toolbar-divider" />

        <!-- Layout Presets Dropdown Trigger -->
        <div class="preset-dropdown-container">
          <button
            class="btn btn-ghost btn-sm toolbar-action-btn preset-btn"
            title="Chọn bố cục chia lưới đa ô (Layout Presets)"
            @click="showPresetMenu = !showPresetMenu"
          >
            <!-- Preset Icon Preview -->
            <svg v-if="currentPreset === '1+2-tiled'" viewBox="0 0 16 16" class="preset-svg-icon">
              <rect x="1" y="1" width="7" height="14" rx="1" fill="#744791" stroke="#e4b5ff" stroke-width="0.8" />
              <rect x="9" y="1" width="6" height="6.5" rx="1" fill="#1a1e2b" stroke="#4c444f" stroke-width="0.8" />
              <rect x="9" y="8.5" width="6" height="6.5" rx="1" fill="#1a1e2b" stroke="#4c444f" stroke-width="0.8" />
            </svg>
            <svg v-else-if="currentPreset === '2x2-grid'" viewBox="0 0 16 16" class="preset-svg-icon">
              <rect x="1" y="1" width="6.5" height="6.5" rx="1" fill="#744791" stroke="#e4b5ff" stroke-width="0.8" />
              <rect x="8.5" y="1" width="6.5" height="6.5" rx="1" fill="#744791" stroke="#e4b5ff" stroke-width="0.8" />
              <rect x="1" y="8.5" width="6.5" height="6.5" rx="1" fill="#744791" stroke="#e4b5ff" stroke-width="0.8" />
              <rect x="8.5" y="8.5" width="6.5" height="6.5" rx="1" fill="#744791" stroke="#e4b5ff" stroke-width="0.8" />
            </svg>
            <svg v-else-if="currentPreset === '2-columns'" viewBox="0 0 16 16" class="preset-svg-icon">
              <rect x="1" y="1" width="6.5" height="14" rx="1" fill="#744791" stroke="#e4b5ff" stroke-width="0.8" />
              <rect x="8.5" y="1" width="6.5" height="14" rx="1" fill="#1a1e2b" stroke="#4c444f" stroke-width="0.8" />
            </svg>
            <svg v-else-if="currentPreset === '2-rows'" viewBox="0 0 16 16" class="preset-svg-icon">
              <rect x="1" y="1" width="14" height="6.5" rx="1" fill="#744791" stroke="#e4b5ff" stroke-width="0.8" />
              <rect x="1" y="8.5" width="14" height="6.5" rx="1" fill="#1a1e2b" stroke="#4c444f" stroke-width="0.8" />
            </svg>
            <svg v-else-if="currentPreset === '3-columns'" viewBox="0 0 16 16" class="preset-svg-icon">
              <rect x="1" y="1" width="4" height="14" rx="1" fill="#744791" stroke="#e4b5ff" stroke-width="0.8" />
              <rect x="6" y="1" width="4" height="14" rx="1" fill="#744791" stroke="#e4b5ff" stroke-width="0.8" />
              <rect x="11" y="1" width="4" height="14" rx="1" fill="#744791" stroke="#e4b5ff" stroke-width="0.8" />
            </svg>
            <svg v-else viewBox="0 0 16 16" class="preset-svg-icon">
              <rect x="1" y="1" width="14" height="14" rx="1" fill="#744791" stroke="#e4b5ff" stroke-width="0.8" />
            </svg>

            <span>{{ presetLabel(currentPreset) }}</span>
            <ChevronDown :size="11" class="chevron-icon" />
          </button>

          <!-- Dropdown Menu -->
          <div v-if="showPresetMenu" class="preset-dropdown-menu">
            <button
              class="preset-item"
              :class="{ active: currentPreset === 'single' }"
              @click="applyPresetMode('single')"
            >
              <Square :size="13" />
              <span>Single Pane (100%)</span>
            </button>
            <button
              class="preset-item"
              :class="{ active: currentPreset === '2-columns' }"
              @click="applyPresetMode('2-columns')"
            >
              <Columns2 :size="13" />
              <span>2 Cột Dọc (50:50)</span>
            </button>
            <button
              class="preset-item"
              :class="{ active: currentPreset === '2-rows' }"
              @click="applyPresetMode('2-rows')"
            >
              <Rows2 :size="13" />
              <span>2 Hàng Ngang (50:50)</span>
            </button>
            <button
              class="preset-item"
              :class="{ active: currentPreset === '2x2-grid' }"
              @click="applyPresetMode('2x2-grid')"
            >
              <Grid2x2 :size="13" />
              <span>Lưới 2x2 (4 Ô Vuông)</span>
            </button>
            <button
              class="preset-item"
              :class="{ active: currentPreset === '1+2-tiled' }"
              @click="applyPresetMode('1+2-tiled')"
            >
              <LayoutTemplate :size="13" />
              <span>1+2 Tiled (Tilix Classic: 1 Lớn + 2 Nhỏ)</span>
            </button>
            <button
              class="preset-item"
              :class="{ active: currentPreset === '3-columns' }"
              @click="applyPresetMode('3-columns')"
            >
              <Columns3 :size="13" />
              <span>3 Cột Song Song</span>
            </button>
          </div>
        </div>

        <div class="toolbar-divider" />

        <!-- Synchronized Input Broadcast Button -->
        <button
          class="btn btn-ghost btn-sm toolbar-action-btn sync-btn"
          :class="{ active: syncMode === 'session' }"
          :title="syncMode === 'session' ? 'Đang phát sóng phím đồng bộ tới tất cả terminal. Bấm để tắt' : 'Bật phát sóng bàn phím đồng bộ (Ctrl+Alt+S)'"
          @click="toggleSyncMode"
        >
          <Link2 :size="13" />
          <span>{{ syncMode === 'session' ? 'Sync: Session [ON]' : 'Sync Input: Off' }}</span>
        </button>

        <div class="toolbar-divider" />

        <!-- Quick Split Actions -->
        <button
          class="btn btn-ghost btn-sm toolbar-action-btn"
          title="Chia đôi sang phải ô đang chọn (Ctrl+Alt+R)"
          @click="splitActiveRight"
        >
          <Columns2 :size="12" />
          <span>Split Phải</span>
        </button>

        <button
          class="btn btn-ghost btn-sm toolbar-action-btn"
          title="Chia đôi xuống dưới ô đang chọn (Ctrl+Alt+D)"
          @click="splitActiveDown"
        >
          <Rows2 :size="12" />
          <span>Split Dưới</span>
        </button>

        <!-- Zoom Indicator & Restore -->
        <button
          v-if="zoomedTab"
          class="btn btn-primary btn-sm toolbar-action-btn zoom-pill"
          title="Bấm hoặc nhấn Ctrl+Shift+Z để khôi phục bố cục lưới cũ"
          @click="zoomedTabId = null"
        >
          <Minimize2 :size="12" />
          <span>Đang Phóng To: {{ zoomedTab.name }} (Khôi Phục)</span>
        </button>
      </div>

      <div class="toolbar-right-actions">
        <!-- New Terminal Button -->
        <button
          class="btn btn-primary btn-sm toolbar-action-btn new-term-btn"
          title="Mở thêm một terminal mới vào bố cục (Ctrl+N)"
          :disabled="openingTerminal"
          @click="openEmptyTerminal()"
        >
          <LoaderCircle v-if="openingTerminal" :size="12" class="spin" />
          <Plus v-else :size="13" />
          <span>Terminal mới</span>
        </button>

        <!-- Hide all tabs button -->
        <button
          v-if="allOpenTabs.length > 0"
          class="btn btn-ghost btn-sm toolbar-action-btn"
          title="Ẩn tất cả tab đang mở (Ẩn UI, tiến trình vẫn tiếp tục chạy ngầm)"
          @click="closeAllTabs"
        >
          Ẩn tất cả
        </button>
      </div>
    </div>

    <!-- Main Workspace Body: Sessions Drawer + Tiling Canvas -->
    <div class="workspace-tiling-body">
      <!-- Tilix Sessions Drawer (Collapsible) -->
      <SessionsDrawer
        v-if="showSessionsDrawer"
        :sessions="sessions"
        :active-session-id="activeSessionId"
        @select-session="handleSelectSession"
        @create-session="handleCreateSession"
      />

      <!-- Main Tiling Canvas -->
      <div class="tiling-canvas-container">
        <!-- ZOOMED STATE: Single tile fills 100% of workspace -->
        <div v-if="zoomedTab" class="zoomed-tile-wrapper">
          <TilingTile
            :tab="zoomedTab"
            :is-active="true"
            :is-zoomed="true"
            :sync-mode="syncMode"
            :ghost-text-enabled="ghostTextEnabled"
            :font-family="terminalFontFamily"
            :font-size="terminalFontSize"
            :restarting="restartingCmdId === zoomedTab.commandId"
            @focus="handleTileFocus"
            @split-right="handleSplitRight"
            @split-down="handleSplitDown"
            @toggle-zoom="handleToggleZoom"
            @close="handleCloseTab"
            @stop="handleStopProcess"
            @restart="handleRestartProcess"
            @rename="handleTabDblClick"
            @register-xterm="registerXtermRef"
            @data="handleTerminalData"
          />
        </div>

        <!-- NORMAL TILING TREE VIEW -->
        <template v-else-if="tilingRoot">
          <TilingNodeView
            :node="tilingRoot"
            :tabs-map="tabsMap"
            :active-tab-id="activeTabId"
            :zoomed-tab-id="zoomedTabId"
            :sync-mode="syncMode"
            :ghost-text-enabled="ghostTextEnabled"
            :font-family="terminalFontFamily"
            :font-size="terminalFontSize"
            :restarting-cmd-id="restartingCmdId"
            @focus="handleTileFocus"
            @split-right="handleSplitRight"
            @split-down="handleSplitDown"
            @toggle-zoom="handleToggleZoom"
            @close-tile="handleCloseTab"
            @stop-process="handleStopProcess"
            @restart-process="handleRestartProcess"
            @rename-tab="handleTabDblClick"
            @register-xterm="registerXtermRef"
            @update-ratio="handleRatioUpdate"
            @data="handleTerminalData"
          />
        </template>

        <!-- EMPTY STATE: No open terminals -->
        <div v-else class="workspace-empty-state">
          <div class="empty-card">
            <div class="empty-icon-wrap">
              <Terminal :size="32" class="empty-icon" />
            </div>
            <h3 class="empty-title">Không có terminal nào đang mở</h3>
            <p class="empty-desc">
              Chọn một câu lệnh từ danh sách bên trái hoặc bấm nút bên dưới để mở terminal mới.
            </p>
            <button
              class="btn btn-primary btn-md"
              :disabled="openingTerminal"
              @click="openEmptyTerminal()"
            >
              <Plus :size="14" />
              <span>Mở Terminal Mới</span>
            </button>
          </div>
        </div>
      </div>
    </div>

    <!-- Rename Terminal Modal (MOD-14) -->
    <RenameTerminalModal
      :visible="renameModalVisible"
      :tab-id="renamingTab?.id || ''"
      :current-name="renamingTab?.name || ''"
      :pid="renamingTab ? getProcessInfo(renamingTab.commandId)?.pid ?? undefined : undefined"
      :shell-kind="renamingTab?.shellKind || (renamingTab?.isManual ? 'Manual PTY' : 'Task Runner')"
      @save="handleRenameSave"
      @cancel="handleRenameCancel"
    />

    <!-- Kill Panel Confirmation Modal -->
    <KillPanelModal
      :visible="showKillModal"
      :panel-name="'paneA'"
      :tabs="allOpenTabs"
      :loading="killInProgress"
      :is-split-mode="allOpenTabs.length > 1"
      @confirm="handleConfirmKillAll"
      @cancel="showKillModal = false"
    />

    <!-- Toast Notification for Drops & Broadcast -->
    <div v-if="toastMessage" class="toast-popup" :class="toastType">
      <span>{{ toastMessage }}</span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount, nextTick, watch } from 'vue';
import {
  FolderKanban,
  Columns2,
  Rows2,
  Grid2x2,
  LayoutTemplate,
  Columns3,
  Square,
  Plus,
  Link2,
  Minimize2,
  ChevronDown,
  Terminal,
  LoaderCircle,
} from 'lucide-vue-next';
import type { OpenTabItem } from '@/composables/useSplitLayout';
import type {
  TilingNode,
  LayoutPreset,
  SyncInputMode,
  WorkspaceSessionItem,
} from '@/types/tiling';
import {
  createLeaf,
  buildPresetTree,
  splitLeafInTree,
  removeLeafFromTree,
  updateRatioInTree,
  collectLeafTabIds,
} from '@/composables/useTilingTree';
import TilingNodeView from './tiling/TilingNodeView.vue';
import TilingTile from './tiling/TilingTile.vue';
import SessionsDrawer from './tiling/SessionsDrawer.vue';
import RenameTerminalModal from '@/components/dialogs/RenameTerminalModal.vue';
import KillPanelModal from '@/components/dialogs/KillPanelModal.vue';
import { useRunSession } from '@/composables/useRunSession';
import { ipcClient } from '@/ipc/client';
import type { ProcessLifecycleStatus } from '@/types/models';

export type { OpenTabItem };

const props = defineProps<{
  initialTabs?: OpenTabItem[];
}>();

const emit = defineEmits<{
  (e: 'request-stop-process', commandId: number): void;
}>();

const { getProcessStatus, getProcessInfo, refreshProcesses, updateProcessName } = useRunSession();

// ----------------------------------------------------
// Core Tiling State
// ----------------------------------------------------
const PRESET_KEY = 'cm_tilix_preset_v2';
const savedPreset = (typeof window !== 'undefined' ? localStorage.getItem(PRESET_KEY) : null) as LayoutPreset | null;

const currentPreset = ref<LayoutPreset>(savedPreset || '1+2-tiled');
const syncMode = ref<SyncInputMode>('off');
const showPresetMenu = ref(false);
const showSessionsDrawer = ref(false);

const allOpenTabs = ref<OpenTabItem[]>([...(props.initialTabs || [])]);
const activeTabId = ref<string>(allOpenTabs.value[0]?.id || '');
const zoomedTabId = ref<string | null>(null);

const tilingRoot = ref<TilingNode | null>(null);

const ghostTextEnabled = ref(true);
const terminalFontFamily = ref('JetBrains Mono');
const terminalFontSize = ref(13);
const openingTerminal = ref(false);
const restartingCmdId = ref<number | null>(null);

// Lookup map for instant tab retrieval
const tabsMap = computed(() => {
  const map = new Map<string, OpenTabItem>();
  allOpenTabs.value.forEach(t => map.set(t.id, t));
  return map;
});

const zoomedTab = computed(() => {
  if (!zoomedTabId.value) return null;
  return tabsMap.value.get(zoomedTabId.value) || null;
});

const allOpenTabCommandIds = computed(() => allOpenTabs.value.map(t => t.commandId));

// ----------------------------------------------------
// Sessions Management
// ----------------------------------------------------
const sessions = ref<WorkspaceSessionItem[]>([
  {
    id: 'sess-dev-stack',
    name: 'Dev Stack',
    preset: '1+2-tiled',
    panelCount: 3,
    runningCount: 3,
    ramUsage: '384 MB',
    tags: ['NestJS', 'Vite', 'Redis'],
    isActive: true,
  },
  {
    id: 'sess-microservices',
    name: 'Microservices Cluster',
    preset: '2x2-grid',
    panelCount: 4,
    runningCount: 0,
    ramUsage: '128 MB',
    tags: ['Auth', 'Billing', 'Queue'],
  },
  {
    id: 'sess-infra',
    name: 'Docker & Database Infra',
    preset: '2-columns',
    panelCount: 2,
    runningCount: 1,
    ramUsage: '210 MB',
    tags: ['PostgreSQL', 'Redis'],
  },
]);

const activeSessionId = ref<string>('sess-dev-stack');

const handleSelectSession = (sessionId: string) => {
  activeSessionId.value = sessionId;
  sessions.value.forEach(s => {
    s.isActive = s.id === sessionId;
  });
  const current = sessions.value.find(s => s.id === sessionId);
  if (current) {
    applyPresetMode(current.preset);
  }
};

const handleCreateSession = () => {
  const newId = `sess-${Date.now().toString(36)}`;
  const count = sessions.value.length + 1;
  const newSess: WorkspaceSessionItem = {
    id: newId,
    name: `Phiên Làm Việc #${count}`,
    preset: '1+2-tiled',
    panelCount: 1,
    runningCount: 0,
    tags: ['Custom'],
  };
  sessions.value.push(newSess);
  handleSelectSession(newId);
};

// ----------------------------------------------------
// Quick Access & Xterm registry
// ----------------------------------------------------
export interface QuickAccessTarget {
  tabId: string;
  label: string;
  pane: 'paneA' | 'paneB';
  status: ProcessLifecycleStatus;
  busy: boolean;
  shellKind?: string | null;
}

export type SendResult = 'sent' | 'stopped' | 'input-not-at-end';

export interface XtermPaneInstance {
  focusTerminal: () => void;
  pasteText: (text: string) => boolean;
  sendCommand: (text: string, execute: boolean) => SendResult;
  inputState: () => { phase: 'prompt' | 'input' | 'running'; hasInput: boolean; cursorAtEnd: boolean };
}

const xtermPaneRefs = new Map<string, XtermPaneInstance>();
const registerXtermRef = (tabId: string, inst: unknown) => {
  if (inst) {
    xtermPaneRefs.set(tabId, inst as XtermPaneInstance);
  }
};

const quickAccessTarget = computed<QuickAccessTarget | null>(() => {
  const current = tabsMap.value.get(activeTabId.value) || allOpenTabs.value[0];
  if (!current) return null;
  const status = getProcessStatus(current.commandId);
  const busy = current.phase === 'running';
  return {
    tabId: current.id,
    label: current.name,
    pane: 'paneA',
    status,
    busy,
    shellKind: current.shellKind,
  };
});

const sendToFocusedTerminal = (
  text: string,
  options: { execute: boolean }
): SendResult | 'no-target' => {
  const target = quickAccessTarget.value;
  if (!target) return 'no-target';
  const paneInstance = xtermPaneRefs.get(target.tabId);
  if (!paneInstance) return 'no-target';

  const result = paneInstance.sendCommand(text, options.execute);
  if (result === 'input-not-at-end') {
    showToast('Đưa con trỏ về cuối dòng hoặc xoá dòng đang gõ trước khi Run', 'warning');
  } else if (result === 'stopped') {
    showToast(`Tiến trình trên tab "${target.label}" đã dừng`, 'warning');
  }
  return result;
};

// ----------------------------------------------------
// Toast Notification Helper
// ----------------------------------------------------
const toastMessage = ref('');
const toastType = ref<'info' | 'warning' | 'error'>('info');
let toastTimer: ReturnType<typeof setTimeout> | null = null;

const showToast = (msg: string, type: 'info' | 'warning' | 'error' = 'info') => {
  if (toastTimer) clearTimeout(toastTimer);
  toastMessage.value = msg;
  toastType.value = type;
  toastTimer = setTimeout(() => {
    toastMessage.value = '';
  }, 3200);
};

// ----------------------------------------------------
// Tree Layout Initialization & Preset Application
// ----------------------------------------------------
const presetLabel = (preset: LayoutPreset) => {
  switch (preset) {
    case 'single': return 'Single (100%)';
    case '2-columns': return '2 Cột (50:50)';
    case '2-rows': return '2 Hàng (50:50)';
    case '2x2-grid': return '2x2 Grid';
    case '1+2-tiled': return '1+2 Tiled';
    case '3-columns': return '3 Cột';
  }
};

const createFallbackTab = (): string => {
  const id = `terminal-fallback-${Date.now()}-${Math.random().toString(36).slice(2, 5)}`;
  const newTab: OpenTabItem = {
    id,
    commandId: -Date.now(),
    name: 'Terminal',
    isManual: true,
  };
  allOpenTabs.value.push(newTab);
  return id;
};

const applyPresetMode = (preset: LayoutPreset) => {
  currentPreset.value = preset;
  showPresetMenu.value = false;
  zoomedTabId.value = null;

  try {
    localStorage.setItem(PRESET_KEY, preset);
  } catch {}

  const tabIds = allOpenTabs.value.map(t => t.id);
  tilingRoot.value = buildPresetTree(preset, tabIds, createFallbackTab);

  if (tilingRoot.value) {
    const leafIds = collectLeafTabIds(tilingRoot.value);
    if (leafIds.length > 0 && !leafIds.includes(activeTabId.value)) {
      activeTabId.value = leafIds[0];
    }
  }
};

// ----------------------------------------------------
// Synchronized Keystroke Broadcasting (Tilix Model)
// ----------------------------------------------------
const toggleSyncMode = () => {
  if (syncMode.value === 'off') {
    syncMode.value = 'session';
    showToast('🔗 Đã bật phát sóng bàn phím đồng bộ tới toàn bộ terminal trong Session', 'info');
  } else {
    syncMode.value = 'off';
    showToast('Đã tắt phát sóng bàn phím đồng bộ', 'info');
  }
};

const handleTerminalData = (sourceTabId: string, chunk: string) => {
  if (syncMode.value !== 'session') return;

  // Broadcast typed keystroke to all OTHER running terminals in the session
  allOpenTabs.value.forEach(tab => {
    if (tab.id !== sourceTabId && getProcessStatus(tab.commandId) === 'running') {
      void ipcClient.writePty(tab.commandId, chunk, tab.runEventId).catch(() => undefined);
    }
  });
};

// ----------------------------------------------------
// Tile Focus, Zoom & Splitting Actions
// ----------------------------------------------------
const handleTileFocus = (tabId: string) => {
  activeTabId.value = tabId;
  const instance = xtermPaneRefs.get(tabId);
  instance?.focusTerminal();
};

const handleToggleZoom = (tabId: string) => {
  if (zoomedTabId.value === tabId) {
    zoomedTabId.value = null;
  } else {
    zoomedTabId.value = tabId;
  }
};

const handleRatioUpdate = (splitId: string, ratio: number) => {
  if (tilingRoot.value) {
    tilingRoot.value = updateRatioInTree(tilingRoot.value, splitId, ratio);
  }
};

const splitActiveRight = () => {
  if (activeTabId.value) {
    handleSplitRight(activeTabId.value);
  } else if (allOpenTabs.value[0]) {
    handleSplitRight(allOpenTabs.value[0].id);
  }
};

const splitActiveDown = () => {
  if (activeTabId.value) {
    handleSplitDown(activeTabId.value);
  } else if (allOpenTabs.value[0]) {
    handleSplitDown(allOpenTabs.value[0].id);
  }
};

const handleSplitRight = async (targetTabId: string) => {
  await openEmptyTerminal(undefined, undefined, targetTabId, 'horizontal');
};

const handleSplitDown = async (targetTabId: string) => {
  await openEmptyTerminal(undefined, undefined, targetTabId, 'vertical');
};

// ----------------------------------------------------
// Tab Closing, Manual Terminal & Rename
// ----------------------------------------------------
const handleCloseTab = (tabId: string) => {
  xtermPaneRefs.delete(tabId);

  if (zoomedTabId.value === tabId) {
    zoomedTabId.value = null;
  }

  // Remove from tree
  if (tilingRoot.value) {
    tilingRoot.value = removeLeafFromTree(tilingRoot.value, tabId);
  }

  // Remove from tabs array
  const idx = allOpenTabs.value.findIndex(t => t.id === tabId);
  if (idx !== -1) {
    allOpenTabs.value.splice(idx, 1);
  }

  // Update active tab
  if (activeTabId.value === tabId) {
    const leafIds = collectLeafTabIds(tilingRoot.value);
    activeTabId.value = leafIds[0] || allOpenTabs.value[0]?.id || '';
  }
};

const closeAllTabs = () => {
  xtermPaneRefs.clear();
  zoomedTabId.value = null;
  tilingRoot.value = null;
  allOpenTabs.value = [];
  activeTabId.value = '';
};

// Manual Terminal Rename Modal (MOD-14)
const renameModalVisible = ref(false);
const renamingTab = ref<OpenTabItem | null>(null);

const handleTabDblClick = (tab: OpenTabItem) => {
  renamingTab.value = tab;
  renameModalVisible.value = true;
};

const handleRenameSave = (tabId: string, newName: string) => {
  const target = allOpenTabs.value.find(t => t.id === tabId);
  if (target) {
    target.name = newName;
    ipcClient.setTerminalName?.(target.commandId, newName);
    updateProcessName(target.commandId, newName);
  }
  renameModalVisible.value = false;
  renamingTab.value = null;
};

const handleRenameCancel = () => {
  renameModalVisible.value = false;
  renamingTab.value = null;
};

// Kill Panel Modal
const showKillModal = ref(false);
const killInProgress = ref(false);

const handleConfirmKillAll = async (force: boolean) => {
  killInProgress.value = true;
  try {
    await Promise.allSettled(
      allOpenTabs.value.map(t => ipcClient.stopProcess(t.commandId, force))
    );
    closeAllTabs();
    await refreshProcesses();
    showKillModal.value = false;
  } finally {
    killInProgress.value = false;
  }
};

const handleStopProcess = (commandId: number) => {
  emit('request-stop-process', commandId);
};

const handleRestartProcess = async (commandId: number) => {
  restartingCmdId.value = commandId;
  await nextTick();
  try {
    try {
      await ipcClient.stopProcess(commandId, false);
    } catch {}
    const result = await ipcClient.runCommand(commandId);
    const tab = allOpenTabs.value.find(item => item.commandId === commandId);
    if (tab) tab.runEventId = result.runEventId;
    await refreshProcesses();
  } catch (err) {
    console.error('[DockHost] Không thể khởi động lại lệnh:', err);
  } finally {
    restartingCmdId.value = null;
  }
};

// ----------------------------------------------------
// Terminal Spawning (openEmptyTerminal & openCommandTab)
// ----------------------------------------------------
const openEmptyTerminal = async (
  _pane?: unknown,
  _shellKind?: string,
  targetTabId?: string,
  orientation: 'horizontal' | 'vertical' = 'horizontal'
) => {
  if (openingTerminal.value) return;
  openingTerminal.value = true;
  try {
    const focusedTab = tabsMap.value.get(activeTabId.value);
    const cwd = focusedTab?.cwd;
    await refreshProcesses();
    const terminal = await ipcClient.openTerminal(cwd);
    await refreshProcesses();

    const newTab: OpenTabItem = {
      id: `terminal-${terminal.commandId}-${Date.now()}`,
      commandId: terminal.commandId,
      name: 'Terminal',
      runEventId: terminal.runEventId,
      shellKind: terminal.shellKind,
      historyLevel: terminal.historyLevel,
      isManual: true,
      cwd,
    };

    allOpenTabs.value.push(newTab);

    if (!tilingRoot.value) {
      tilingRoot.value = createLeaf(newTab.id);
    } else {
      const splitTarget = targetTabId || activeTabId.value || allOpenTabs.value[0]?.id;
      tilingRoot.value = splitLeafInTree(tilingRoot.value, splitTarget, orientation, newTab.id);
    }

    activeTabId.value = newTab.id;
  } catch (error) {
    console.error('[DockHost] Không thể mở terminal mới:', error);
  } finally {
    openingTerminal.value = false;
  }
};

const openCommandTab = (
  commandId: number,
  commandName: string,
  runEventId?: string,
  shellKind?: string
) => {
  const isManual = Boolean(
    commandId <= 0 ||
    runEventId?.startsWith('terminal:') ||
    (shellKind && shellKind !== 'command')
  );

  const finalName = ipcClient.getTerminalName?.(commandId) || commandName || 'Terminal';
  const finalShellKind = shellKind || (isManual ? 'powershell' : 'command');

  const newTab: OpenTabItem = {
    id: isManual ? `terminal-${commandId}-${Date.now()}` : `cmd-${commandId}-${Date.now()}`,
    commandId,
    name: finalName,
    runEventId,
    shellKind: finalShellKind,
    historyLevel: isManual ? 2 : 0,
    isManual,
  };

  allOpenTabs.value.push(newTab);

  if (!tilingRoot.value) {
    tilingRoot.value = createLeaf(newTab.id);
  } else {
    // If layout is single, replace root or split
    if (currentPreset.value === 'single') {
      tilingRoot.value = createLeaf(newTab.id);
    } else {
      const splitTarget = activeTabId.value || allOpenTabs.value[0]?.id;
      tilingRoot.value = splitLeafInTree(tilingRoot.value, splitTarget, 'horizontal', newTab.id);
    }
  }

  activeTabId.value = newTab.id;
};

// ----------------------------------------------------
// Global Hotkey Navigation & Shortcuts
// ----------------------------------------------------
const handleGlobalKeydown = (e: KeyboardEvent) => {
  // Ctrl+Alt+R: Split right
  if (e.ctrlKey && e.altKey && (e.key === 'r' || e.key === 'R')) {
    e.preventDefault();
    splitActiveRight();
    return;
  }

  // Ctrl+Alt+D: Split down
  if (e.ctrlKey && e.altKey && (e.key === 'd' || e.key === 'D')) {
    e.preventDefault();
    splitActiveDown();
    return;
  }

  // Ctrl+Shift+Z: Toggle zoom
  if (e.ctrlKey && e.shiftKey && (e.key === 'z' || e.key === 'Z')) {
    e.preventDefault();
    if (activeTabId.value) {
      handleToggleZoom(activeTabId.value);
    }
    return;
  }

  // Ctrl+Alt+S: Toggle sync input
  if (e.ctrlKey && e.altKey && (e.key === 's' || e.key === 'S')) {
    e.preventDefault();
    toggleSyncMode();
    return;
  }

  // Ctrl+Shift+T: New session
  if (e.ctrlKey && e.shiftKey && (e.key === 't' || e.key === 'T')) {
    e.preventDefault();
    handleCreateSession();
    return;
  }
};

const handleWindowClick = (e: MouseEvent) => {
  const target = e.target as HTMLElement | null;
  if (!target?.closest('.preset-dropdown-container')) {
    showPresetMenu.value = false;
  }
};

onMounted(() => {
  window.addEventListener('keydown', handleGlobalKeydown);
  window.addEventListener('click', handleWindowClick);

  // Initialize tree based on existing tabs or preset
  if (allOpenTabs.value.length > 0) {
    applyPresetMode(currentPreset.value);
  }
});

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleGlobalKeydown);
  window.removeEventListener('click', handleWindowClick);
  if (toastTimer) clearTimeout(toastTimer);
});

// Backward compatibility bridge for old useSplitLayout props
const splitMode = computed(() => (currentPreset.value === 'single' ? 'single' : 'horizontal'));
const splitRatio = ref(50);
const setSplitMode = (m: string) => { applyPresetMode(m === 'single' ? 'single' : '2-columns'); };
const toggleSplitHorizontal = () => { applyPresetMode(currentPreset.value === '2-columns' ? 'single' : '2-columns'); };
const toggleSplitDirection = () => { applyPresetMode(currentPreset.value === '2-columns' ? '2-rows' : '2-columns'); };
const swapPanes = () => {};
const resetSplitRatio = () => {
  if (tilingRoot.value) {
    applyPresetMode(currentPreset.value);
  }
};
const moveTabToOppositePane = () => {};
const splitWithTab = () => {};
const transferTab = () => {};

defineExpose({
  openEmptyTerminal,
  openCommandTab,
  openTabCommandIds: allOpenTabCommandIds,
  openTabs: allOpenTabs,
  splitMode,
  splitRatio,
  setSplitMode,
  toggleSplitHorizontal,
  toggleSplitDirection,
  swapPanes,
  resetSplitRatio,
  moveTabToOppositePane,
  splitWithTab,
  transferTab,
  quickAccessTarget,
  sendToFocusedTerminal,
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
  position: relative;
}

/* ----------------------------------------------------
   Workspace Tilix Split View Toolbar (36px)
---------------------------------------------------- */
.workspace-split-toolbar {
  height: 36px;
  background-color: #12151f;
  border-bottom: 1px solid var(--border-subtle);
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 10px;
  flex-shrink: 0;
  user-select: none;
  z-index: 30;
}

.toolbar-left-controls {
  display: flex;
  align-items: center;
  gap: 6px;
}

.toolbar-divider {
  width: 1px;
  height: 16px;
  background-color: var(--border-subtle);
  margin: 0 4px;
}

.toolbar-action-btn {
  display: flex;
  align-items: center;
  gap: 5px;
  font-size: 11px;
  font-weight: 500;
  color: var(--text-secondary);
  padding: 4px 8px;
  border-radius: 4px;
  border: 1px solid transparent;
  background: transparent;
  cursor: pointer;
  transition: all 0.15s ease;
}

.toolbar-action-btn:hover {
  color: var(--text-primary);
  background-color: var(--bg-surface-hover);
}

.toolbar-action-btn.active {
  color: var(--primary-light, #e4b5ff);
  background-color: rgba(116, 71, 145, 0.2);
  border-color: rgba(116, 71, 145, 0.5);
}

/* Synchronized Input Toggle Button */
.sync-btn.active {
  color: #ffffff;
  background-color: var(--primary);
  border-color: var(--primary-light, #e4b5ff);
  box-shadow: 0 0 10px rgba(116, 71, 145, 0.5);
  font-weight: 600;
}

/* Preset Dropdown */
.preset-dropdown-container {
  position: relative;
}

.preset-btn {
  border: 1px solid var(--border-subtle);
  background-color: var(--bg-surface);
}

.preset-btn:hover {
  border-color: rgba(116, 71, 145, 0.5);
}

.preset-svg-icon {
  width: 14px;
  height: 14px;
  flex-shrink: 0;
}

.chevron-icon {
  color: var(--text-muted);
}

.preset-dropdown-menu {
  position: absolute;
  top: 100%;
  left: 0;
  margin-top: 4px;
  width: 280px;
  background-color: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: 6px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
  padding: 4px;
  display: flex;
  flex-direction: column;
  gap: 2px;
  z-index: 50;
}

.preset-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  font-size: 11px;
  color: var(--text-secondary);
  border-radius: 4px;
  border: none;
  background: transparent;
  cursor: pointer;
  text-align: left;
  transition: all 0.15s ease;
}

.preset-item:hover {
  background-color: var(--bg-surface-hover);
  color: var(--text-primary);
}

.preset-item.active {
  background-color: rgba(116, 71, 145, 0.2);
  color: var(--primary-light, #e4b5ff);
  font-weight: 600;
}

/* Zoom pill badge in toolbar */
.zoom-pill {
  background-color: rgba(116, 71, 145, 0.25);
  border-color: var(--primary);
  color: #e4b5ff;
  font-size: 11px;
  padding: 3px 8px;
  animation: pulse-border 2s infinite ease-in-out;
}

.toolbar-right-actions {
  display: flex;
  align-items: center;
  gap: 6px;
}

.new-term-btn {
  font-size: 11px;
  font-weight: 600;
  padding: 4px 10px;
  height: 26px;
}

/* ----------------------------------------------------
   Workspace Tiling Body
---------------------------------------------------- */
.workspace-tiling-body {
  flex: 1;
  display: flex;
  width: 100%;
  height: calc(100% - 36px);
  overflow: hidden;
  position: relative;
}

.tiling-canvas-container {
  flex: 1;
  display: flex;
  width: 100%;
  height: 100%;
  overflow: hidden;
  position: relative;
  background-color: var(--bg-app-base);
}

.zoomed-tile-wrapper {
  width: 100%;
  height: 100%;
  display: flex;
}

/* Empty State */
.workspace-empty-state {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
}

.empty-card {
  max-width: 380px;
  text-align: center;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
}

.empty-icon-wrap {
  width: 64px;
  height: 64px;
  border-radius: 12px;
  background-color: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  display: flex;
  align-items: center;
  justify-content: center;
}

.empty-icon {
  color: var(--primary-light, #e4b5ff);
}

.empty-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
  margin: 0;
}

.empty-desc {
  font-size: 12px;
  color: var(--text-muted);
  line-height: 1.5;
  margin: 0;
}

/* Toast popup */
.toast-popup {
  position: absolute;
  bottom: 16px;
  right: 16px;
  padding: 8px 14px;
  border-radius: 6px;
  font-size: 11px;
  background-color: #1a1e2b;
  border: 1px solid var(--border-subtle);
  color: var(--text-primary);
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);
  z-index: 100;
  animation: fade-in 0.2s ease;
}

.toast-popup.warning {
  border-color: #f59e0b;
  color: #fcd34d;
}

@keyframes fade-in {
  from { opacity: 0; transform: translateY(6px); }
  to { opacity: 1; transform: translateY(0); }
}

@keyframes pulse-border {
  0%, 100% {
    box-shadow: 0 0 0 1px rgba(116, 71, 145, 0.6);
  }
  50% {
    box-shadow: 0 0 0 2px rgba(116, 71, 145, 0.9), 0 0 10px rgba(116, 71, 145, 0.4);
  }
}
</style>
