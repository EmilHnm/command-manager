<template>
  <div
    class="tiling-panel-container"
    :data-tiling-panel-id="panel.id"
    :class="{
      'is-active': isActive,
      'is-zoomed': isZoomed,
      'is-sync-linked': isSyncLinked,
      'is-drop-target': isPanelDropTarget,
    }"
    @pointerdown="handlePanelPointerDown"
  >
    <!-- Panel Header: Tab Strip on the Left, Panel Controls on the Right -->
    <div class="panel-headerbar" :class="{ 'header-active': isActive }">
      <!-- Left: Tabs List + New Tab Button -->
      <div class="panel-tabs-scroll custom-scrollbar">
        <div class="panel-tabs-list">
          <template v-for="item in displayedTabs" :key="item.key">
            <!-- Tab Placeholder (shows insertion target while dragging) -->
            <div
              v-if="item.isPlaceholder"
              class="panel-tab-item tab-placeholder"
              :style="{ width: item.width ? `${item.width}px` : '100px' }"
            >
              <span class="placeholder-dot" />
              <span class="tab-title-text">{{ item.title }}</span>
            </div>

            <!-- Regular Tab Item -->
            <div
              v-else-if="item.tabId"
              class="panel-tab-item"
              :class="{
                'active': item.tabId === panel.activeTabId,
                'running': getTabStatus(item.tabId) === 'running',
              }"
              :data-tab-id="item.tabId"
              :title="getTabTooltip(item.tabId)"
              @pointerdown="handleTabPointerDown($event, item.tabId)"
              @dblclick.stop="handleTabDblClick(item.tabId)"
            >
              <!-- Tab status indicator dot -->
              <span
                class="tab-status-dot"
                :class="[getTabStatus(item.tabId), { active: getTabStatus(item.tabId) === 'running' }]"
              />

              <!-- Tab Title with tooltip -->
              <span
                class="tab-title-text"
                :title="getTabTooltip(item.tabId)"
              >
                {{ getTabTitle(item.tabId) }}
              </span>

              <!-- PID badge if running -->
              <span
                v-if="getTabPid(item.tabId)"
                class="tab-pid-badge font-mono"
              >
                {{ getTabPid(item.tabId) }}
              </span>

              <!-- Tab Close button -->
              <button
                class="tab-close-btn"
                title="Đóng tab này"
                @pointerdown.stop
                @click.stop="$emit('close-tab', item.tabId)"
              >
                <X :size="11" />
              </button>
            </div>
          </template>

          <!-- New Tab Button inside this panel (inherits active tab CWD) -->
          <button
            class="panel-new-tab-btn"
            title="Mở tab mới trong khung này (Giữ nguyên CWD)"
            @click.stop="$emit('new-tab', panel.id)"
          >
            <Plus :size="12" />
          </button>
        </div>
      </div>

      <!-- Right: Action Buttons (CWD badge, Split Right/Down, Zoom, Reattach, Clear, Stop, Close Panel) -->
      <div class="panel-actions-toolbar">
        <!-- Compact CWD Display of active tab -->
        <span
          v-if="activeTabCwd"
          class="panel-cwd-badge"
          :title="`Thư mục hiện tại: ${activeTabCwd}`"
        >
          {{ compactCwd(activeTabCwd) }}
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

        <!-- Split to Right -->
        <button
          class="panel-action-btn"
          title="Chia đôi panel sang phải (Ctrl+Alt+R)"
          @click.stop="$emit('split-right', panel.id)"
        >
          <Columns2 :size="12" />
        </button>

        <!-- Split to Down -->
        <button
          class="panel-action-btn"
          title="Chia đôi panel xuống dưới (Ctrl+Alt+D)"
          @click.stop="$emit('split-down', panel.id)"
        >
          <Rows2 :size="12" />
        </button>

        <!-- Zoom / Maximize Toggle -->
        <button
          class="panel-action-btn"
          :class="{ 'zoom-active': isZoomed }"
          :title="isZoomed ? 'Khôi phục bố cục lưới (Ctrl+Shift+Z)' : 'Phóng to panel toàn màn hình (Ctrl+Shift+Z)'"
          @click.stop="$emit('toggle-zoom', panel.id)"
        >
          <Minimize2 v-if="isZoomed" :size="12" />
          <Maximize2 v-else :size="12" />
        </button>

        <!-- Reattach Buffer -->
        <button
          class="panel-action-btn"
          title="Xả lại dữ liệu từ Ring Buffer in-memory"
          @click.stop="handleReattachActive"
        >
          <RefreshCw :size="12" />
        </button>

        <!-- Clear Screen -->
        <button
          class="panel-action-btn"
          title="Xóa màn hình terminal"
          @click.stop="handleClearActive"
        >
          <Eraser :size="12" />
        </button>

        <!-- Stop Process Button (if active tab running) -->
        <button
          v-if="activeTabStatus === 'running' && activeTab"
          class="panel-action-btn btn-stop"
          title="Dừng tiến trình (SIGTERM)"
          @click.stop="$emit('stop-process', activeTab.commandId)"
        >
          <Square :size="11" />
        </button>

        <!-- Close Entire Panel -->
        <button
          class="panel-action-btn btn-close-panel"
          title="Đóng toàn bộ panel này"
          @click.stop="$emit('close-panel', panel.id)"
        >
          <X :size="12" />
        </button>
      </div>
    </div>

    <!-- Panel Viewport: Renders all tabs in this panel (only activeTabId is shown) -->
    <div
      class="panel-viewport"
      :class="{
        'viewport-drag-target': isPanelDropTarget,
      }"
    >
      <!-- Drop Overlay when dragging tab from another panel -->
      <div v-if="isPanelDropTarget" class="panel-drop-overlay">
        <div class="drop-overlay-card">
          <FolderInput :size="24" class="drop-overlay-icon" />
          <span class="drop-overlay-text">Thả để chuyển Tab vào Panel này</span>
        </div>
      </div>
      <template v-for="tabId in panel.tabIds" :key="tabId">
        <div
          v-show="tabId === panel.activeTabId"
          class="panel-tab-pane"
        >
          <XtermPane
            v-if="tabsMap.get(tabId)"
            :ref="(el) => registerPaneRef(tabId, el)"
            :command-id="tabsMap.get(tabId)!.commandId"
            :command-name="tabsMap.get(tabId)!.name"
            :run-event-id="tabsMap.get(tabId)!.runEventId"
            :pid="getTabPid(tabId)"
            :process-status="getTabStatus(tabId)"
            :shell-kind="tabsMap.get(tabId)!.shellKind"
            :history-level="tabsMap.get(tabId)!.historyLevel"
            :ghost-text-enabled="ghostTextEnabled"
            :font-family="fontFamily"
            :font-size="fontSize"
            :restarting="restartingCmdId === tabsMap.get(tabId)!.commandId"
            :cwd="tabsMap.get(tabId)!.cwd"
            :hide-toolbar="true"
            @stop-process="$emit('stop-process', $event)"
            @restart-process="$emit('restart-process', $event)"
            @cwd-change="(cwd) => handleTabCwdChange(tabId, cwd)"
            @title-change="(title) => handleTabTitleChange(tabId, title)"
            @phase-change="(phase) => handleTabPhaseChange(tabId, phase)"
            @data="(chunk) => $emit('data', tabId, chunk)"
          />
        </div>
      </template>

      <!-- Empty Tab Fallback -->
      <div v-if="panel.tabIds.length === 0" class="empty-panel-placeholder">
        <span>Không có tab nào trong panel này</span>
        <button
          class="btn btn-primary btn-xs"
          @click="$emit('new-tab', panel.id)"
        >
          <Plus :size="12" />
          <span>Mở Terminal</span>
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue';
import {
  Columns2,
  Rows2,
  Maximize2,
  Minimize2,
  RefreshCw,
  Eraser,
  Square,
  X,
  Plus,
  Link2,
  FolderInput,
} from 'lucide-vue-next';
import XtermPane from '@/components/terminal/XtermPane.vue';
import type { TilingPanelNode, SyncInputMode } from '@/types/tiling';
import type { ProcessLifecycleStatus } from '@/types/models';
import { useRunSession } from '@/composables/useRunSession';

