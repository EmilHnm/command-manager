<template>
  <div class="dock-host-container">
    <!-- Top Workspace Toolbar (36px) -->
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
              <span>Khung Đơn (1 Panel)</span>
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
              <span>Lưới 2x2 (4 Panels)</span>
            </button>
            <button
              class="preset-item"
              :class="{ active: currentPreset === '1+2-tiled' }"
              @click="applyPresetMode('1+2-tiled')"
            >
              <LayoutTemplate :size="13" />
              <span>1+2 Tiled (1 Chính + 2 Phụ)</span>
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
          <span>{{ syncMode === 'session' ? 'Sync: On' : 'Sync: Off' }}</span>
        </button>

        <div class="toolbar-divider" />

        <!-- Quick Split Actions for Active Panel -->
        <button
          class="btn btn-ghost btn-sm toolbar-action-btn"
          title="Chia đôi sang phải panel đang chọn (Ctrl+Alt+R)"
          @click="splitActiveRight"
        >
          <Columns2 :size="12" />
          <span>Split Phải</span>
        </button>

        <button
          class="btn btn-ghost btn-sm toolbar-action-btn"
          title="Chia đôi xuống dưới panel đang chọn (Ctrl+Alt+D)"
          @click="splitActiveDown"
        >
          <Rows2 :size="12" />
          <span>Split Dưới</span>
        </button>

        <!-- Zoom Indicator & Restore -->
        <button
          v-if="zoomedPanel"
          class="btn btn-primary btn-sm toolbar-action-btn zoom-pill"
          title="Bấm hoặc nhấn Ctrl+Shift+Z để khôi phục bố cục lưới cũ"
          @click="currentZoomedPanelId = null"
        >
          <Minimize2 :size="12" />
          <span>Đang Phóng To Panel (Bấm để Khôi Phục)</span>
        </button>
      </div>

      <div class="toolbar-right-actions">
        <!-- New Terminal / Tab Button -->
        <button
          class="btn btn-primary btn-sm toolbar-action-btn new-term-btn"
          title="Mở thêm một tab terminal mới vào panel hiện tại (Ctrl+N)"
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

    <!-- Windows Bar: Multiple Windows / Workspaces Tabs -->
    <div class="workspace-windows-bar">
      <div class="windows-tab-list custom-scrollbar">
        <div
          v-for="win in windows"
          :key="win.id"
          class="window-tab-item"
          :class="{ active: win.id === activeWindowId }"
          @click="switchWindow(win.id)"
          @dblclick="startRenameWindow(win)"
        >
          <Monitor :size="12" class="window-icon" />
          <input
            v-if="renamingWindowId === win.id"
            ref="renameWindowInputRef"
            v-model="renamingWindowName"
            type="text"
            class="window-rename-input"
            maxlength="24"
            @blur="saveRenameWindow"
            @keydown.enter="saveRenameWindow"
            @keydown.esc="renamingWindowId = null"
            @click.stop
          />
          <span v-else class="window-title" :title="`Cửa sổ: ${win.name}`">{{ win.name }}</span>
          <span class="window-panels-count">{{ getWindowPanelCount(win) }}p</span>
          <button
            v-if="windows.length > 1"
            class="window-close-btn"
            title="Đóng cửa sổ này"
            @click.stop="closeWindow(win.id)"
          >
            <X :size="10" />
          </button>
        </div>

        <!-- Add New Window Button -->
        <button
          class="window-add-tab-btn"
          title="Mở thêm cửa sổ Workspace mới (Ctrl+Shift+T)"
          @click="createNewWindow"
        >
          <Plus :size="12" />
          <span>Cửa Sổ Mới</span>
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
        <!-- ZOOMED STATE: Single panel fills 100% of workspace -->
        <div v-if="zoomedPanel" class="zoomed-panel-wrapper">
          <TilingPanel
            :panel="zoomedPanel"
            :tabs-map="tabsMap"
            :is-active="true"
            :is-zoomed="true"
            :sync-mode="syncMode"
            :ghost-text-enabled="ghostTextEnabled"
            :font-family="terminalFontFamily"
            :font-size="terminalFontSize"
            :restarting-cmd-id="restartingCmdId"
            @focus="handlePanelFocus"
            @select-tab="handleSelectTab"
            @new-tab="handleNewTabInPanel"
            @close-tab="handleCloseTab"
            @close-panel="handleClosePanel"
            @split-right="handleSplitRight"
            @split-down="handleSplitDown"
            @toggle-zoom="handleToggleZoom"
            @rename-tab="handleTabDblClick"
            @stop-process="handleStopProcess"
            @restart-process="handleRestartProcess"
            @register-xterm="registerXtermRef"
            @data="handleTerminalData"
            @cwd-change="handleTabCwdChange"
            @title-change="handleTabTitleChange"
            @phase-change="handleTabPhaseChange"
            @transfer-tab="handleTransferTab"
          />
        </div>

        <!-- NORMAL TILING TREE VIEW -->
        <template v-else-if="currentTilingRoot">
          <TilingNodeView
            :node="currentTilingRoot"
            :tabs-map="tabsMap"
            :active-panel-id="currentActivePanelId"
            :zoomed-panel-id="currentZoomedPanelId"
            :sync-mode="syncMode"
            :ghost-text-enabled="ghostTextEnabled"
            :font-family="terminalFontFamily"
            :font-size="terminalFontSize"
            :restarting-cmd-id="restartingCmdId"
            @focus="handlePanelFocus"
            @select-tab="handleSelectTab"
            @new-tab="handleNewTabInPanel"
            @close-tab="handleCloseTab"
            @close-panel="handleClosePanel"
            @split-right="handleSplitRight"
            @split-down="handleSplitDown"
            @toggle-zoom="handleToggleZoom"
            @rename-tab="handleTabDblClick"
            @stop-process="handleStopProcess"
            @restart-process="handleRestartProcess"
            @register-xterm="registerXtermRef"
            @update-ratio="handleRatioUpdate"
            @data="handleTerminalData"
            @cwd-change="handleTabCwdChange"
            @title-change="handleTabTitleChange"
            @phase-change="handleTabPhaseChange"
            @transfer-tab="handleTransferTab"
          />
        </template>

        <!-- EMPTY STATE -->
        <div v-else class="workspace-empty-state">
          <div class="empty-card">
            <div class="empty-icon-wrap">
              <Terminal :size="32" class="empty-icon" />
            </div>
            <h4 class="empty-title">Không gian làm việc sẵn sàng</h4>
            <p class="empty-desc">
              Chọn lệnh từ danh mục bên trái hoặc mở terminal mới để bắt đầu.
            </p>
            <button
              class="btn btn-primary btn-sm"
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

    <!-- Toast Notification Popup -->
    <div v-if="toastMessage" class="toast-popup" :class="toastType">
      <span>{{ toastMessage }}</span>
    </div>

    <!-- Floating Dragged Tab (follows cursor during tab drag) -->
    <Teleport to="body">
      <div
        v-if="dragState?.isDragging"
        class="floating-drag-tab"
        :style="{
          top: `${dragState.currentY - dragState.grabOffsetY}px`,
          left: `${dragState.currentX - dragState.grabOffsetX}px`,
          width: `${dragState.width}px`,
          height: `${dragState.height}px`,
        }"
      >
        <span
          class="tab-status-dot"
          :class="[getProcessStatus(dragState.tab.commandId), { active: getProcessStatus(dragState.tab.commandId) === 'running' }]"
        />
        <span class="tab-title-text">{{ dragState.tab.title || dragState.tab.name }}</span>
      </div>
    </Teleport>

    <!-- Tab Drag Guard (protects against xterm/canvas swallowing mouse events) -->
    <Teleport to="body">
      <div
        v-if="dragState?.isDragging"
        class="tab-drag-guard"
      />
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount, nextTick } from 'vue';
import {
  Square,
  Columns2,
  Rows2,
  Columns3,
  Grid2x2,
  LayoutTemplate,
  Plus,
  Link2,
  ChevronDown,
  Terminal,
  FolderKanban,
  Minimize2,
  Monitor,
  X,
  LoaderCircle,
} from 'lucide-vue-next';
import RenameTerminalModal from '@/components/dialogs/RenameTerminalModal.vue';
import KillPanelModal from '@/components/dialogs/KillPanelModal.vue';
import TilingNodeView from './tiling/TilingNodeView.vue';
import TilingPanel, { type OpenTabItem } from './tiling/TilingPanel.vue';
import SessionsDrawer from './tiling/SessionsDrawer.vue';
import XtermPane from '@/components/terminal/XtermPane.vue';
import {
  createPanel,
  buildPresetTree,
  splitPanelInTree,
  removePanelFromTree,
  addTabToPanel,
  removeTabFromPanel,
  findPanelById,
  findPanelByTabId,
  collectAllPanels,
  collectAllTabIds,
  updateRatioInTree,
  transferTabBetweenPanels,
} from '@/composables/useTilingTree';
import type {
  LayoutPreset,
  SyncInputMode,
  WorkspaceSessionItem,
  WorkspaceWindowItem,
} from '@/types/tiling';
import type { ProcessLifecycleStatus } from '@/types/models';
import { useRunSession } from '@/composables/useRunSession';
import { useTabDrag } from '@/composables/useTabDrag';
import { ipcClient } from '@/ipc/client';

