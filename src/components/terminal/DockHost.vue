<template>
  <div class="dock-host-container">
    <!-- Top Split View Controls & Action Bar -->
    <div class="workspace-split-toolbar">
      <div class="toolbar-left-controls">
        <!-- Split Mode Switcher -->
        <div class="split-mode-group" role="group" aria-label="Chế độ chia màn hình">
          <button
            class="split-mode-btn"
            :class="{ active: splitMode === 'single' }"
            title="Chế độ 1 khung đơn (100% diện tích)"
            @click="setSplitMode('single')"
          >
            <Square :size="13" />
            <span>Single</span>
          </button>
          <button
            class="split-mode-btn"
            :class="{ active: splitMode === 'horizontal' }"
            title="Chia đôi theo chiều dọc: Trái | Phải (Ctrl+\)"
            @click="setSplitMode('horizontal')"
          >
            <Columns2 :size="13" />
            <span>Split Dọc</span>
          </button>
          <button
            class="split-mode-btn"
            :class="{ active: splitMode === 'vertical' }"
            title="Chia đôi theo chiều ngang: Trên / Dưới (Ctrl+Shift+\)"
            @click="setSplitMode('vertical')"
          >
            <Rows2 :size="13" />
            <span>Split Ngang</span>
          </button>
        </div>

        <!-- Additional Split Controls when in Split Mode -->
        <template v-if="splitMode !== 'single'">
          <div class="toolbar-divider" />

          <button
            class="btn btn-ghost btn-sm toolbar-action-btn"
            title="Hoán đổi vị trí Khung Trái ⇄ Khung Phải"
            @click="swapPanes"
          >
            <ArrowLeftRight :size="13" />
            <span>Hoán Đổi</span>
          </button>

          <button
            class="btn btn-ghost btn-sm toolbar-action-btn ratio-badge-btn"
            title="Đặt lại tỉ lệ cân bằng 50:50 (Ctrl+Alt+R hoặc nháy đúp vào thanh phân cách)"
            @click="resetSplitRatio"
          >
            <RotateCcw :size="11" class="ratio-reset-icon" />
            <span>{{ Math.round(splitRatio) }} : {{ Math.round(100 - splitRatio) }}</span>
          </button>
        </template>
      </div>

      <div class="toolbar-right-actions">
        <button
          class="btn btn-ghost btn-sm toolbar-action-btn"
          title="Mở một terminal shell mới vào khung đang active"
          :disabled="openingTerminal"
          @click="openEmptyTerminal()"
        >
          <Plus :size="13" />
          <span>Terminal mới</span>
        </button>
        <button
          v-if="allOpenTabs.length > 0"
          class="btn btn-ghost btn-sm toolbar-action-btn"
          title="Đóng tất cả tab đang mở (Ẩn UI, lệnh vẫn tiếp tục chạy ngầm)"
          @click="closeAllTabs"
        >
          Ẩn tất cả tab
        </button>
      </div>
    </div>

    <!-- Main Workspace Area: Single View or Dual-Pane Split View -->
    <div
      ref="splitContainerRef"
      class="split-workspace-body"
      :class="[
        `mode-${splitMode}`,
        { 'is-dragging-sash': isDraggingSash }
      ]"
      :style="splitGridStyle"
    >
      <!-- PANE A (Khung Trái / Khung Trên) -->
      <div
        class="terminal-pane pane-a"
        :class="{
          'is-focused': splitMode !== 'single' && activePane === 'paneA',
          'is-single': splitMode === 'single',
          'is-drop-target': dragState?.isDragging && dragState?.hoverPane === 'paneA' && dragState?.fromPane !== 'paneA'
        }"
        @pointerdown="handlePaneFocus('paneA')"
      >
        <!-- Pane A Tab Strip Header -->
        <div class="pane-tab-strip">
          <div v-if="splitMode !== 'single'" class="pane-indicator" :class="{ active: activePane === 'paneA' }">
            <span class="pane-dot" />
            <span class="pane-name">PANE A</span>
          </div>

          <div
            ref="tabsListARef"
            class="tabs-list"
            @wheel.passive="handleTabsWheel($event, 'paneA')"
          >
            <div
              v-for="item in displayedTabsA"
              :key="item.key"
              class="terminal-tab"
              :class="{
                'tab-placeholder': item.isPlaceholder && showPlaceholder,
                'is-drag-hidden': item.isPlaceholder && !showPlaceholder,
                active: !item.isPlaceholder && item.tab?.id === activeTabIdA,
                'is-manual': !item.isPlaceholder && isManualTab(item.tab),
              }"
              :style="item.isPlaceholder && item.width ? { width: `${item.width}px` } : undefined"
              @pointerdown="!item.isPlaceholder && item.tab ? handleTabPointerDown($event, item.tab, 'paneA') : undefined"
              @dblclick="!item.isPlaceholder && item.tab ? handleTabDblClick(item.tab) : undefined"
              @contextmenu.prevent="!item.isPlaceholder && item.tab ? openTabContextMenu($event, item.tab, 'paneA') : undefined"
            >
              <!-- PLACEHOLDER TAB VIEW -->
              <template v-if="item.isPlaceholder && showPlaceholder">
                <span class="tab-status-dot placeholder-dot" />
                <span class="tab-title">{{ item.name }}</span>
              </template>

              <!-- NORMAL TAB VIEW -->
              <template v-else-if="item.tab && !item.isPlaceholder">
                <div class="tab-active-bar" />
                <span
                  class="tab-status-dot"
                  :class="{ active: getProcessStatus(item.tab.commandId) === 'running' }"
                />
                <span
                  class="tab-title"
                  :class="{ 'is-manual': isManualTab(item.tab) }"
                  :title="isManualTab(item.tab) ? `${item.tab.name} (Nháy đúp để đổi tên)` : item.tab.name"
                >
                  {{ item.tab.name }}
                </span>
                <span v-if="getProcessInfo(item.tab.commandId)?.pid" class="tab-pid">
                  {{ getProcessInfo(item.tab.commandId)?.pid }}
                </span>

                <!-- Quick Move to Opposite Pane Button (Split Mode) -->
                <button
                  v-if="splitMode !== 'single'"
                  class="tab-move-btn"
                  title="Chuyển tab này sang Khung B (Ctrl+Alt+M)"
                  @pointerdown.stop
                  @click.stop="moveTabToOppositePane(item.tab.id)"
                >
                  <ArrowLeftRight :size="10" />
                </button>

                <!-- Close tab button: chỉ ẩn tab, KHÔNG kill process -->
                <button
                  class="tab-close-btn"
                  title="Ẩn tab này (Tiến trình vẫn tiếp tục chạy ngầm)"
                  @pointerdown.stop
                  @click.stop="handleCloseTab('paneA', item.tab.id)"
                >
                  <X :size="12" />
                </button>
              </template>
            </div>

            <div v-if="paneATabs.length === 0" class="no-tabs-msg">
              Chưa mở tab terminal nào ở Khung A
            </div>
          </div>

          <!-- Actions for Pane A: Add terminal & Kill all terminals -->
          <div class="pane-strip-actions">
            <button
              class="pane-strip-btn add-btn"
              :title="splitMode === 'single' ? 'Mở thêm terminal mới' : 'Mở thêm terminal mới vào Khung A'"
              :disabled="openingTerminal"
              @click="openEmptyTerminal('paneA')"
            >
              <Plus :size="13" />
            </button>

            <button
              v-if="paneATabs.length > 0"
              class="pane-strip-btn kill-btn"
              :title="`Dừng toàn bộ ${paneATabs.length} terminal trong ${splitMode === 'single' ? 'Workspace' : 'Khung A'}`"
              :disabled="killInProgress"
              @click="promptKillPanel('paneA')"
            >
              <LoaderCircle v-if="killInProgress && killingPaneTarget === 'paneA'" :size="12" class="spin" />
              <OctagonX v-else :size="12" />
              <span class="kill-btn-label">{{ killInProgress && killingPaneTarget === 'paneA' ? 'Đang Kill...' : 'Kill Hết' }}</span>
            </button>
          </div>
        </div>

        <!-- Pane A Terminal Viewport -->
        <div class="pane-viewport">
          <template v-for="tab in paneATabs" :key="tab.id">
            <div v-show="tab.id === activeTabIdA" class="pane-wrapper">
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
                :restarting="restartingCmdId === tab.commandId"
                @stop-process="handleStopProcess"
                @restart-process="handleRestartProcess"
              />
            </div>
          </template>

          <!-- Empty State khi không có tab nào ở Pane A -->
          <div v-if="paneATabs.length === 0" class="empty-pane-state">
            <div class="empty-pane-box">
              <Terminal :size="28" class="empty-icon" />
              <h5>Khung A sẵn sàng</h5>
              <p>Chọn lệnh từ danh mục bên trái hoặc mở terminal mới.</p>
              <button
                class="btn btn-primary btn-sm"
                :disabled="openingTerminal"
                @click="openEmptyTerminal('paneA')"
              >
                <Plus :size="13" />
                <span>Mở terminal mới</span>
              </button>
            </div>
          </div>
        </div>
      </div>

      <!-- RESIZABLE SASH / SPLITTER (chỉ hiển thị khi ở chế độ Split) -->
      <div
        v-if="splitMode !== 'single'"
        class="resizable-sash"
        :class="`sash-${splitMode}`"
        title="Kéo để điều chỉnh tỉ lệ (20% – 80%) | Nháy đúp để reset 50:50"
        @pointerdown="handleSashPointerDown"
        @dblclick="resetSplitRatio"
      >
        <div class="sash-grip-line" />
      </div>

      <!-- PANE B (Khung Phải / Khung Dưới - chỉ hiển thị khi ở chế độ Split) -->
      <div
        v-if="splitMode !== 'single'"
        class="terminal-pane pane-b"
        :class="{
          'is-focused': activePane === 'paneB',
          'is-drop-target': dragState?.isDragging && dragState?.hoverPane === 'paneB' && dragState?.fromPane !== 'paneB'
        }"
        @pointerdown="handlePaneFocus('paneB')"
      >
        <!-- Pane B Tab Strip Header -->
        <div class="pane-tab-strip">
          <div class="pane-indicator" :class="{ active: activePane === 'paneB' }">
            <span class="pane-dot" />
            <span class="pane-name">PANE B</span>
          </div>

          <div
            ref="tabsListBRef"
            class="tabs-list"
            @wheel.passive="handleTabsWheel($event, 'paneB')"
          >
            <div
              v-for="item in displayedTabsB"
              :key="item.key"
              class="terminal-tab"
              :class="{
                'tab-placeholder': item.isPlaceholder && showPlaceholder,
                'is-drag-hidden': item.isPlaceholder && !showPlaceholder,
                active: !item.isPlaceholder && item.tab?.id === activeTabIdB,
                'is-manual': !item.isPlaceholder && isManualTab(item.tab),
              }"
              :style="item.isPlaceholder && item.width ? { width: `${item.width}px` } : undefined"
              @pointerdown="!item.isPlaceholder && item.tab ? handleTabPointerDown($event, item.tab, 'paneB') : undefined"
              @dblclick="!item.isPlaceholder && item.tab ? handleTabDblClick(item.tab) : undefined"
              @contextmenu.prevent="!item.isPlaceholder && item.tab ? openTabContextMenu($event, item.tab, 'paneB') : undefined"
            >
              <!-- PLACEHOLDER TAB VIEW -->
              <template v-if="item.isPlaceholder && showPlaceholder">
                <span class="tab-status-dot placeholder-dot" />
                <span class="tab-title">{{ item.name }}</span>
              </template>

              <!-- NORMAL TAB VIEW -->
              <template v-else-if="item.tab && !item.isPlaceholder">
                <div class="tab-active-bar" />
                <span
                  class="tab-status-dot"
                  :class="{ active: getProcessStatus(item.tab.commandId) === 'running' }"
                />
                <span
                  class="tab-title"
                  :class="{ 'is-manual': isManualTab(item.tab) }"
                  :title="isManualTab(item.tab) ? `${item.tab.name} (Nháy đúp để đổi tên)` : item.tab.name"
                >
                  {{ item.tab.name }}
                </span>
                <span v-if="getProcessInfo(item.tab.commandId)?.pid" class="tab-pid">
                  {{ getProcessInfo(item.tab.commandId)?.pid }}
                </span>

                <!-- Quick Move to Opposite Pane Button (Split Mode) -->
                <button
                  class="tab-move-btn"
                  title="Chuyển tab này sang Khung A (Ctrl+Alt+M)"
                  @pointerdown.stop
                  @click.stop="moveTabToOppositePane(item.tab.id)"
                >
                  <ArrowLeftRight :size="10" />
                </button>

                <!-- Close tab button: chỉ ẩn tab, KHÔNG kill process -->
                <button
                  class="tab-close-btn"
                  title="Ẩn tab này (Tiến trình vẫn tiếp tục chạy ngầm)"
                  @pointerdown.stop
                  @click.stop="handleCloseTab('paneB', item.tab.id)"
                >
                  <X :size="12" />
                </button>
              </template>
            </div>

            <div v-if="paneBTabs.length === 0" class="no-tabs-msg">
              Chưa mở tab terminal nào ở Khung B
            </div>
          </div>

          <!-- Actions for Pane B: Add terminal & Kill all terminals -->
          <div class="pane-strip-actions">
            <button
              class="pane-strip-btn add-btn"
              title="Mở thêm terminal mới vào Khung B"
              :disabled="openingTerminal"
              @click="openEmptyTerminal('paneB')"
            >
              <Plus :size="13" />
            </button>

            <button
              v-if="paneBTabs.length > 0"
              class="pane-strip-btn kill-btn"
              :title="`Dừng toàn bộ ${paneBTabs.length} terminal trong Khung B`"
              :disabled="killInProgress"
              @click="promptKillPanel('paneB')"
            >
              <LoaderCircle v-if="killInProgress && killingPaneTarget === 'paneB'" :size="12" class="spin" />
              <OctagonX v-else :size="12" />
              <span class="kill-btn-label">{{ killInProgress && killingPaneTarget === 'paneB' ? 'Đang Kill...' : 'Kill Hết' }}</span>
            </button>
          </div>
        </div>

        <!-- Pane B Terminal Viewport -->
        <div class="pane-viewport">
          <template v-for="tab in paneBTabs" :key="tab.id">
            <div v-show="tab.id === activeTabIdB" class="pane-wrapper">
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
                :restarting="restartingCmdId === tab.commandId"
                @stop-process="handleStopProcess"
                @restart-process="handleRestartProcess"
              />
            </div>
          </template>

          <!-- Empty State khi không có tab nào ở Pane B -->
          <div v-if="paneBTabs.length === 0" class="empty-pane-state">
            <div class="empty-pane-box">
              <Terminal :size="28" class="empty-icon" />
              <h5>Khung B sẵn sàng</h5>
              <p>Kéo thả tab sang đây hoặc mở một terminal shell mới.</p>
              <button
                class="btn btn-primary btn-sm"
                :disabled="openingTerminal"
                @click="openEmptyTerminal('paneB')"
              >
                <Plus :size="13" />
                <span>Mở terminal mới</span>
              </button>
            </div>
          </div>
        </div>
      </div>

      <!-- Drop Zones Overlay khi kéo tab trong Single View Mode để kích hoạt Split -->
      <div
        v-if="dragState?.isDragging && splitMode === 'single' && paneATabs.length > 1"
        class="single-mode-split-hints"
      >
        <div
          class="split-drop-zone right-zone"
          :class="{ 'is-hovered': dragState.hoverPane === 'split-right' }"
        >
          <Columns2 :size="22" />
          <span>Thả vào đây để chia đôi Cột Dọc</span>
        </div>
        <div
          class="split-drop-zone bottom-zone"
          :class="{ 'is-hovered': dragState.hoverPane === 'split-bottom' }"
        >
          <Rows2 :size="22" />
          <span>Thả vào đây để chia đôi Hàng Ngang</span>
        </div>
      </div>
    </div>

    <!-- Floating Dragged Tab (follows cursor during tab drag) -->
    <Teleport to="body">
      <div
        v-if="dragState?.isDragging"
        class="terminal-tab floating-drag-tab"
        :style="{
          top: `${dragState.top + Math.max(-8, Math.min(8, dragState.currentY - dragState.startY))}px`,
          left: `${dragState.currentX - dragState.grabOffsetX}px`,
          width: `${dragState.width}px`,
          height: `${dragState.height}px`,
        }"
      >
        <div class="tab-active-bar" style="opacity: 1; transform: scaleX(1);" />
        <span
          class="tab-status-dot"
          :class="{ active: getProcessStatus(dragState.tabCommandId) === 'running' }"
        />
        <span class="tab-title">{{ dragState.tabName }}</span>
      </div>
    </Teleport>

    <!-- Pointer Overlay Guard khi đang kéo thanh Sash để tránh mất mouse events -->
    <Teleport to="body">
      <div
        v-if="isDraggingSash"
        class="sash-drag-guard"
        :style="{ cursor: splitMode === 'horizontal' ? 'col-resize' : 'row-resize' }"
      />
    </Teleport>

    <!-- Tab Context Menu (Right-Click) -->
    <Teleport to="body">
      <div
        v-if="contextMenuVisible && contextMenuTarget"
        class="tab-context-menu"
        :style="{ top: `${contextMenuPos.y}px`, left: `${contextMenuPos.x}px` }"
        @pointerdown.stop
      >
        <button
          class="context-menu-item"
          @click="handleContextMenuMoveOpposite"
        >
          <ArrowLeftRight :size="13" />
          <span>{{ splitMode === 'single' ? 'Mở sang Khung Phải (Split Dọc)' : (contextMenuTarget.pane === 'paneA' ? 'Chuyển sang Khung B' : 'Chuyển sang Khung A') }}</span>
        </button>

        <button
          v-if="splitMode === 'single'"
          class="context-menu-item"
          @click="handleContextMenuSplitVertical"
        >
          <Rows2 :size="13" />
          <span>Mở sang Khung Dưới (Split Ngang)</span>
        </button>

        <button
          v-if="isManualTab(contextMenuTarget.tab)"
          class="context-menu-item"
          @click="handleContextMenuRename"
        >
          <Terminal :size="13" />
          <span>Đổi tên terminal...</span>
        </button>

        <div class="context-menu-divider" />

        <button
          class="context-menu-item danger"
          @click="handleContextMenuKillPanel"
        >
          <OctagonX :size="13" />
          <span>Dừng toàn bộ terminal trong khung này...</span>
        </button>

        <button
          class="context-menu-item danger"
          @click="handleContextMenuCloseTab"
        >
          <X :size="13" />
          <span>Ẩn thẻ tab này</span>
        </button>
      </div>
    </Teleport>

    <!-- Rename Manual Terminal Modal (MOD-14) -->
    <RenameTerminalModal
      :visible="renameModalVisible"
      :tab-id="renamingTab?.id || ''"
      :current-name="renamingTab?.name || ''"
      :pid="renamingTab ? getProcessInfo(renamingTab.commandId)?.pid ?? undefined : undefined"
      :shell-kind="renamingTab?.shellKind"
      @save="handleRenameSave"
      @cancel="handleRenameCancel"
    />

    <!-- Kill Panel Confirmation Modal (MOD-KILL) -->
    <KillPanelModal
      :visible="showKillModal"
      :panel-name="killingPaneTarget || 'paneA'"
      :tabs="targetKillTabs"
      :loading="killInProgress"
      :is-split-mode="splitMode !== 'single'"
      @confirm="handleConfirmKillPanel"
      @cancel="handleCancelKillPanel"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount, nextTick } from 'vue';