export interface OpenTabItem {
  id: string;
  commandId: number;
  name: string;
  runEventId?: string;
  shellKind?: string;
  historyLevel?: number;
  isManual?: boolean;
  cwd?: string;
  title?: string;
  phase?: 'prompt' | 'input' | 'running';
}

import { useTabDrag } from '@/composables/useTabDrag';

const { dragState, isDragging, startTabDrag } = useTabDrag();

const props = withDefaults(
  defineProps<{
    panel: TilingPanelNode;
    tabsMap: Map<string, OpenTabItem>;
    isActive: boolean;
    isZoomed: boolean;
    syncMode: SyncInputMode;
    ghostTextEnabled?: boolean;
    fontFamily?: string;
    fontSize?: number;
    restartingCmdId?: number | null;
  }>(),
  {
    ghostTextEnabled: true,
    fontFamily: 'JetBrains Mono',
    fontSize: 13,
    restartingCmdId: null,
  }
);

const emit = defineEmits<{
  (e: 'focus', panelId: string): void;
  (e: 'select-tab', panelId: string, tabId: string): void;
  (e: 'new-tab', panelId: string): void;
  (e: 'close-tab', tabId: string): void;
  (e: 'close-panel', panelId: string): void;
  (e: 'split-right', panelId: string): void;
  (e: 'split-down', panelId: string): void;
  (e: 'toggle-zoom', panelId: string): void;
  (e: 'rename-tab', tab: OpenTabItem): void;
  (e: 'stop-process', commandId: number): void;
  (e: 'restart-process', commandId: number): void;
  (e: 'register-xterm', tabId: string, instance: InstanceType<typeof XtermPane> | null): void;
  (e: 'data', tabId: string, chunk: string): void;
  (e: 'cwd-change', tabId: string, cwd: string): void;
  (e: 'title-change', tabId: string, title: string): void;
  (e: 'phase-change', tabId: string, phase: 'prompt' | 'input' | 'running'): void;
  (e: 'transfer-tab', payload: {
    sourcePanelId: string;
    targetPanelId: string;
    tabId: string;
    targetIndex?: number;
  }): void;
}>();