const emit = defineEmits<{
  (e: 'request-stop-process', commandId: number): void;
}>();

const { getProcessStatus, getProcessInfo, refreshProcesses, updateProcessName } = useRunSession();

// Terminal Appearance Settings
const ghostTextEnabled = ref(true);
const terminalFontFamily = ref('JetBrains Mono');
const terminalFontSize = ref(13);
const restartingCmdId = ref<number | null>(null);
const openingTerminal = ref(false);

// Active Tab reference for QuickAccessPanel / keyboard input
const activeTabId = ref<string>('');

// Master tabs list
const allOpenTabs = ref<OpenTabItem[]>([]);
const tabsMap = computed(() => new Map(allOpenTabs.value.map(t => [t.id, t])));

// ----------------------------------------------------
// Level 1: Multi-Window Management
// ----------------------------------------------------
const windows = ref<WorkspaceWindowItem[]>([
  {
    id: 'win-1',
    name: 'Cửa Sổ 1',
    tilingRoot: null,
    activePanelId: '',
    zoomedPanelId: null,
    layoutPreset: 'single',
  },
]);

const activeWindowId = ref<string>('win-1');

const activeWindow = computed(() => {
  return windows.value.find(w => w.id === activeWindowId.value) || windows.value[0];
});

const currentTilingRoot = computed({
  get: () => activeWindow.value?.tilingRoot || null,
  set: (val) => {
    if (activeWindow.value) activeWindow.value.tilingRoot = val;
  },
});