import {
  Square,
  Columns2,
  Rows2,
  ArrowLeftRight,
  RotateCcw,
  Plus,
  X,
  Terminal,
  OctagonX,
  LoaderCircle,
} from 'lucide-vue-next';
import XtermPane from './XtermPane.vue';
import RenameTerminalModal from '@/components/dialogs/RenameTerminalModal.vue';
import KillPanelModal from '@/components/dialogs/KillPanelModal.vue';
import { useRunSession } from '@/composables/useRunSession';
import { ipcClient } from '@/ipc/client';
import { useSplitLayout, type OpenTabItem, type ActivePane } from '@/composables/useSplitLayout';

export type { OpenTabItem };

const props = defineProps<{
  initialTabs?: OpenTabItem[];
}>();

const emit = defineEmits<{
  (e: 'request-stop-process', commandId: number): void;
}>();

const { getProcessStatus, getProcessInfo, refreshProcesses, updateProcessName } = useRunSession();

// Sử dụng composable Split Layout
const {
  splitMode,
  splitRatio,
  activePane,
  paneATabs,
  paneBTabs,
  activeTabIdA,
  activeTabIdB,
  allOpenTabs,
  allOpenTabCommandIds,
  setSplitMode,
  toggleSplitHorizontal,
  toggleSplitDirection,
  resetSplitRatio,
  setSplitRatio,
  swapPanes,
  selectTab,
  moveTabToOppositePane,
  transferTab,
  closeTab: splitCloseTab,
  closePane: splitClosePane,
  closeAllTabs: splitCloseAllTabs,
  addTab: splitAddTab,
} = useSplitLayout(props.initialTabs || []);