const { getProcessStatus, getProcessInfo } = useRunSession();

const activeTab = computed<OpenTabItem | null>(() => {
  return props.tabsMap.get(props.panel.activeTabId) || null;
});

const activeTabCwd = computed(() => {
  return activeTab.value?.cwd || '';
});

const activeTabStatus = computed<ProcessLifecycleStatus>(() => {
  if (!activeTab.value) return 'idle';
  return getProcessStatus(activeTab.value.commandId);
});

const isSyncLinked = computed(() => {
  return props.syncMode === 'session' && activeTabStatus.value === 'running';
});

const getTabTitle = (tabId: string): string => {
  const tab = props.tabsMap.get(tabId);
  if (!tab) return 'Terminal';
  return tab.title || tab.name || 'Terminal';
};

const getTabTooltip = (tabId: string): string => {
  const tab = props.tabsMap.get(tabId);
  if (!tab) return '';
  const fullTitle = tab.title || tab.name || 'Terminal';
  const nameExtra = tab.title && tab.name && tab.title !== tab.name ? ` [${tab.name}]` : '';
  const pid = getTabPid(tabId);
  const pidStr = pid ? ` | PID: ${pid}` : '';
  const cwdStr = tab.cwd ? `\nThư mục: ${tab.cwd}` : '';
  return `${fullTitle}${nameExtra}${pidStr}${cwdStr}\n(Nháy đúp để đổi tên)`;
};

const getTabStatus = (tabId: string): ProcessLifecycleStatus => {
  const tab = props.tabsMap.get(tabId);
  if (!tab) return 'idle';
  return getProcessStatus(tab.commandId);
};

const getTabPid = (tabId: string): number | undefined => {
  const tab = props.tabsMap.get(tabId);
  if (!tab) return undefined;
  return getProcessInfo(tab.commandId)?.pid ?? undefined;
};

const compactCwd = (cwd: string): string => {
  if (!cwd) return '';
  const normalized = cwd.replace(/\\/g, '/');
  const parts = normalized.split('/').filter(Boolean);
  if (parts.length <= 2) return normalized;
  return `…/${parts.slice(-2).join('/')}`;
};

const handlePanelPointerDown = () => {
  emit('focus', props.panel.id);
};

const handleTabDblClick = (tabId: string) => {
  const tab = props.tabsMap.get(tabId);
  if (tab) {
    emit('rename-tab', tab);
  }
};

const handleTabCwdChange = (tabId: string, cwd: string) => {
  const tab = props.tabsMap.get(tabId);
  if (tab) {
    tab.cwd = cwd;
  }
  emit('cwd-change', tabId, cwd);
};

const handleTabTitleChange = (tabId: string, title: string) => {
  const tab = props.tabsMap.get(tabId);
  if (tab) {
    tab.title = title;
  }
  emit('title-change', tabId, title);
};