const currentActivePanelId = computed({
  get: () => activeWindow.value?.activePanelId || '',
  set: (val) => {
    if (activeWindow.value) activeWindow.value.activePanelId = val;
  },
});

const currentZoomedPanelId = computed({
  get: () => activeWindow.value?.zoomedPanelId || null,
  set: (val) => {
    if (activeWindow.value) activeWindow.value.zoomedPanelId = val;
  },
});

const currentPreset = computed(() => activeWindow.value?.layoutPreset || 'single');

const zoomedPanel = computed(() => {
  if (!currentZoomedPanelId.value) return null;
  return findPanelById(currentTilingRoot.value, currentZoomedPanelId.value);
});

const getWindowPanelCount = (win: WorkspaceWindowItem): number => {
  return collectAllPanels(win.tilingRoot).length;
};

const switchWindow = (winId: string) => {
  activeWindowId.value = winId;
  const targetWin = windows.value.find(w => w.id === winId);
  if (targetWin && targetWin.tilingRoot) {
    const panels = collectAllPanels(targetWin.tilingRoot);
    if (panels.length > 0) {
      const activeP = panels.find(p => p.id === targetWin.activePanelId) || panels[0];
      targetWin.activePanelId = activeP.id;
      activeTabId.value = activeP.activeTabId || activeP.tabIds[0] || '';
      nextTick(() => {
        const pane = xtermPaneRefs.get(activeTabId.value);
        pane?.focusTerminal();
      });
    }
  }
};

const renamingWindowId = ref<string | null>(null);
const renamingWindowName = ref('');
const renameWindowInputRef = ref<HTMLInputElement[] | null>(null);

const startRenameWindow = (win: WorkspaceWindowItem) => {
  renamingWindowId.value = win.id;
  renamingWindowName.value = win.name;
  nextTick(() => {
    if (renameWindowInputRef.value && renameWindowInputRef.value[0]) {
      renameWindowInputRef.value[0].focus();
      renameWindowInputRef.value[0].select();
    }
  });
};

const saveRenameWindow = () => {
  if (renamingWindowId.value) {
    const win = windows.value.find(w => w.id === renamingWindowId.value);
    if (win && renamingWindowName.value.trim()) {
      win.name = renamingWindowName.value.trim();
    }
  }
  renamingWindowId.value = null;
};

const closeWindow = (winId: string) => {
  if (windows.value.length <= 1) return;
  const idx = windows.value.findIndex(w => w.id === winId);
  if (idx !== -1) {
    const targetWin = windows.value[idx];
    const tabsInWin = collectAllTabIds(targetWin.tilingRoot);
    // Remove tabs in this window
    tabsInWin.forEach(tabId => {
      xtermPaneRefs.delete(tabId);
      const tabIdx = allOpenTabs.value.findIndex(t => t.id === tabId);
      if (tabIdx !== -1) allOpenTabs.value.splice(tabIdx, 1);
    });

    windows.value.splice(idx, 1);
    if (activeWindowId.value === winId) {
      const nextWin = windows.value[Math.max(0, idx - 1)];
      switchWindow(nextWin.id);
    }
  }
};

const createNewWindow = async () => {
  const newWinId = `win-${Date.now().toString(36)}`;
  const winNumber = windows.value.length + 1;
  const focusedTab = tabsMap.value.get(activeTabId.value);
  const cwd = focusedTab?.cwd;

  try {
    openingTerminal.value = true;
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

    const initialPanel = createPanel([newTab.id], newTab.id);
    const newWin: WorkspaceWindowItem = {
      id: newWinId,
      name: `Cửa Sổ ${winNumber}`,
      tilingRoot: initialPanel,
      activePanelId: initialPanel.id,
      zoomedPanelId: null,
      layoutPreset: 'single',
    };

    windows.value.push(newWin);
    switchWindow(newWinId);
  } catch (error) {
    console.error('[DockHost] Không thể tạo cửa sổ mới:', error);
  } finally {
    openingTerminal.value = false;
  }
};

// ----------------------------------------------------
// UI Controls: Presets, Sync Mode, Sessions
// ----------------------------------------------------
const showPresetMenu = ref(false);
const showSessionsDrawer = ref(false);
const syncMode = ref<SyncInputMode>('off');

const presetLabel = (preset: LayoutPreset) => {
  switch (preset) {
    case '2-columns': return '2 Cột';
    case '2-rows': return '2 Hàng';
    case '2x2-grid': return 'Lưới 2x2';
    case '1+2-tiled': return '1+2 Tiled';
    case '3-columns': return '3 Cột';
    default: return 'Single Pane';
  }
};