const openingTerminal = ref(false);
const ghostTextEnabled = ref(true);
const terminalFontFamily = ref('JetBrains Mono');
const terminalFontSize = ref(13);

const splitContainerRef = ref<HTMLElement | null>(null);
const tabsListARef = ref<HTMLElement | null>(null);
const tabsListBRef = ref<HTMLElement | null>(null);

// Tính toán Grid / Flex CSS style cho split layout
const splitGridStyle = computed(() => {
  if (splitMode.value === 'single') {
    return {};
  }
  return {
    '--split-ratio': `${splitRatio.value}%`,
  };
});

// Focus active pane
const handlePaneFocus = (pane: ActivePane) => {
  activePane.value = pane;
};

// Đóng tab an toàn
const handleCloseTab = (pane: ActivePane, tabId: string) => {
  const closed = splitCloseTab(pane, tabId);
  if (closed && isManualTab(closed)) {
    setManualMeta(closed.commandId, {
      name: closed.name,
      shellKind: closed.shellKind,
      historyLevel: closed.historyLevel,
    });
    ipcClient.setTerminalName?.(closed.commandId, closed.name);
    updateProcessName(closed.commandId, closed.name);
  }
};

const closeAllTabs = () => {
  allOpenTabs.value.forEach(tab => {
    if (isManualTab(tab)) {
      setManualMeta(tab.commandId, {
        name: tab.name,
        shellKind: tab.shellKind,
        historyLevel: tab.historyLevel,
      });
      ipcClient.setTerminalName?.(tab.commandId, tab.name);
      updateProcessName(tab.commandId, tab.name);
    }
  });
  splitCloseAllTabs();
};

// ----------------------------------------------------
// Thanh phân cách Resizable Sash Drag Logic
// ----------------------------------------------------
const isDraggingSash = ref(false);

const handleSashPointerDown = (e: PointerEvent) => {
  if (e.button !== 0) return;
  e.preventDefault();
  isDraggingSash.value = true;
  const container = splitContainerRef.value;
  if (!container) return;

  const rect = container.getBoundingClientRect();
  const isHorizontal = splitMode.value === 'horizontal';

  const onPointerMove = (moveEvt: PointerEvent) => {
    let newRatio: number;
    if (isHorizontal) {
      const offsetX = moveEvt.clientX - rect.left;
      const clampedX = Math.max(220, Math.min(rect.width - 220, offsetX));
      newRatio = (clampedX / rect.width) * 100;
    } else {
      const offsetY = moveEvt.clientY - rect.top;
      const clampedY = Math.max(150, Math.min(rect.height - 150, offsetY));
      newRatio = (clampedY / rect.height) * 100;
    }
    setSplitRatio(newRatio);
  };

  const onPointerUp = () => {
    isDraggingSash.value = false;
    window.removeEventListener('pointermove', onPointerMove);
    window.removeEventListener('pointerup', onPointerUp);
    window.removeEventListener('pointercancel', onPointerUp);
  };

  window.addEventListener('pointermove', onPointerMove);
  window.addEventListener('pointerup', onPointerUp);
  window.addEventListener('pointercancel', onPointerUp);
};