const handleTabPhaseChange = (tabId: string, phase: 'prompt' | 'input' | 'running') => {
  const tab = props.tabsMap.get(tabId);
  if (tab) {
    tab.phase = phase;
  }
  emit('phase-change', tabId, phase);
};

// Map of XtermPane instances for tabs in this panel
const paneInstanceMap = new Map<string, InstanceType<typeof XtermPane>>();

const registerPaneRef = (tabId: string, el: unknown) => {
  const inst = el as InstanceType<typeof XtermPane> | null;
  if (inst) {
    paneInstanceMap.set(tabId, inst);
  } else {
    paneInstanceMap.delete(tabId);
  }
  emit('register-xterm', tabId, inst);
};

const handleReattachActive = () => {
  const inst = paneInstanceMap.get(props.panel.activeTabId);
  inst?.handleReattach?.();
};

const handleClearActive = () => {
  const inst = paneInstanceMap.get(props.panel.activeTabId);
  inst?.handleClear?.();
};

// ----------------------------------------------------
// Tab Drag and Drop (Pointer-based with Placeholder)
// ----------------------------------------------------
const handleTabPointerDown = (e: PointerEvent, tabId: string) => {
  const tab = props.tabsMap.get(tabId);
  if (!tab) return;
  emit('focus', props.panel.id);
  startTabDrag(
    e,
    {
      id: tab.id,
      name: tab.name,
      commandId: tab.commandId,
      title: tab.title,
      cwd: tab.cwd,
    },
    props.panel.id
  );
};

const isPanelDropTarget = computed(() => {
  return Boolean(
    dragState.value?.isDragging &&
    dragState.value?.hoverPanelId === props.panel.id &&
    dragState.value?.fromPanelId !== props.panel.id
  );
});

const isTabBeingDragged = (tabId: string) => {
  return Boolean(dragState.value?.isDragging && dragState.value?.tab.id === tabId);
};

interface DisplayedTabItem {
  key: string;
  isPlaceholder: boolean;
  tabId?: string;
  title: string;
  width?: number;
}

const displayedTabs = computed<DisplayedTabItem[]>(() => {
  const currentTabIds = props.panel.tabIds;
  if (!dragState.value || !dragState.value.isDragging) {
    return currentTabIds.map(id => ({
      key: id,
      isPlaceholder: false,
      tabId: id,
      title: getTabTitle(id),
    }));
  }

  const draggedTabId = dragState.value.tab.id;
  const isHoveredPanel = dragState.value.hoverPanelId === props.panel.id;
  const isSourcePanel = dragState.value.fromPanelId === props.panel.id;

  const remainingTabIds = currentTabIds.filter(id => id !== draggedTabId);

  if (!isHoveredPanel) {
    const tabsToRender = isSourcePanel ? remainingTabIds : currentTabIds;
    return tabsToRender.map(id => ({
      key: id,
      isPlaceholder: false,
      tabId: id,
      title: getTabTitle(id),
    }));
  }

  const slot = Math.max(
    0,
    Math.min(
      dragState.value.hoverSlotIndex ?? remainingTabIds.length,
      remainingTabIds.length
    )
  );

  const items: DisplayedTabItem[] = [];
  for (let i = 0; i <= remainingTabIds.length; i++) {
    if (i === slot) {
      items.push({
        key: `placeholder-${draggedTabId}`,
        isPlaceholder: true,
        title: dragState.value.tab.title || dragState.value.tab.name,
        width: dragState.value.width,
      });
    }
    if (i < remainingTabIds.length) {
      const id = remainingTabIds[i];
      items.push({
        key: id,
        isPlaceholder: false,
        tabId: id,
        title: getTabTitle(id),
      });
    }
  }

  return items;
});

const focusTerminal = () => {
  const inst = paneInstanceMap.get(props.panel.activeTabId);
  inst?.focusTerminal?.();
};

defineExpose({
  focusTerminal,
  handleReattachActive,
  handleClearActive,
});
</script>