const applyPresetMode = (preset: LayoutPreset) => {
  showPresetMenu.value = false;
  if (!activeWindow.value) return;
  activeWindow.value.layoutPreset = preset;

  const existingTabIds = collectAllTabIds(currentTilingRoot.value);
  const createFallback = () => {
    const dummyId = `terminal-fallback-${Date.now()}-${Math.random().toString(36).slice(2, 5)}`;
    const newTab: OpenTabItem = {
      id: dummyId,
      commandId: 0,
      name: 'Terminal',
      isManual: true,
    };
    allOpenTabs.value.push(newTab);
    return dummyId;
  };

  const panelLists: string[][] = [];
  if (preset === 'single') {
    panelLists.push(existingTabIds.length > 0 ? existingTabIds : [createFallback()]);
  } else if (preset === '2-columns' || preset === '2-rows') {
    panelLists.push([existingTabIds[0] || createFallback()]);
    panelLists.push([existingTabIds[1] || createFallback()]);
  } else if (preset === '1+2-tiled' || preset === '3-columns') {
    panelLists.push([existingTabIds[0] || createFallback()]);
    panelLists.push([existingTabIds[1] || createFallback()]);
    panelLists.push([existingTabIds[2] || createFallback()]);
  } else if (preset === '2x2-grid') {
    panelLists.push([existingTabIds[0] || createFallback()]);
    panelLists.push([existingTabIds[1] || createFallback()]);
    panelLists.push([existingTabIds[2] || createFallback()]);
    panelLists.push([existingTabIds[3] || createFallback()]);
  }

  currentTilingRoot.value = buildPresetTree(preset, panelLists, createFallback);
  const panels = collectAllPanels(currentTilingRoot.value);
  if (panels[0]) {
    currentActivePanelId.value = panels[0].id;
    activeTabId.value = panels[0].activeTabId || panels[0].tabIds[0] || '';
  }
};

// ----------------------------------------------------
// Synchronized Keystroke Broadcasting (Tilix Model)
// ----------------------------------------------------
const toggleSyncMode = () => {
  if (syncMode.value === 'off') {
    syncMode.value = 'session';
    showToast('🔗 Đã bật phát sóng bàn phím đồng bộ tới tất cả terminal trong Session', 'info');
  } else {
    syncMode.value = 'off';
    showToast('Đã tắt phát sóng bàn phím đồng bộ', 'info');
  }
};

const handleTerminalData = (sourceTabId: string, chunk: string) => {
  if (syncMode.value !== 'session') return;

  allOpenTabs.value.forEach(tab => {
    if (tab.id !== sourceTabId && tab.commandId > 0) {
      void ipcClient.writePty(tab.commandId, chunk, tab.runEventId).catch(() => undefined);
    }
  });
};

// ----------------------------------------------------
// Level 2 & 3: Panel and Tab Interactions
// ----------------------------------------------------
const handlePanelFocus = (panelId: string) => {
  currentActivePanelId.value = panelId;
  const panel = findPanelById(currentTilingRoot.value, panelId);
  if (panel && panel.activeTabId) {
    activeTabId.value = panel.activeTabId;
    const pane = xtermPaneRefs.get(panel.activeTabId);
    pane?.focusTerminal();
  }
};

const handleSelectTab = (panelId: string, tabId: string) => {
  currentActivePanelId.value = panelId;
  activeTabId.value = tabId;
  if (currentTilingRoot.value) {
    const panel = findPanelById(currentTilingRoot.value, panelId);
    if (panel) {
      panel.activeTabId = tabId;
    }
  }
  nextTick(() => {
    const pane = xtermPaneRefs.get(tabId);
    pane?.focusTerminal();
  });
};

const handleTabCwdChange = (tabId: string, cwd: string) => {
  const tab = tabsMap.value.get(tabId);
  if (tab) {
    tab.cwd = cwd;
  }
};

const handleTabTitleChange = (tabId: string, title: string) => {
  const tab = tabsMap.value.get(tabId);
  if (tab) {
    tab.title = title;
  }
};

const handleTabPhaseChange = (tabId: string, phase: 'prompt' | 'input' | 'running') => {
  const tab = tabsMap.value.get(tabId);
  if (tab) {
    tab.phase = phase;
  }
};

// ----------------------------------------------------
// Level 2 & 3: SPLIT PANEL & INHERIT CWD (WORKING PATH)
// ----------------------------------------------------
const splitActiveRight = () => {
  if (currentActivePanelId.value) {
    handleSplitRight(currentActivePanelId.value);
  } else {
    const panels = collectAllPanels(currentTilingRoot.value);
    if (panels[0]) handleSplitRight(panels[0].id);
  }
};

const splitActiveDown = () => {
  if (currentActivePanelId.value) {
    handleSplitDown(currentActivePanelId.value);
  } else {
    const panels = collectAllPanels(currentTilingRoot.value);
    if (panels[0]) handleSplitDown(panels[0].id);
  }
};

const handleSplitRight = async (targetPanelId: string) => {
  await splitPanelWithNewTerminal(targetPanelId, 'horizontal');
};

const handleSplitDown = async (targetPanelId: string) => {
  await splitPanelWithNewTerminal(targetPanelId, 'vertical');
};

/**
 * Splits target panel into 2 panels, strictly inheriting the CWD of the source tab!
 */