// ----------------------------------------------------
// Quản lý metadata tên và shell của terminal mở thủ công
// ----------------------------------------------------
interface ManualTerminalRecord {
  name: string;
  shellKind?: string;
  historyLevel?: number;
}

const MANUAL_TERMINAL_STORAGE_KEY = 'cm_manual_terminal_meta_v1';

function loadManualMeta(): Map<number, ManualTerminalRecord> {
  const map = new Map<number, ManualTerminalRecord>();
  if (typeof window === 'undefined') return map;
  try {
    const raw = localStorage.getItem(MANUAL_TERMINAL_STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw);
      for (const [k, v] of Object.entries(parsed)) {
        if (v && typeof v === 'object') {
          map.set(Number(k), v as ManualTerminalRecord);
        }
      }
    }
  } catch {}
  return map;
}

function saveManualMeta(map: Map<number, ManualTerminalRecord>) {
  if (typeof window === 'undefined') return;
  try {
    const obj: Record<string, ManualTerminalRecord> = {};
    map.forEach((v, k) => {
      obj[String(k)] = v;
    });
    localStorage.setItem(MANUAL_TERMINAL_STORAGE_KEY, JSON.stringify(obj));
  } catch {}
}

const manualTerminalMeta = loadManualMeta();

const setManualMeta = (commandId: number, record: ManualTerminalRecord) => {
  manualTerminalMeta.set(commandId, record);
  saveManualMeta(manualTerminalMeta);
};

const isManualTab = (tab?: OpenTabItem): boolean => {
  if (!tab) return false;
  return tab.isManual === true
    || manualTerminalMeta.has(tab.commandId)
    || tab.id.startsWith('terminal-')
    || tab.commandId <= 0
    || (!!tab.shellKind && tab.shellKind !== 'command');
};

// Modal Đổi tên Terminal thủ công (MOD-14)
const renameModalVisible = ref(false);
const renamingTab = ref<OpenTabItem | null>(null);

const handleTabDblClick = (tab?: OpenTabItem) => {
  if (!tab || !isManualTab(tab)) return;
  renamingTab.value = tab;
  renameModalVisible.value = true;
};