<style scoped>
.tiling-panel-container {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  background-color: var(--bg-app-base, #0c0e14);
  border: 1px solid var(--border-subtle, #1a1e2b);
  overflow: hidden;
  position: relative;
  transition: border-color 0.15s ease, box-shadow 0.15s ease;
}

.tiling-panel-container.is-active {
  border-color: var(--primary, #744791);
  box-shadow: inset 0 0 0 1px rgba(116, 71, 145, 0.4);
}

.tiling-panel-container.is-sync-linked {
  box-shadow: inset 0 0 0 1px #744791, 0 0 8px rgba(116, 71, 145, 0.35);
}

/* ----------------------------------------------------
   Panel Headerbar: Tabs & Controls (32px)
---------------------------------------------------- */
.panel-headerbar {
  height: 32px;
  background-color: var(--bg-surface, #141721);
  border-bottom: 1px solid var(--border-subtle, #1a1e2b);
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 4px 0 0;
  user-select: none;
  flex-shrink: 0;
  z-index: 10;
  gap: 8px;
}

.panel-headerbar.header-active {
  background-color: #161b29;
  border-bottom-color: rgba(116, 71, 145, 0.4);
}

/* Tabs Scroll Container */
.panel-tabs-scroll {
  display: flex;
  align-items: center;
  overflow-x: auto;
  overflow-y: hidden;
  flex: 1;
  min-width: 0;
  height: 100%;
}

.panel-tabs-list {
  display: flex;
  align-items: center;
  height: 100%;
  gap: 3px;
  padding: 0 4px;
  min-width: 0;
}

/* Individual Tab inside Panel */
.panel-tab-item {
  display: flex;
  align-items: center;
  gap: 6px;
  height: 26px;
  padding: 0 8px;
  border-radius: 4px;
  background-color: transparent;
  border: 1px solid transparent;
  color: var(--text-secondary, #94a3b8);
  font-size: 11.5px;
  cursor: pointer;
  transition: all 0.12s ease;
  white-space: nowrap;
  max-width: 180px;
  min-width: 48px;
  flex-shrink: 0;
  overflow: hidden;
}

.panel-tab-item:hover {
  background-color: rgba(255, 255, 255, 0.05);
  color: var(--text-primary, #e2e8f0);
}

.panel-tab-item.active {
  background-color: var(--bg-surface-active, #1f2538);
  border-color: rgba(116, 71, 145, 0.5);
  color: #ffffff;
  font-weight: 500;
  max-width: 220px;
}

.tab-title-text {
  flex: 1 1 auto;
  min-width: 0;
  max-width: 140px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  display: inline-block;
}

.tab-status-dot {
  width: 6px;
  height: 6px;
  border-radius: 9999px;
  background-color: var(--text-muted, #64748b);
  flex-shrink: 0;
}

.tab-status-dot.running {
  background-color: var(--status-success, #22c55e);
  box-shadow: 0 0 6px rgba(34, 197, 94, 0.7);
}

.tab-status-dot.failed {
  background-color: var(--status-failed, #ef4444);
}

.tab-pid-badge {
  font-size: 9.5px;
  color: var(--text-muted, #64748b);
  background-color: rgba(255, 255, 255, 0.06);
  padding: 1px 3px;
  border-radius: 3px;
}

.tab-close-btn {
  background: transparent;
  border: none;
  padding: 2px;
  border-radius: 3px;
  color: var(--text-muted, #64748b);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  opacity: 0.6;
  transition: all 0.12s ease;
}

.tab-close-btn:hover {
  background-color: rgba(239, 68, 68, 0.2);
  color: #ef4444;
  opacity: 1;
}

.panel-new-tab-btn {
  background: transparent;
  border: 1px dashed rgba(255, 255, 255, 0.12);
  width: 22px;
  height: 22px;
  border-radius: 4px;
  color: var(--text-muted, #64748b);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: all 0.12s ease;
  flex-shrink: 0;
}

.panel-new-tab-btn:hover {
  border-color: var(--primary, #744791);
  color: var(--primary-light, #e4b5ff);
  background-color: rgba(116, 71, 145, 0.15);
}

/* ----------------------------------------------------
   Panel Controls Toolbar (Right side)
---------------------------------------------------- */
.panel-actions-toolbar {
  display: flex;
  align-items: center;
  gap: 3px;
  flex-shrink: 0;
}

.panel-cwd-badge {
  font-size: 10px;
  font-family: var(--font-mono, monospace);
  color: var(--text-muted, #64748b);
  background-color: rgba(255, 255, 255, 0.04);
  padding: 2px 6px;
  border-radius: 3px;
  max-width: 140px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.sync-pill {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  font-size: 9px;
  font-weight: 700;
  padding: 2px 5px;
  border-radius: 3px;
  background-color: rgba(116, 71, 145, 0.3);
  border: 1px solid var(--primary, #744791);
  color: var(--primary-light, #e4b5ff);
  letter-spacing: 0.5px;
}

.panel-action-btn {
  width: 24px;
  height: 24px;
  border-radius: 4px;
  background: transparent;
  border: none;
  color: var(--text-secondary, #94a3b8);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: all 0.12s ease;
}

.panel-action-btn:hover {
  background-color: rgba(255, 255, 255, 0.08);
  color: #ffffff;
}

.panel-action-btn.zoom-active {
  background-color: rgba(116, 71, 145, 0.3);
  color: var(--primary-light, #e4b5ff);
}

.panel-action-btn.btn-stop {
  color: #ef4444;
}

.panel-action-btn.btn-stop:hover {
  background-color: rgba(239, 68, 68, 0.2);
}

.panel-action-btn.btn-close-panel:hover {
  background-color: rgba(239, 68, 68, 0.2);
  color: #ef4444;
}

/* ----------------------------------------------------
   Panel Viewport: hosts XtermPane for tabs
---------------------------------------------------- */
.panel-viewport {
  flex: 1;
  position: relative;
  overflow: hidden;
  background-color: #0c0e14;
  transition: box-shadow 0.15s ease, border-color 0.15s ease;
}

/* Prevent xterm canvas/iframes from stealing pointer events during drag */
.panel-viewport.is-dragging-global .panel-tab-pane {
  pointer-events: none;
}

.panel-viewport.viewport-drag-target {
  box-shadow: inset 0 0 0 2px var(--primary, #744791);
}

.panel-tab-pane {
  width: 100%;
  height: 100%;
  position: relative;
}

.empty-panel-placeholder {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  color: var(--text-muted, #64748b);
  font-size: 12px;
}

/* ----------------------------------------------------
   Drag & Drop Styles (Tab Placeholder & Viewport)
---------------------------------------------------- */
.tiling-panel-container.is-drop-target {
  border-color: var(--primary, #744791);
  box-shadow: inset 0 0 0 1px var(--primary, #744791);
}

.panel-tab-item {
  cursor: grab;
  user-select: none;
}

.panel-tab-item:active {
  cursor: grabbing;
}

.panel-tab-item.is-drag-source {
  opacity: 0.25;
}

/* Tab Placeholder (dashed slot preview) */
.tab-placeholder {
  height: 24px;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 0 8px;
  background-color: rgba(116, 71, 145, 0.22) !important;
  border: 1.5px dashed var(--primary-light, #e4b5ff) !important;
  border-radius: 4px;
  color: var(--primary-light, #e4b5ff);
  font-size: 11px;
  user-select: none;
  white-space: nowrap;
  box-shadow: inset 0 0 12px rgba(116, 71, 145, 0.35);
  pointer-events: none;
  box-sizing: border-box;
  animation: pulseTabPlaceholder 1.5s infinite ease-in-out;
}

.placeholder-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background-color: var(--primary-light, #e4b5ff);
  box-shadow: 0 0 6px var(--primary-light, #e4b5ff);
  flex-shrink: 0;
}

@keyframes pulseTabPlaceholder {
  0%, 100% {
    opacity: 0.75;
    border-color: rgba(228, 181, 255, 0.5);
  }
  50% {
    opacity: 1;
    border-color: rgba(228, 181, 255, 1);
  }
}

/* Drop Overlay on Panel Viewport */
.panel-drop-overlay {
  position: absolute;
  inset: 0;
  z-index: 50;
  background-color: rgba(12, 14, 20, 0.72);
  backdrop-filter: blur(2px);
  display: flex;
  align-items: center;
  justify-content: center;
  pointer-events: none;
  animation: fadeInDropOverlay 0.15s ease-out;
}

.drop-overlay-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  padding: 16px 24px;
  border-radius: 8px;
  background: rgba(26, 29, 39, 0.95);
  border: 1px dashed var(--primary, #744791);
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.45);
}

.drop-overlay-icon {
  color: var(--primary-light, #e4b5ff);
  animation: bounceDropIcon 1s infinite alternate ease-in-out;
}

.drop-overlay-text {
  font-size: 12px;
  font-weight: 600;
  color: #f1f5f9;
  letter-spacing: 0.3px;
}

@keyframes fadeInDropOverlay {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}

@keyframes bounceDropIcon {
  from {
    transform: translateY(0);
  }
  to {
    transform: translateY(-4px);
  }
}
</style>