const splitPanelWithNewTerminal = async (
  targetPanelId: string,
  orientation: 'horizontal' | 'vertical'
) => {
  if (openingTerminal.value) return;
  openingTerminal.value = true;
  try {
    // 1. Determine CWD strictly from the active tab in the target panel
    const targetPanel = findPanelById(currentTilingRoot.value, targetPanelId);
    const sourceTab = targetPanel ? tabsMap.value.get(targetPanel.activeTabId) : null;
    const cwd = sourceTab?.cwd;

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

    const newPanel = createPanel([newTab.id], newTab.id);

    if (!currentTilingRoot.value) {
      currentTilingRoot.value = newPanel;
    } else {
      currentTilingRoot.value = splitPanelInTree(
        currentTilingRoot.value,
        targetPanelId,
        orientation,
        newPanel
      );
    }

    currentActivePanelId.value = newPanel.id;
    activeTabId.value = newTab.id;

    nextTick(() => {
      const pane = xtermPaneRefs.get(newTab.id);
      pane?.focusTerminal();
    });
  } catch (error) {
    console.error('[DockHost] Không thể split panel:', error);
  } finally {
    openingTerminal.value = false;
  }
};

/**
 * Opens a new tab inside an existing panel, preserving its current working directory (CWD)
 */
const handleNewTabInPanel = async (targetPanelId: string) => {
  if (openingTerminal.value) return;
  openingTerminal.value = true;
  try {
    const targetPanel = findPanelById(currentTilingRoot.value, targetPanelId);
    const sourceTab = targetPanel ? tabsMap.value.get(targetPanel.activeTabId) : null;
    const cwd = sourceTab?.cwd;

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

    if (currentTilingRoot.value) {
      currentTilingRoot.value = addTabToPanel(
        currentTilingRoot.value,
        targetPanelId,
        newTab.id
      );
    }

    currentActivePanelId.value = targetPanelId;
    activeTabId.value = newTab.id;

    nextTick(() => {
      const pane = xtermPaneRefs.get(newTab.id);
      pane?.focusTerminal();
    });
  } catch (error) {
    console.error('[DockHost] Không thể mở tab mới trong panel:', error);
  } finally {
    openingTerminal.value = false;
  }
};

// ----------------------------------------------------
// Zoom / Maximize Panel
// ----------------------------------------------------
const handleToggleZoom = (panelId: string) => {
  if (currentZoomedPanelId.value === panelId) {
    currentZoomedPanelId.value = null;
  } else {
    currentZoomedPanelId.value = panelId;
  }
};

const handleRatioUpdate = (splitId: string, ratio: number) => {
  if (currentTilingRoot.value) {
    currentTilingRoot.value = updateRatioInTree(currentTilingRoot.value, splitId, ratio);
  }
};

// ----------------------------------------------------
// Closing Tabs and Panels
// ----------------------------------------------------
const handleCloseTab = (tabId: string) => {
  xtermPaneRefs.delete(tabId);

  if (currentTilingRoot.value) {
    const { newTree, removedPanelId } = removeTabFromPanel(currentTilingRoot.value, tabId);
    currentTilingRoot.value = newTree;

    if (removedPanelId && currentZoomedPanelId.value === removedPanelId) {
      currentZoomedPanelId.value = null;
    }
  }

  const idx = allOpenTabs.value.findIndex(t => t.id === tabId);
  if (idx !== -1) {
    allOpenTabs.value.splice(idx, 1);
  }

  // Update active tab & panel
  if (activeTabId.value === tabId) {
    const panels = collectAllPanels(currentTilingRoot.value);
    if (panels.length > 0) {
      const activeP = panels.find(p => p.id === currentActivePanelId.value) || panels[0];
      currentActivePanelId.value = activeP.id;
      activeTabId.value = activeP.activeTabId || activeP.tabIds[0] || '';
    } else {
      currentActivePanelId.value = '';
      activeTabId.value = '';
    }
  }
};

const handleClosePanel = (panelId: string) => {
  const panel = findPanelById(currentTilingRoot.value, panelId);
  if (panel) {
    panel.tabIds.forEach(tabId => {
      xtermPaneRefs.delete(tabId);
      const idx = allOpenTabs.value.findIndex(t => t.id === tabId);
      if (idx !== -1) allOpenTabs.value.splice(idx, 1);
    });
  }

  if (currentZoomedPanelId.value === panelId) {
    currentZoomedPanelId.value = null;
  }

  if (currentTilingRoot.value) {
    currentTilingRoot.value = removePanelFromTree(currentTilingRoot.value, panelId);
  }

  const panels = collectAllPanels(currentTilingRoot.value);
  if (panels.length > 0) {
    currentActivePanelId.value = panels[0].id;
    activeTabId.value = panels[0].activeTabId || panels[0].tabIds[0] || '';
  } else {
    currentActivePanelId.value = '';
    activeTabId.value = '';
  }
};

/**
 * Handles dragging and dropping a tab between different panels or reordering within the same panel.
 */
const handleTransferTab = (payload: {
  sourcePanelId: string;
  targetPanelId: string;
  tabId: string;
  targetIndex?: number;
}) => {
  if (!currentTilingRoot.value) return;

  const newTree = transferTabBetweenPanels(
    currentTilingRoot.value,
    payload.sourcePanelId,
    payload.targetPanelId,
    payload.tabId,
    payload.targetIndex
  );
  currentTilingRoot.value = newTree;

  // Make the target panel and moved tab active
  currentActivePanelId.value = payload.targetPanelId;
  activeTabId.value = payload.tabId;

  // If source panel was zoomed and has been collapsed/removed, reset zoom
  if (currentZoomedPanelId.value && !findPanelById(newTree, currentZoomedPanelId.value)) {
    currentZoomedPanelId.value = null;
  }

  nextTick(() => {
    const pane = xtermPaneRefs.get(payload.tabId);
    pane?.focusTerminal();
  });
};