const handleRenameSave = (tabId: string, newName: string) => {
  const target = allOpenTabs.value.find(t => t.id === tabId);
  if (target) {
    target.name = newName;
    setManualMeta(target.commandId, {
      name: newName,
      shellKind: target.shellKind,
      historyLevel: target.historyLevel,
    });
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

// ----------------------------------------------------
// Tab Right-Click Context Menu
// ----------------------------------------------------
const contextMenuVisible = ref(false);
const contextMenuPos = ref({ x: 0, y: 0 });
const contextMenuTarget = ref<{ tab: OpenTabItem; pane: ActivePane } | null>(null);

const openTabContextMenu = (e: MouseEvent, tab: OpenTabItem, pane: ActivePane) => {
  contextMenuPos.value = { x: e.clientX, y: e.clientY };
  contextMenuTarget.value = { tab, pane };
  contextMenuVisible.value = true;
};

const closeContextMenu = () => {
  contextMenuVisible.value = false;
  contextMenuTarget.value = null;
};

const handleContextMenuMoveOpposite = () => {
  if (contextMenuTarget.value) {
    if (splitMode.value === 'single') {
      setSplitMode('horizontal');
    }
    moveTabToOppositePane(contextMenuTarget.value.tab.id);
  }
  closeContextMenu();
};

const handleContextMenuSplitVertical = () => {
  if (contextMenuTarget.value) {
    setSplitMode('vertical');
    moveTabToOppositePane(contextMenuTarget.value.tab.id);
  }
  closeContextMenu();
};

const handleContextMenuRename = () => {
  if (contextMenuTarget.value) {
    handleTabDblClick(contextMenuTarget.value.tab);
  }
  closeContextMenu();
};

const handleContextMenuCloseTab = () => {
  if (contextMenuTarget.value) {
    handleCloseTab(contextMenuTarget.value.pane, contextMenuTarget.value.tab.id);
  }
  closeContextMenu();
};

const handleContextMenuKillPanel = () => {
  if (contextMenuTarget.value) {
    promptKillPanel(contextMenuTarget.value.pane);
  }
  closeContextMenu();
};

// ----------------------------------------------------
// Dừng toàn bộ terminal trong panel (Kill Panel Modal)
// ----------------------------------------------------
const showKillModal = ref(false);
const killingPaneTarget = ref<ActivePane | null>(null);
const killInProgress = ref(false);

const targetKillTabs = computed<OpenTabItem[]>(() => {
  if (killingPaneTarget.value === 'paneB') {
    return paneBTabs.value;
  }
  return paneATabs.value;
});

const promptKillPanel = (pane: ActivePane) => {
  const tabs = pane === 'paneA' ? paneATabs.value : paneBTabs.value;
  if (tabs.length === 0) return;
  killingPaneTarget.value = pane;
  showKillModal.value = true;
};

const handleCancelKillPanel = () => {
  if (killInProgress.value) return;
  showKillModal.value = false;
  killingPaneTarget.value = null;
};

const handleConfirmKillPanel = async (force: boolean) => {
  if (!killingPaneTarget.value) return;
  const targetPane = killingPaneTarget.value;
  const tabsToKill = targetPane === 'paneB' ? [...paneBTabs.value] : [...paneATabs.value];

  killInProgress.value = true;
  await nextTick();
  await new Promise((resolve) => requestAnimationFrame(resolve));
  try {
    // 1. Dừng các tiến trình PTY tương ứng
    await Promise.allSettled(
      tabsToKill.map(async tab => {
        try {
          await ipcClient.stopProcess(tab.commandId, force);
        } catch (err) {
          // Bỏ qua lỗi nếu tiến trình đã hoàn thành hoặc dừng trước đó
          console.warn(`[DockHost] Tiến trình ${tab.commandId} (${tab.name}) đã dừng hoặc lỗi:`, err);
        }
      })
    );

    // 2. Đóng toàn bộ tab của panel và auto-collapse nếu split mode
    const closedTabs = splitClosePane(targetPane);
    closedTabs.forEach(tab => {
      if (isManualTab(tab)) {
        setManualMeta(tab.commandId, {
          name: tab.name,
          shellKind: tab.shellKind,
          historyLevel: tab.historyLevel,
        });
        ipcClient.setTerminalName?.(tab.commandId, tab.name);
        updateProcessName(tab.commandId, tab.name);
      }
    });

    // 3. Làm mới trạng thái tiến trình
    await refreshProcesses();
    showKillModal.value = false;
    killingPaneTarget.value = null;
  } catch (error) {
    console.error('[DockHost] Lỗi khi kill toàn bộ panel:', error);
  } finally {
    killInProgress.value = false;
  }
};

// ----------------------------------------------------
// Tab Drag and Drop (Internal & Cross-Pane Move)
// ----------------------------------------------------
interface TabBarItem {
  key: string;
  isPlaceholder: boolean;
  tab?: OpenTabItem;
  name: string;
  width?: number;
}

type HoverDropTarget = ActivePane | 'split-right' | 'split-bottom';

interface PointerDragState {
  tabId: string;
  tabName: string;
  tabCommandId: number;
  fromPane: ActivePane;
  hoverPane: HoverDropTarget;
  startX: number;
  startY: number;
  currentX: number;
  currentY: number;
  grabOffsetX: number;
  top: number;
  width: number;
  height: number;
  initialIndex: number;
  isDragging: boolean;
  tabMidpoints: number[];
  initialScrollLeft: number;
}

const dragState = ref<PointerDragState | null>(null);
const placeholderIndex = ref<number | null>(null);
const showPlaceholder = ref(false);
const isDropping = ref(false);
let placeholderTimer: ReturnType<typeof setTimeout> | null = null;
let dropTimer: ReturnType<typeof setTimeout> | null = null;

const createDisplayedTabList = (pane: ActivePane) => {
  const tabs = pane === 'paneA' ? paneATabs.value : paneBTabs.value;
  if (!dragState.value?.isDragging && !isDropping.value) {
    return tabs.map(t => ({
      key: t.id,
      isPlaceholder: false,
      tab: t,
      name: t.name,
    }));
  }

  const hoverPane = dragState.value?.hoverPane;
  const fromPane = dragState.value?.fromPane;
  const draggedTabId = dragState.value?.tabId;
  const draggedTab = allOpenTabs.value.find(t => t.id === draggedTabId);

  // Nếu pane này không phải là nơi chuột đang hover:
  if (hoverPane !== pane) {
    // Nếu chính là pane nguồn của tab: ẩn tab đang bị kéo đi
    if (fromPane === pane) {
      return tabs.filter(t => t.id !== draggedTabId).map(t => ({
        key: t.id,
        isPlaceholder: false,
        tab: t,
        name: t.name,
      }));
    }
    // Ngược lại, hiển thị tabs bình thường
    return tabs.map(t => ({
      key: t.id,
      isPlaceholder: false,
      tab: t,
      name: t.name,
    }));
  }

  // Khung này ĐANG được hover để nhận tab:
  if (!draggedTab || placeholderIndex.value === null) {
    return tabs.map(t => ({
      key: t.id,
      isPlaceholder: false,
      tab: t,
      name: t.name,
    }));
  }

  const remaining = tabs.filter(t => t.id !== draggedTabId);
  const slot = Math.max(0, Math.min(placeholderIndex.value, remaining.length));

  const items: TabBarItem[] = [];
  for (let i = 0; i <= remaining.length; i++) {
    if (i === slot) {
      items.push({
        key: `placeholder-${draggedTab.id}`,
        isPlaceholder: true,
        tab: draggedTab,
        name: dragState.value?.tabName || draggedTab.name,
        width: dragState.value?.width,
      });
    }
    if (i < remaining.length) {
      items.push({
        key: remaining[i].id,
        isPlaceholder: false,
        tab: remaining[i],
        name: remaining[i].name,
      });
    }
  }
  return items;
};

const displayedTabsA = computed<TabBarItem[]>(() => createDisplayedTabList('paneA'));
const displayedTabsB = computed<TabBarItem[]>(() => createDisplayedTabList('paneB'));

const getTabsListEl = (pane: ActivePane): HTMLElement | null => {
  return pane === 'paneA' ? tabsListARef.value : tabsListBRef.value;
};

const handleTabsWheel = (e: WheelEvent, pane: ActivePane) => {
  const el = getTabsListEl(pane);
  if (!el) return;
  if (e.deltaY !== 0) {
    el.scrollLeft += e.deltaY;
  }
};

const handleTabPointerDown = (e: PointerEvent, tab: OpenTabItem, pane: ActivePane) => {
  if (e.button !== 0 || isDropping.value) return;

  closeContextMenu();

  const targetEl = e.currentTarget as HTMLElement | null;
  if (!targetEl) return;

  if (placeholderTimer) clearTimeout(placeholderTimer);
  if (dropTimer) clearTimeout(dropTimer);
  showPlaceholder.value = false;

  const rect = targetEl.getBoundingClientRect();
  const tabsList = pane === 'paneA' ? paneATabs.value : paneBTabs.value;
  const fromIndex = tabsList.findIndex(t => t.id === tab.id);
  if (fromIndex === -1) return;

  const containerEl = getTabsListEl(pane);
  const initialScroll = containerEl ? containerEl.scrollLeft : 0;

  const tabElements = containerEl
    ? Array.from(containerEl.querySelectorAll<HTMLElement>('.terminal-tab:not(.tab-placeholder)'))
    : [];

  const midpoints = tabElements.map(el => {
    const r = el.getBoundingClientRect();
    return r.left + r.width / 2;
  });

  dragState.value = {
    tabId: tab.id,
    tabName: tab.name,
    tabCommandId: tab.commandId,
    fromPane: pane,
    hoverPane: pane,
    startX: e.clientX,
    startY: e.clientY,
    currentX: e.clientX,
    currentY: e.clientY,
    grabOffsetX: e.clientX - rect.left,
    top: rect.top,
    width: rect.width,
    height: rect.height,
    initialIndex: fromIndex,
    isDragging: false,
    tabMidpoints: midpoints,
    initialScrollLeft: initialScroll,
  };

  placeholderIndex.value = fromIndex;

  window.addEventListener('pointermove', onTabPointerMove);
  window.addEventListener('pointerup', onTabPointerUp);
  window.addEventListener('pointercancel', onTabPointerUp);
};

const onTabPointerMove = (e: PointerEvent) => {
  if (!dragState.value || isDropping.value) return;

  const dx = e.clientX - dragState.value.startX;
  const dy = e.clientY - dragState.value.startY;

  if (!dragState.value.isDragging) {
    if (Math.hypot(dx, dy) > 4) {
      dragState.value.isDragging = true;
      showPlaceholder.value = false;
      if (placeholderTimer) clearTimeout(placeholderTimer);
      placeholderTimer = setTimeout(() => {
        showPlaceholder.value = true;
      }, 40);
    } else {
      return;
    }
  }

  dragState.value.currentX = e.clientX;
  dragState.value.currentY = e.clientY;

  // Xác định vị trí chuột đang hover qua Pane nào
  let targetDrop: HoverDropTarget = dragState.value.fromPane;

  if (splitMode.value === 'single') {
    const container = splitContainerRef.value;
    if (container && paneATabs.value.length > 1) {
      const cRect = container.getBoundingClientRect();
      // Kéo về 30% cạnh phải -> kích hoạt split-right
      if (e.clientX > cRect.right - Math.max(180, cRect.width * 0.3)) {
        targetDrop = 'split-right';
      } else if (e.clientY > cRect.bottom - Math.max(120, cRect.height * 0.3)) {
        targetDrop = 'split-bottom';
      } else {
        targetDrop = 'paneA';
      }
    } else {
      targetDrop = 'paneA';
    }
  } else {
    // Đang ở Split Mode: kiểm tra xem chuột đang ở Pane A hay Pane B
    const container = splitContainerRef.value;
    if (container) {
      const paneBEl = container.querySelector('.pane-b') as HTMLElement | null;
      if (paneBEl) {
        const bRect = paneBEl.getBoundingClientRect();
        if (
          e.clientX >= bRect.left &&
          e.clientX <= bRect.right &&
          e.clientY >= bRect.top &&
          e.clientY <= bRect.bottom
        ) {
          targetDrop = 'paneB';
        } else {
          targetDrop = 'paneA';
        }
      }
    }
  }

  dragState.value.hoverPane = targetDrop;

  if (targetDrop === 'split-right' || targetDrop === 'split-bottom') {
    placeholderIndex.value = null;
    return;
  }

  const hoverPane = targetDrop as ActivePane;
  const containerEl = getTabsListEl(hoverPane);
  if (!containerEl) return;

  const currentScroll = containerEl.scrollLeft;
  const effectiveX = e.clientX + currentScroll;

  // Đo midpoints của tabs trong pane đang hover
  const tabElements = Array.from(
    containerEl.querySelectorAll<HTMLElement>('.terminal-tab:not(.tab-placeholder)')
  );

  const midpoints = tabElements.map(el => {
    const r = el.getBoundingClientRect();
    return r.left + r.width / 2;
  });

  let newSlot = 0;
  if (hoverPane === dragState.value.fromPane) {
    // Kéo nội bộ cùng một khung
    const fromIndex = dragState.value.initialIndex;
    for (let i = 0; i < midpoints.length; i++) {
      if (i === fromIndex) continue;
      if (effectiveX > midpoints[i]) {
        newSlot++;
      }
    }
  } else {
    // Kéo từ khung khác sang khung này
    const tabStripRect = containerEl.getBoundingClientRect();
    // Nếu con trỏ chuột nằm sâu dưới viewport (không phải trên tab strip), thả vào cuối danh sách tab
    if (e.clientY > tabStripRect.bottom + 8) {
      newSlot = midpoints.length;
    } else {
      for (let i = 0; i < midpoints.length; i++) {
        if (effectiveX > midpoints[i]) {
          newSlot++;
        }
      }
    }
  }

  if (placeholderIndex.value !== newSlot) {
    placeholderIndex.value = newSlot;
  }
};

const onTabPointerUp = () => {
  window.removeEventListener('pointermove', onTabPointerMove);
  window.removeEventListener('pointerup', onTabPointerUp);
  window.removeEventListener('pointercancel', onTabPointerUp);

  if (placeholderTimer) {
    clearTimeout(placeholderTimer);
    placeholderTimer = null;
  }

  if (!dragState.value) return;

  const { fromPane, hoverPane, tabId, isDragging } = dragState.value;
  const toIndex = placeholderIndex.value;

  if (isDragging) {
    showPlaceholder.value = false;
    dragState.value.isDragging = false;
    isDropping.value = true;

    if (dropTimer) clearTimeout(dropTimer);
    dropTimer = setTimeout(() => {
      if (hoverPane === 'split-right') {
        setSplitMode('horizontal');
        moveTabToOppositePane(tabId);
      } else if (hoverPane === 'split-bottom') {
        setSplitMode('vertical');
        moveTabToOppositePane(tabId);
      } else if (hoverPane === 'paneA' || hoverPane === 'paneB') {
        transferTab(fromPane, hoverPane, tabId, toIndex ?? undefined);
      }
      dragState.value = null;
      placeholderIndex.value = null;
      isDropping.value = false;
    }, 45);
  } else {
    // Click đơn giản không kéo: chọn tab ở pane nguồn
    selectTab(fromPane, tabId);
    dragState.value = null;
    placeholderIndex.value = null;
  }
};

// ----------------------------------------------------
// Mở tab terminal mới và chạy lệnh
// ----------------------------------------------------
const openEmptyTerminal = async (targetPane?: ActivePane) => {
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
    setManualMeta(terminal.commandId, {
      name: 'Terminal',
      shellKind: terminal.shellKind,
      historyLevel: terminal.historyLevel,
    });
    splitAddTab(newTab, targetPane);
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
  shellKind?: string,
  targetPane?: ActivePane,
) => {
  const manualMeta = manualTerminalMeta.get(commandId);
  const isManual = Boolean(
    manualMeta ||
    commandId <= 0 ||
    runEventId?.startsWith('terminal:') ||
    (shellKind && shellKind !== 'command')
  );

  const finalName = manualMeta?.name || ipcClient.getTerminalName?.(commandId) || commandName || 'Terminal';
  const finalShellKind = manualMeta?.shellKind || shellKind || (isManual ? 'powershell' : 'command');
  const finalHistoryLevel = manualMeta?.historyLevel ?? (isManual ? 2 : 0);

  if (isManual) {
    setManualMeta(commandId, {
      name: finalName,
      shellKind: finalShellKind,
      historyLevel: finalHistoryLevel,
    });
  }

  const newTab: OpenTabItem = {
    id: isManual ? `terminal-${commandId}-${Date.now()}` : `cmd-${commandId}-${Date.now()}`,
    commandId,
    name: finalName,
    runEventId,
    shellKind: finalShellKind,
    historyLevel: finalHistoryLevel,
    isManual,
  };

  splitAddTab(newTab, targetPane);
};

const handleStopProcess = (commandId: number) => {
  emit('request-stop-process', commandId);
};

const restartingCmdId = ref<number | null>(null);

const handleRestartProcess = (commandId: number) => {
  void restartProcess(commandId);
};

const restartProcess = async (commandId: number) => {
  restartingCmdId.value = commandId;
  await nextTick();
  await new Promise((resolve) => requestAnimationFrame(resolve));
  try {
    if (commandId <= 0) {
      await openEmptyTerminal(activePane.value);
      splitCloseTab(activePane.value, activeTabIdA.value);
      return;
    }
    try {
      await ipcClient.stopProcess(commandId, false);
    } catch {
      // Completed process
    }
    const result = await ipcClient.runCommand(commandId);
    const tab = allOpenTabs.value.find(item => item.commandId === commandId);
    if (tab) tab.runEventId = result.runEventId;
    await refreshProcesses();
  } catch (error) {
    console.error('[DockHost] Không thể khởi động lại lệnh:', error);
  } finally {
    restartingCmdId.value = null;
  }
};

// ----------------------------------------------------
// Phím tắt toàn cục cho Split View Terminal Mode
// ----------------------------------------------------
const handleGlobalKeydown = (e: KeyboardEvent) => {
  const tag = (e.target as HTMLElement | null)?.tagName;
  if (tag === 'INPUT' || tag === 'TEXTAREA') return;

  // Ctrl+\ hoặc Cmd+\ : Bật/Tắt Split Horizontal
  if ((e.ctrlKey || e.metaKey) && e.key === '\\' && !e.shiftKey && !e.altKey) {
    e.preventDefault();
    toggleSplitHorizontal();
    return;
  }

  // Ctrl+Shift+\ : Đổi hướng Dọc ⇄ Ngang
  if ((e.ctrlKey || e.metaKey) && e.key === '\\' && e.shiftKey && !e.altKey) {
    e.preventDefault();
    toggleSplitDirection();
    return;
  }

  // Ctrl+Alt+ArrowLeft / ArrowUp : Focus Pane A
  if (e.ctrlKey && e.altKey && (e.key === 'ArrowLeft' || e.key === 'ArrowUp')) {
    e.preventDefault();
    activePane.value = 'paneA';
    return;
  }

  // Ctrl+Alt+ArrowRight / ArrowDown : Focus Pane B
  if (e.ctrlKey && e.altKey && (e.key === 'ArrowRight' || e.key === 'ArrowDown')) {
    e.preventDefault();
    if (splitMode.value !== 'single') {
      activePane.value = 'paneB';
    }
    return;
  }

  // Ctrl+Alt+M : Di chuyển tab đang active sang Pane đối diện
  if (e.ctrlKey && e.altKey && (e.key === 'm' || e.key === 'M')) {
    e.preventDefault();
    const curTabId = activePane.value === 'paneA' ? activeTabIdA.value : activeTabIdB.value;
    if (curTabId) {
      moveTabToOppositePane(curTabId);
    }
    return;
  }

  // Ctrl+Alt+R : Đặt lại tỉ lệ 50:50
  if (e.ctrlKey && e.altKey && (e.key === 'r' || e.key === 'R')) {
    e.preventDefault();
    resetSplitRatio();
    return;
  }
};

onMounted(async () => {
  window.addEventListener('click', closeContextMenu);
  window.addEventListener('keydown', handleGlobalKeydown);

  try {
    const settings = await ipcClient.getSettings();
    ghostTextEnabled.value = settings.ghostTextEnabled;
    terminalFontFamily.value = settings.fontFamily || 'JetBrains Mono';
    terminalFontSize.value = settings.fontSize || 13;
  } catch {
    ghostTextEnabled.value = true;
  }
});

onBeforeUnmount(() => {
  window.removeEventListener('click', closeContextMenu);
  window.removeEventListener('keydown', handleGlobalKeydown);
  window.removeEventListener('pointermove', onTabPointerMove);
  window.removeEventListener('pointerup', onTabPointerUp);
  window.removeEventListener('pointercancel', onTabPointerUp);

  if (placeholderTimer) clearTimeout(placeholderTimer);
  if (dropTimer) clearTimeout(dropTimer);
});

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
  transferTab,
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
   Workspace Split View Toolbar (Thanh công cụ chia màn hình)
---------------------------------------------------- */
.workspace-split-toolbar {
  height: 36px;
  background-color: #12151f;
  border-bottom: 1px solid var(--border-subtle);
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 8px;
  flex-shrink: 0;
  user-select: none;
  z-index: 10;
}

.toolbar-left-controls {
  display: flex;
  align-items: center;
  gap: 6px;
}

.split-mode-group {
  display: flex;
  align-items: center;
  background-color: rgba(26, 30, 43, 0.7);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  padding: 2px;
  gap: 2px;
}

.split-mode-btn {
  background: transparent;
  border: none;
  color: var(--text-secondary);
  font-size: 11px;
  padding: 3px 8px;
  border-radius: 2px;
  display: flex;
  align-items: center;
  gap: 5px;
  cursor: pointer;
  transition: all 0.16s ease;
  white-space: nowrap;
}

.split-mode-btn:hover {
  color: var(--text-primary);
  background-color: var(--bg-surface-hover);
}

.split-mode-btn.active {
  background-color: var(--primary, #744791);
  color: #ffffff;
  font-weight: 500;
  box-shadow: 0 0 8px var(--primary-glow);
}

.toolbar-divider {
  width: 1px;
  height: 18px;
  background-color: var(--border-subtle);
  margin: 0 4px;
}

.toolbar-action-btn {
  font-size: 11.5px;
  height: 26px;
  padding: 0 8px;
  gap: 5px;
}

.ratio-badge-btn {
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--primary-accent, #e4b5ff);
  background-color: rgba(116, 71, 145, 0.12);
  border: 1px solid rgba(116, 71, 145, 0.3);
}

.ratio-reset-icon {
  opacity: 0.8;
}

.toolbar-right-actions {
  display: flex;
  align-items: center;
  gap: 6px;
}

/* ----------------------------------------------------
   Split Workspace Body (Container chia 2 nửa)
---------------------------------------------------- */
.split-workspace-body {
  flex: 1;
  display: flex;
  width: 100%;
  height: calc(100% - 36px);
  overflow: hidden;
  position: relative;
  background-color: var(--bg-app-base);
}

.split-workspace-body.mode-single {
  flex-direction: row;
}

.split-workspace-body.mode-horizontal {
  flex-direction: row;
}

.split-workspace-body.mode-vertical {
  flex-direction: column;
}

/* ----------------------------------------------------
   Terminal Pane (Pane A & Pane B)
---------------------------------------------------- */
.terminal-pane {
  display: flex;
  flex-direction: column;
  overflow: hidden;
  position: relative;
  background-color: var(--bg-terminal);
  transition: box-shadow 0.2s ease, border-color 0.2s ease;
}

.split-workspace-body.mode-single .terminal-pane.pane-a {
  flex: 1 1 100%;
  width: 100%;
  height: 100%;
}

.split-workspace-body.mode-horizontal .terminal-pane.pane-a {
  flex: 0 0 calc(var(--split-ratio, 50%) - 3px);
  min-width: 220px;
  max-width: calc(100% - 220px);
  height: 100%;
}

.split-workspace-body.mode-horizontal .terminal-pane.pane-b {
  flex: 1 1 0;
  min-width: 220px;
  height: 100%;
}

.split-workspace-body.mode-vertical .terminal-pane.pane-a {
  flex: 0 0 calc(var(--split-ratio, 50%) - 3px);
  min-height: 150px;
  max-height: calc(100% - 150px);
  width: 100%;
}

.split-workspace-body.mode-vertical .terminal-pane.pane-b {
  flex: 1 1 0;
  min-height: 150px;
  width: 100%;
}

/* Chỉ báo Khung đang Active (Active Focus Indicator) */
.terminal-pane.is-focused {
  outline: 1.5px solid var(--primary, #744791);
  outline-offset: -1.5px;
  box-shadow: inset 0 0 12px rgba(116, 71, 145, 0.2);
}

/* Hiệu ứng Drop Target khi kéo tab qua khung */
.terminal-pane.is-drop-target {
  outline: 2px dashed var(--primary, #744791) !important;
  outline-offset: -2px;
  box-shadow: inset 0 0 24px rgba(116, 71, 145, 0.35) !important;
}

/* ----------------------------------------------------
   Pane Tab Strip Header (Thanh tab của từng khung)
---------------------------------------------------- */
.pane-tab-strip {
  height: 34px;
  background-color: #141722;
  border-bottom: 1px solid var(--border-subtle);
  display: flex;
  align-items: center;
  padding: 0 6px;
  flex-shrink: 0;
  user-select: none;
  gap: 6px;
}

.pane-indicator {
  display: flex;
  align-items: center;
  gap: 5px;
  padding: 2px 7px;
  border-radius: 3px;
  background-color: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  font-size: 10px;
  font-family: var(--font-mono);
  color: var(--text-muted);
  flex-shrink: 0;
}

.pane-indicator.active {
  color: var(--primary-accent, #e4b5ff);
  border-color: rgba(116, 71, 145, 0.4);
  background-color: rgba(116, 71, 145, 0.15);
}

.pane-dot {
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background-color: var(--text-muted);
}

.pane-indicator.active .pane-dot {
  background-color: var(--primary, #744791);
  box-shadow: 0 0 6px var(--primary);
}

.tabs-list {
  display: flex;
  align-items: center;
  gap: 4px;
  overflow-x: auto;
  flex: 1;
  position: relative;
  scrollbar-width: thin;
}

.pane-strip-actions {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-shrink: 0;
  margin-left: 4px;
}

.pane-strip-btn {
  height: 24px;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 4px;
  background: transparent;
  border-radius: var(--radius-sm);
  font-family: var(--font-mono);
  font-size: 11px;
  cursor: pointer;
  flex-shrink: 0;
  transition: all 0.15s ease;
  user-select: none;
}

.pane-strip-btn.add-btn,
.pane-add-btn {
  width: 24px;
  height: 24px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: 1px dashed var(--border-subtle);
  border-radius: var(--radius-sm);
  color: var(--text-muted);
  cursor: pointer;
  flex-shrink: 0;
  transition: all 0.15s ease;
}

.pane-strip-btn.add-btn:hover,
.pane-add-btn:hover {
  background-color: var(--bg-surface-hover);
  color: var(--text-primary);
  border-color: var(--border-medium);
}

.pane-strip-btn.kill-btn {
  padding: 0 7px;
  border: 1px solid rgba(239, 68, 68, 0.28);
  background: rgba(239, 68, 68, 0.08);
  color: #f87171;
}

.pane-strip-btn.kill-btn:hover {
  background: rgba(239, 68, 68, 0.22);
  border-color: rgba(239, 68, 68, 0.6);
  color: #fca5a5;
  box-shadow: 0 0 10px rgba(239, 68, 68, 0.25);
}

.kill-btn-label {
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 0.03em;
  text-transform: uppercase;
}

/* ----------------------------------------------------
   Tab Item Styling
---------------------------------------------------- */
.terminal-tab {
  position: relative;
  height: 28px;
  display: flex;
  align-items: center;
  gap: 7px;
  padding: 0 8px;
  background-color: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm) var(--radius-sm) 0 0;
  color: var(--text-secondary);
  font-size: 11.5px;
  cursor: grab;
  user-select: none;
  transition: background-color 0.18s cubic-bezier(0.16, 1, 0.3, 1),
              color 0.18s cubic-bezier(0.16, 1, 0.3, 1),
              border-color 0.18s cubic-bezier(0.16, 1, 0.3, 1),
              box-shadow 0.18s cubic-bezier(0.16, 1, 0.3, 1),
              opacity 0.15s ease;
  white-space: nowrap;
  overflow: hidden;
}

.terminal-tab:active {
  cursor: grabbing;
}

.floating-drag-tab {
  position: fixed;
  z-index: 99999;
  pointer-events: none;
  background-color: #1a1e2e;
  border: 1px solid var(--primary-accent);
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.7), 0 0 16px var(--primary-glow);
  opacity: 0.95;
  cursor: grabbing;
  border-radius: var(--radius-sm) var(--radius-sm) 0 0;
  color: var(--text-primary);
  will-change: left, top;
  transform: translateY(-2px);
}

.terminal-tab.is-drag-hidden {
  opacity: 0 !important;
  pointer-events: none !important;
  border-color: transparent !important;
  background-color: transparent !important;
  box-shadow: none !important;
}

.tab-placeholder {
  position: relative;
  height: 28px;
  display: flex;
  align-items: center;
  gap: 7px;
  padding: 0 10px;
  background-color: rgba(116, 71, 145, 0.2);
  border: 1.5px dashed var(--primary-accent);
  border-radius: var(--radius-sm) var(--radius-sm) 0 0;
  color: var(--primary-accent);
  font-size: 11.5px;
  user-select: none;
  white-space: nowrap;
  box-shadow: inset 0 0 12px rgba(116, 71, 145, 0.4);
  pointer-events: none;
  box-sizing: border-box;
}

.placeholder-dot {
  background-color: var(--primary-accent);
  box-shadow: 0 0 6px var(--primary-accent);
}

.terminal-tab > *:not(.tab-close-btn):not(.tab-move-btn) {
  pointer-events: none;
}

.terminal-tab:hover {
  background-color: var(--bg-surface-hover);
  color: var(--text-primary);
}

.terminal-tab.active {
  background-color: var(--bg-terminal);
  border-color: var(--border-subtle);
  color: var(--text-primary);
  font-weight: 500;
  box-shadow: 0 -2px 10px rgba(116, 71, 145, 0.22);
}

.tab-active-bar {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  height: 2px;
  background: linear-gradient(90deg, var(--primary), var(--primary-accent));
  opacity: 0;
  transform: scaleX(0);
  transform-origin: center;
  transition: transform 0.22s cubic-bezier(0.16, 1, 0.3, 1), opacity 0.2s ease;
  border-radius: 2px 2px 0 0;
  box-shadow: 0 0 6px var(--primary);
}

.terminal-tab.active .tab-active-bar {
  opacity: 1;
  transform: scaleX(1);
}

.tab-status-dot {
  width: 7px;
  height: 7px;
  border-radius: 9999px;
  background-color: var(--status-idle);
  transition: all 0.2s ease;
}

.tab-status-dot.active {
  background-color: var(--status-running);
  box-shadow: 0 0 5px var(--status-running);
}

.tab-title {
  max-width: 125px;
  overflow: hidden;
  text-overflow: ellipsis;
}

.tab-title.is-manual {
  cursor: pointer;
  transition: color 0.15s ease;
}

.terminal-tab:hover .tab-title.is-manual {
  color: var(--primary-accent, #e4b5ff);
}

.tab-pid {
  font-size: 9px;
  font-family: var(--font-mono);
  color: var(--text-muted);
  background: var(--bg-surface);
  padding: 1px 4px;
  border-radius: 2px;
}

.tab-move-btn {
  background: transparent;
  border: none;
  color: var(--text-muted);
  width: 15px;
  height: 15px;
  border-radius: 2px;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  opacity: 0.6;
  transition: background-color 0.15s ease, color 0.15s ease, transform 0.15s ease, opacity 0.15s ease;
}

.terminal-tab:hover .tab-move-btn {
  opacity: 1;
}

.tab-move-btn:hover {
  background-color: rgba(116, 71, 145, 0.25);
  color: var(--primary-accent, #e4b5ff);
  transform: scale(1.15);
}

.tab-close-btn {
  background: transparent;
  border: none;
  color: var(--text-muted);
  width: 15px;
  height: 15px;
  border-radius: 2px;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: background-color 0.15s ease, color 0.15s ease, transform 0.15s ease;
}

.tab-close-btn:hover {
  background-color: rgba(239, 68, 68, 0.2);
  color: #ef4444;
  transform: scale(1.1);
}

.no-tabs-msg {
  font-size: 11px;
  color: var(--text-muted);
  padding: 0 8px;
}

/* ----------------------------------------------------
   Pane Viewport
---------------------------------------------------- */
.pane-viewport {
  flex: 1;
  position: relative;
  overflow: hidden;
  background-color: var(--bg-terminal);
}

.pane-wrapper {
  width: 100%;
  height: 100%;
  animation: tabPaneFadeIn 0.18s cubic-bezier(0.16, 1, 0.3, 1) forwards;
}

@keyframes tabPaneFadeIn {
  from {
    opacity: 0.15;
    transform: translateY(2px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.empty-pane-state {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 16px;
}

.empty-pane-box {
  max-width: 320px;
  text-align: center;
  background-color: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  padding: 24px 18px;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
}

.empty-icon {
  color: var(--primary-accent, #e4b5ff);
  opacity: 0.7;
}

.empty-pane-box h5 {
  font-size: 13.5px;
  font-weight: 600;
  color: var(--text-primary);
}

.empty-pane-box p {
  font-size: 11.5px;
  color: var(--text-secondary);
  line-height: 1.5;
}

/* ----------------------------------------------------
   Resizable Sash / Splitter Bar
---------------------------------------------------- */
.resizable-sash {
  position: relative;
  background-color: #171b26;
  flex-shrink: 0;
  z-index: 20;
  transition: background-color 0.15s ease, box-shadow 0.15s ease;
  user-select: none;
}

.resizable-sash.sash-horizontal {
  width: 6px;
  height: 100%;
  cursor: col-resize;
  border-left: 1px solid var(--border-subtle);
  border-right: 1px solid var(--border-subtle);
}

.resizable-sash.sash-vertical {
  height: 6px;
  width: 100%;
  cursor: row-resize;
  border-top: 1px solid var(--border-subtle);
  border-bottom: 1px solid var(--border-subtle);
}

.resizable-sash:hover,
.split-workspace-body.is-dragging-sash .resizable-sash {
  background-color: var(--primary, #744791);
  box-shadow: 0 0 10px var(--primary-glow);
}

.sash-grip-line {
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  background-color: var(--text-muted);
  border-radius: 9999px;
  opacity: 0.6;
}

.sash-horizontal .sash-grip-line {
  width: 2px;
  height: 24px;
}

.sash-vertical .sash-grip-line {
  width: 24px;
  height: 2px;
}

.resizable-sash:hover .sash-grip-line {
  background-color: #ffffff;
  opacity: 1;
}

/* Guard toàn màn hình khi kéo Sash để chuột không bị nuốt sự kiện */
.sash-drag-guard {
  position: fixed;
  inset: 0;
  z-index: 999999;
  user-select: none;
}

/* ----------------------------------------------------
   Single View Mode Drop Zones (Chia đôi màn hình khi kéo tab)
---------------------------------------------------- */
.single-mode-split-hints {
  position: absolute;
  inset: 0;
  pointer-events: none;
  z-index: 50;
}

.split-drop-zone {
  position: absolute;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  background: rgba(15, 17, 23, 0.88);
  backdrop-filter: blur(6px);
  border: 2px dashed rgba(116, 71, 145, 0.5);
  border-radius: var(--radius-md);
  color: var(--text-secondary);
  font-size: 12px;
  transition: all 0.16s ease;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.6);
}

.split-drop-zone.right-zone {
  top: 6px;
  right: 6px;
  bottom: 6px;
  width: 240px;
}

.split-drop-zone.bottom-zone {
  left: 6px;
  right: 254px;
  bottom: 6px;
  height: 140px;
}

.split-drop-zone.is-hovered {
  border-color: var(--primary-accent, #e4b5ff);
  color: var(--primary-accent, #e4b5ff);
  background: rgba(116, 71, 145, 0.28);
  box-shadow: 0 0 24px var(--primary-glow);
  transform: scale(1.01);
}

/* ----------------------------------------------------
   Tab Context Menu (Right-Click)
---------------------------------------------------- */
.tab-context-menu {
  position: fixed;
  z-index: 100000;
  background-color: #1a1e2b;
  border: 1px solid var(--border-medium);
  border-radius: var(--radius-sm);
  padding: 4px;
  min-width: 220px;
  box-shadow: 0 10px 28px rgba(0, 0, 0, 0.6), 0 0 14px var(--primary-glow);
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.context-menu-item {
  background: transparent;
  border: none;
  color: var(--text-primary);
  font-size: 11.5px;
  padding: 6px 10px;
  border-radius: 3px;
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
  text-align: left;
  transition: background-color 0.12s ease;
}

.context-menu-item:hover {
  background-color: var(--bg-surface-hover);
}

.context-menu-item.danger {
  color: var(--status-failed, #ef4444);
}

.context-menu-item.danger:hover {
  background-color: rgba(239, 68, 68, 0.15);
}

.context-menu-divider {
  height: 1px;
  background-color: var(--border-subtle);
  margin: 2px 0;
}

.spin {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}
</style>