const { dragState, registerTransferHandler, registerSelectHandler } = useTabDrag();

registerTransferHandler((payload) => {
  handleTransferTab(payload);
});

registerSelectHandler((panelId, tabId) => {
  handleSelectTab(panelId, tabId);
});

const closeAllTabs = () => {
  xtermPaneRefs.clear();
  currentZoomedPanelId.value = null;
  currentTilingRoot.value = null;
  allOpenTabs.value = [];
  activeTabId.value = '';
  currentActivePanelId.value = '';
};

// ----------------------------------------------------
// Terminal Spawning (Toolbar / Programmatic)
// ----------------------------------------------------
const openEmptyTerminal = async () => {
  const panels = collectAllPanels(currentTilingRoot.value);
  if (panels.length > 0) {
    const targetPanelId = currentActivePanelId.value || panels[0].id;
    await handleNewTabInPanel(targetPanelId);
  } else {
    // No panels exist, create first panel in current window
    if (openingTerminal.value) return;
    openingTerminal.value = true;
    try {
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
        isManual: true,
      };
      allOpenTabs.value.push(newTab);

      const panel = createPanel([newTab.id], newTab.id);
      currentTilingRoot.value = panel;
      currentActivePanelId.value = panel.id;
      activeTabId.value = newTab.id;
    } catch (error) {
      console.error('[DockHost] Không thể mở terminal mới:', error);
    } finally {
      openingTerminal.value = false;
    }
  }
};

const openCommandTab = (
  commandId: number,
  commandName: string,
  runEventId?: string,
  shellKind?: string
) => {
  // Check if already open
  const existing = allOpenTabs.value.find(t => t.commandId === commandId);
  if (existing) {
    const containingPanel = findPanelByTabId(currentTilingRoot.value, existing.id);
    if (containingPanel) {
      handleSelectTab(containingPanel.id, existing.id);
      return;
    }
  }

  const newTab: OpenTabItem = {
    id: `cmd-${commandId}-${Date.now()}`,
    commandId,
    name: commandName,
    runEventId,
    shellKind,
    isManual: false,
  };
  allOpenTabs.value.push(newTab);

  const panels = collectAllPanels(currentTilingRoot.value);
  if (panels.length > 0) {
    const targetPanelId = currentActivePanelId.value || panels[0].id;
    currentTilingRoot.value = addTabToPanel(currentTilingRoot.value!, targetPanelId, newTab.id);
    handleSelectTab(targetPanelId, newTab.id);
  } else {
    const panel = createPanel([newTab.id], newTab.id);
    currentTilingRoot.value = panel;
    currentActivePanelId.value = panel.id;
    activeTabId.value = newTab.id;
  }
};

// ----------------------------------------------------
// Terminal Process Control & Modals
// ----------------------------------------------------
const handleStopProcess = (commandId: number) => {
  emit('request-stop-process', commandId);
};

const handleRestartProcess = async (commandId: number) => {
  restartingCmdId.value = commandId;
  try {
    await ipcClient.stopProcess(commandId, false);
    await refreshProcesses();
    await ipcClient.runCommand(commandId);
    await refreshProcesses();
  } catch (err) {
    console.error('[DockHost] Không thể khởi động lại lệnh:', err);
  } finally {
    restartingCmdId.value = null;
  }
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

// ----------------------------------------------------
// Xterm Pane Instance Registry & Stdin Dispatching
// ----------------------------------------------------
const xtermPaneRefs = new Map<string, InstanceType<typeof XtermPane>>();

const registerXtermRef = (tabId: string, instance: unknown) => {
  const inst = instance as InstanceType<typeof XtermPane> | null;
  if (inst) {
    xtermPaneRefs.set(tabId, inst);
  } else {
    xtermPaneRefs.delete(tabId);
  }
};

export interface QuickAccessTarget {
  tabId: string;
  label: string;
  pane: 'paneA' | 'paneB';
  status: ProcessLifecycleStatus;
  busy: boolean;
  shellKind?: string | null;
}

export type SendResult = 'sent' | 'stopped' | 'input-not-at-end';

const tabLabel = (tab: OpenTabItem) => {
  return tab.title || tab.name || `Terminal #${tab.commandId}`;
};

const quickAccessTarget = computed<QuickAccessTarget | null>(() => {
  if (!activeTabId.value) return null;
  const targetTab = tabsMap.value.get(activeTabId.value);
  if (!targetTab) return null;
  const status = getProcessStatus(targetTab.commandId);
  const busy = targetTab.phase === 'running';
  return {
    tabId: targetTab.id,
    label: tabLabel(targetTab),
    pane: 'paneA',
    status,
    busy,
    shellKind: targetTab.shellKind,
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
// Sessions Management (Drawer)
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
    tags: ['New Workspace'],
    isActive: true,
  };
  sessions.value.forEach(s => { s.isActive = false; });
  sessions.value.push(newSess);
  activeSessionId.value = newId;
  applyPresetMode('1+2-tiled');
};

// Toast notification helper
const toastMessage = ref('');
const toastType = ref<'info' | 'warning'>('info');
let toastTimer: ReturnType<typeof setTimeout> | undefined;

const showToast = (msg: string, type: 'info' | 'warning' = 'info') => {
  toastMessage.value = msg;
  toastType.value = type;
  if (toastTimer) clearTimeout(toastTimer);
  toastTimer = setTimeout(() => {
    toastMessage.value = '';
  }, 3500);
};

// ----------------------------------------------------
// Global Hotkeys Listener
// ----------------------------------------------------
const handleGlobalKeydown = (e: KeyboardEvent) => {
  // Ctrl+Alt+R: Split Right
  if (e.ctrlKey && e.altKey && (e.key === 'r' || e.key === 'R')) {
    e.preventDefault();
    splitActiveRight();
    return;
  }
  // Ctrl+Alt+D: Split Down
  if (e.ctrlKey && e.altKey && (e.key === 'd' || e.key === 'D')) {
    e.preventDefault();
    splitActiveDown();
    return;
  }
  // Ctrl+Shift+Z: Toggle Zoom on Active Panel
  if (e.ctrlKey && e.shiftKey && (e.key === 'z' || e.key === 'Z')) {
    e.preventDefault();
    if (currentActivePanelId.value) {
      handleToggleZoom(currentActivePanelId.value);
    }
    return;
  }
  // Ctrl+Alt+S: Toggle Synchronized Keystrokes
  if (e.ctrlKey && e.altKey && (e.key === 's' || e.key === 'S')) {
    e.preventDefault();
    toggleSyncMode();
    return;
  }
  // Ctrl+Shift+T: New Window
  if (e.ctrlKey && e.shiftKey && (e.key === 't' || e.key === 'T')) {
    e.preventDefault();
    void createNewWindow();
    return;
  }
  // Ctrl+N: New terminal tab in active panel
  if (e.ctrlKey && !e.shiftKey && !e.altKey && (e.key === 'n' || e.key === 'N')) {
    e.preventDefault();
    void openEmptyTerminal();
    return;
  }
  // Ctrl+Shift+W: Close active panel
  if (e.ctrlKey && e.shiftKey && (e.key === 'w' || e.key === 'W')) {
    e.preventDefault();
    if (currentActivePanelId.value) {
      handleClosePanel(currentActivePanelId.value);
    }
  }
};

onMounted(() => {
  window.addEventListener('keydown', handleGlobalKeydown);
  const clickOutsideHandler = (ev: MouseEvent) => {
    const target = ev.target as HTMLElement | null;
    if (!target?.closest('.preset-dropdown-container')) {
      showPresetMenu.value = false;
    }
  };
  window.addEventListener('click', clickOutsideHandler);
});

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleGlobalKeydown);
  if (toastTimer) clearTimeout(toastTimer);
});

defineExpose({
  openCommandTab,
  openEmptyTerminal,
  openTabs: allOpenTabs,
  openTabCommandIds: computed(() => allOpenTabs.value.map(t => t.commandId)),
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
  overflow: hidden;
  background-color: var(--bg-app-base, #0c0e14);
}

/* ----------------------------------------------------
   Workspace Toolbar (36px)
---------------------------------------------------- */
.workspace-split-toolbar {
  height: 36px;
  background-color: var(--bg-surface, #141721);
  border-bottom: 1px solid var(--border-subtle, #1a1e2b);
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 12px;
  user-select: none;
  flex-shrink: 0;
  z-index: 30;
}

.toolbar-left-controls,
.toolbar-right-actions {
  display: flex;
  align-items: center;
  gap: 6px;
}

.toolbar-divider {
  width: 1px;
  height: 16px;
  background-color: var(--border-subtle, #1a1e2b);
  margin: 0 4px;
}

.toolbar-action-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 24px;
  padding: 0 8px;
  border-radius: 4px;
  font-size: 11.5px;
  font-weight: 500;
  transition: all 0.15s ease;
}

.toolbar-action-btn.active {
  background-color: rgba(116, 71, 145, 0.25);
  border: 1px solid var(--primary, #744791);
  color: var(--primary-light, #e4b5ff);
}

.new-term-btn {
  background-color: var(--primary, #744791);
  color: #ffffff;
  border: none;
}

.new-term-btn:hover {
  background-color: var(--primary-hover, #8a55ac);
}

/* ----------------------------------------------------
   Windows Bar (Multiple Windows / Workspaces)
---------------------------------------------------- */
.workspace-windows-bar {
  height: 30px;
  background-color: #10131d;
  border-bottom: 1px solid var(--border-subtle, #1a1e2b);
  display: flex;
  align-items: center;
  padding: 0 8px;
  user-select: none;
  flex-shrink: 0;
  z-index: 25;
}

.windows-tab-list {
  display: flex;
  align-items: center;
  gap: 4px;
  overflow-x: auto;
  overflow-y: hidden;
  height: 100%;
  width: 100%;
}

.window-tab-item {
  display: flex;
  align-items: center;
  gap: 6px;
  height: 24px;
  padding: 0 10px;
  border-radius: 4px;
  background-color: transparent;
  border: 1px solid transparent;
  color: var(--text-secondary, #94a3b8);
  font-size: 11.5px;
  cursor: pointer;
  transition: all 0.12s ease;
  white-space: nowrap;
}

.window-tab-item:hover {
  background-color: rgba(255, 255, 255, 0.04);
  color: var(--text-primary, #e2e8f0);
}

.window-tab-item.active {
  background-color: var(--bg-surface, #141721);
  border-color: rgba(116, 71, 145, 0.6);
  color: #ffffff;
  font-weight: 600;
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.3);
}

.window-icon {
  color: var(--primary-light, #e4b5ff);
  opacity: 0.8;
}

.window-title {
  max-width: 120px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  display: inline-block;
}

.window-panels-count {
  font-size: 9.5px;
  color: var(--text-muted, #64748b);
  background-color: rgba(255, 255, 255, 0.06);
  padding: 1px 4px;
  border-radius: 3px;
}

.window-close-btn {
  background: transparent;
  border: none;
  padding: 1px;
  border-radius: 3px;
  color: var(--text-muted, #64748b);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  opacity: 0.6;
}

.window-close-btn:hover {
  background-color: rgba(239, 68, 68, 0.2);
  color: #ef4444;
  opacity: 1;
}

.window-rename-input {
  background-color: #0c0e14;
  border: 1px solid var(--primary, #744791);
  color: #ffffff;
  font-size: 11px;
  padding: 1px 4px;
  border-radius: 3px;
  width: 100px;
  outline: none;
}

.window-add-tab-btn {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  height: 22px;
  padding: 0 8px;
  border-radius: 4px;
  background-color: transparent;
  border: 1px dashed rgba(255, 255, 255, 0.15);
  color: var(--text-muted, #64748b);
  font-size: 11px;
  cursor: pointer;
  transition: all 0.12s ease;
  white-space: nowrap;
}

.window-add-tab-btn:hover {
  border-color: var(--primary, #744791);
  color: var(--primary-light, #e4b5ff);
  background-color: rgba(116, 71, 145, 0.15);
}

/* ----------------------------------------------------
   Preset Dropdown
---------------------------------------------------- */
.preset-dropdown-container {
  position: relative;
}

.preset-btn {
  gap: 6px;
}

.preset-svg-icon {
  width: 14px;
  height: 14px;
}

.preset-dropdown-menu {
  position: absolute;
  top: calc(100% + 4px);
  left: 0;
  width: 260px;
  background-color: #141721;
  border: 1px solid var(--border-medium, #282d3f);
  border-radius: 6px;
  padding: 4px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.6);
  z-index: 100;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.preset-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  border-radius: 4px;
  background: transparent;
  border: none;
  color: var(--text-secondary, #94a3b8);
  font-size: 12px;
  text-align: left;
  cursor: pointer;
  transition: all 0.12s ease;
}

.preset-item:hover {
  background-color: rgba(255, 255, 255, 0.05);
  color: #ffffff;
}

.preset-item.active {
  background-color: rgba(116, 71, 145, 0.25);
  color: var(--primary-light, #e4b5ff);
  font-weight: 600;
}

/* Sync Button and Pill */
.sync-btn.active {
  background-color: rgba(116, 71, 145, 0.35);
  border: 1px solid var(--primary, #744791);
  color: #e4b5ff;
}

.zoom-pill {
  animation: pulse-border 2s infinite;
}

/* ----------------------------------------------------
   Main Workspace Body: Sessions Drawer + Tiling Canvas
---------------------------------------------------- */
.workspace-tiling-body {
  display: flex;
  flex: 1;
  width: 100%;
  height: calc(100% - 66px);
  overflow: hidden;
  position: relative;
}

.tiling-canvas-container {
  flex: 1;
  height: 100%;
  overflow: hidden;
  position: relative;
  background-color: var(--bg-app-base, #0c0e14);
}

.zoomed-panel-wrapper {
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
  background-color: var(--bg-surface, #141721);
  border: 1px solid var(--border-subtle, #1a1e2b);
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
  color: var(--text-primary, #e2e8f0);
  margin: 0;
}

.empty-desc {
  font-size: 12px;
  color: var(--text-muted, #64748b);
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
  border: 1px solid var(--border-subtle, #1a1e2b);
  color: var(--text-primary, #e2e8f0);
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

.spin {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

/* ----------------------------------------------------
   Floating Tab & Drag Guard (Global DnD across Panels)
---------------------------------------------------- */
.floating-drag-tab {
  position: fixed;
  z-index: 99999;
  pointer-events: none;
  background-color: #1a1e2e;
  border: 1px solid var(--primary-light, #e4b5ff);
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.7), 0 0 16px rgba(116, 71, 145, 0.45);
  opacity: 0.95;
  cursor: grabbing;
  border-radius: 4px;
  color: #f1f5f9;
  will-change: left, top;
  transform: translateY(-2px);
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 0 10px;
  font-size: 11.5px;
  font-family: var(--font-sans, system-ui);
  white-space: nowrap;
  box-sizing: border-box;
}

.floating-drag-tab .tab-title-text {
  max-width: 140px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.floating-drag-tab .tab-status-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background-color: #64748b;
  flex-shrink: 0;
}

.floating-drag-tab .tab-status-dot.running {
  background-color: #22c55e;
  box-shadow: 0 0 6px rgba(34, 197, 94, 0.6);
}

.tab-drag-guard {
  position: fixed;
  inset: 0;
  z-index: 99998;
  cursor: grabbing;
  user-select: none;
  background: transparent;
}
</style>
