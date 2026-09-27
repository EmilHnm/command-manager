<template>
  <div class="dock-host-container">
    <!-- Tab Strip Header -->
    <div class="tabs-header">
      <TransitionGroup
        ref="tabsListRef"
        tag="div"
        name="tab-anim"
        class="tabs-list"
        @wheel.passive="handleTabsWheel"
      >
        <div
          v-for="item in displayedTabList"
          :key="item.key"
          class="terminal-tab"
          :class="{
            'tab-placeholder': item.isPlaceholder && showPlaceholder,
            'is-drag-hidden': item.isPlaceholder && !showPlaceholder,
            active: !item.isPlaceholder && item.tab?.id === activeTabId,
            'is-manual': !item.isPlaceholder && isManualTab(item.tab),
          }"
          :style="item.isPlaceholder && item.width ? { width: `${item.width}px` } : undefined"
          @pointerdown="!item.isPlaceholder && item.tab ? handleTabPointerDown($event, item.tab) : undefined"
          @dblclick="!item.isPlaceholder && item.tab ? handleTabDblClick(item.tab) : undefined"
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

            <!-- Close tab button: chỉ ẩn tab, KHÔNG kill process -->
            <button
              class="tab-close-btn"
              title="Ẩn tab này (Tiến trình vẫn tiếp tục chạy ngầm)"
              @pointerdown.stop
              @click.stop="closeTab(item.tab.id)"
            >
              <X :size="12" />
            </button>
          </template>
        </div>

        <div v-if="openTabs.length === 0" key="__no_tabs__" class="no-tabs-msg">
          Chưa mở tab terminal nào
        </div>
      </TransitionGroup>

      <!-- Floating Dragged Tab (follows cursor during drag) -->
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
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount } from 'vue';
import { Plus, X, Terminal } from 'lucide-vue-next';
import XtermPane from './XtermPane.vue';
import RenameTerminalModal from '@/components/dialogs/RenameTerminalModal.vue';
import { useRunSession } from '@/composables/useRunSession';
import { ipcClient } from '@/ipc/client';

export interface OpenTabItem {
  id: string; // e.g. `cmd-1`
  commandId: number;
  name: string;
  runEventId?: string;
  shellKind?: string;
  historyLevel?: number;
  isManual?: boolean;
}

const props = defineProps<{
  initialTabs?: OpenTabItem[];
}>();

const emit = defineEmits<{
  (e: 'request-stop-process', commandId: number): void;
}>();

const { getProcessStatus, getProcessInfo, refreshProcesses, updateProcessName } = useRunSession();

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

// Quản lý metadata tên và shell của các tab terminal mở thủ công (ngay cả khi tab bị ẩn/đóng)
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

const closeTab = (tabId: string) => {
  const idx = openTabs.value.findIndex(t => t.id === tabId);
  if (idx !== -1) {
    const tabToClose = openTabs.value[idx];
    if (tabToClose && isManualTab(tabToClose)) {
      setManualMeta(tabToClose.commandId, {
        name: tabToClose.name,
        shellKind: tabToClose.shellKind,
        historyLevel: tabToClose.historyLevel,
      });
      ipcClient.setTerminalName?.(tabToClose.commandId, tabToClose.name);
      updateProcessName(tabToClose.commandId, tabToClose.name);
    }
    openTabs.value.splice(idx, 1);
    if (activeTabId.value === tabId) {
      activeTabId.value = openTabs.value[0]?.id || '';
    }
  }
};

const closeAllTabs = () => {
  openTabs.value.forEach(tab => {
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
  openTabs.value = [];
  activeTabId.value = '';
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
  const target = openTabs.value.find(t => t.id === tabId);
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

// Drag & Drop ngang cho danh sách Tab với Animation Placeholder thời gian thực (Pointer Events)
interface TabBarItem {
  key: string;
  isPlaceholder: boolean;
  tab?: OpenTabItem;
  name: string;
  width?: number;
}

interface PointerDragState {
  tabId: string;
  tabName: string;
  tabCommandId: number;
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

const tabsListRef = ref<any>(null);
const dragState = ref<PointerDragState | null>(null);
const placeholderIndex = ref<number | null>(null);
const showPlaceholder = ref(false);
const isDropping = ref(false);
let placeholderTimer: ReturnType<typeof setTimeout> | null = null;
let dropTimer: ReturnType<typeof setTimeout> | null = null;

const displayedTabList = computed<TabBarItem[]>(() => {
  if (!dragState.value?.isDragging && !isDropping.value) {
    return openTabs.value.map(t => ({
      key: t.id,
      isPlaceholder: false,
      tab: t,
      name: t.name,
    }));
  }

  const draggedTabId = dragState.value?.tabId;
  const draggedTab = openTabs.value.find(t => t.id === draggedTabId);
  if (!draggedTab || placeholderIndex.value === null || !dragState.value) {
    return openTabs.value.map(t => ({
      key: t.id,
      isPlaceholder: false,
      tab: t,
      name: t.name,
    }));
  }

  const remaining = openTabs.value.filter(t => t.id !== draggedTabId);
  const slot = Math.max(0, Math.min(placeholderIndex.value, remaining.length));

  const items: TabBarItem[] = [];
  for (let i = 0; i <= remaining.length; i++) {
    if (i === slot) {
      items.push({
        key: draggedTab.id, // Dùng chính ID của tab gốc: danh sách luôn giữ nguyên đúng số lượng tab, không bao giờ nháy lên tab thứ 4
        isPlaceholder: true,
        tab: draggedTab,
        name: dragState.value.tabName,
        width: dragState.value.width,
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
});

const getTabsListEl = (): HTMLElement | null => {
  if (tabsListRef.value) {
    if (tabsListRef.value instanceof HTMLElement) return tabsListRef.value;
    if (tabsListRef.value.$el instanceof HTMLElement) return tabsListRef.value.$el;
  }
  return document.querySelector('.tabs-list') as HTMLElement | null;
};

const checkAutoScroll = (clientX: number) => {
  const el = getTabsListEl();
  if (!el) return;
  const rect = el.getBoundingClientRect();
  const edgeThreshold = 48;
  if (clientX - rect.left < edgeThreshold && el.scrollLeft > 0) {
    el.scrollLeft -= 8;
  } else if (rect.right - clientX < edgeThreshold) {
    el.scrollLeft += 8;
  }
};

const handleTabsWheel = (e: WheelEvent) => {
  const el = getTabsListEl();
  if (!el) return;
  if (e.deltaY !== 0) {
    el.scrollLeft += e.deltaY;
  }
};

const handleTabPointerDown = (e: PointerEvent, tab: OpenTabItem) => {
  if (e.button !== 0 || isDropping.value) return;

  const targetEl = e.currentTarget as HTMLElement | null;
  if (!targetEl) return;

  if (placeholderTimer) clearTimeout(placeholderTimer);
  if (dropTimer) clearTimeout(dropTimer);
  showPlaceholder.value = false;

  const rect = targetEl.getBoundingClientRect();
  const fromIndex = openTabs.value.findIndex(t => t.id === tab.id);
  if (fromIndex === -1) return;

  const containerEl = getTabsListEl();
  const initialScroll = containerEl ? containerEl.scrollLeft : 0;

  // Lấy vị trí trung tâm tĩnh (un-transformed midpoints) của từng tab trước khi kéo
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

  window.addEventListener('pointermove', onWindowPointerMove);
  window.addEventListener('pointerup', onWindowPointerUp);
  window.addEventListener('pointercancel', onWindowPointerUp);
};

const onWindowPointerMove = (e: PointerEvent) => {
  if (!dragState.value || isDropping.value) return;

  const dx = e.clientX - dragState.value.startX;
  const dy = e.clientY - dragState.value.startY;

  if (!dragState.value.isDragging) {
    if (Math.hypot(dx, dy) > 4) {
      dragState.value.isDragging = true;
      // Ẩn tab gốc trước, placeholder chưa hiện
      showPlaceholder.value = false;
      if (placeholderTimer) clearTimeout(placeholderTimer);
      placeholderTimer = setTimeout(() => {
        // Sau một nhịp ngắn mới hiện tab placeholder
        showPlaceholder.value = true;
      }, 50);
    } else {
      return;
    }
  }

  dragState.value.currentX = e.clientX;
  dragState.value.currentY = e.clientY;
  checkAutoScroll(e.clientX);

  const containerEl = getTabsListEl();
  const currentScroll = containerEl ? containerEl.scrollLeft : dragState.value.initialScrollLeft;
  const scrollDelta = currentScroll - dragState.value.initialScrollLeft;

  // Tọa độ X thực tế sau khi tính đến cuộn ngang
  const effectiveX = e.clientX + scrollDelta;

  const fromIndex = dragState.value.initialIndex;
  const midpoints = dragState.value.tabMidpoints;

  // Tính slot mục tiêu dựa trên ngưỡng trung tâm của các tab còn lại
  // Khi chuột kéo vượt qua tâm của tab khác, placeholder lập tức trượt sang vị trí đó trong thời gian thực
  let newSlot = 0;
  for (let i = 0; i < midpoints.length; i++) {
    if (i === fromIndex) continue;
    if (effectiveX > midpoints[i]) {
      newSlot++;
    }
  }

  if (placeholderIndex.value !== newSlot) {
    placeholderIndex.value = newSlot;
  }
};

const onWindowPointerUp = () => {
  window.removeEventListener('pointermove', onWindowPointerMove);
  window.removeEventListener('pointerup', onWindowPointerUp);
  window.removeEventListener('pointercancel', onWindowPointerUp);

  if (placeholderTimer) {
    clearTimeout(placeholderTimer);
    placeholderTimer = null;
  }

  if (!dragState.value) return;

  if (dragState.value.isDragging && placeholderIndex.value !== null) {
    const fromIndex = openTabs.value.findIndex(t => t.id === dragState.value!.tabId);
    const toIndex = placeholderIndex.value;

    // Giai đoạn kết thúc: Ẩn tab placeholder trước
    showPlaceholder.value = false;
    dragState.value.isDragging = false;
    isDropping.value = true;

    // Sau khi placeholder đã ẩn, mới hiện tab thật ở vị trí đích
    if (dropTimer) clearTimeout(dropTimer);
    dropTimer = setTimeout(() => {
      if (fromIndex !== -1 && fromIndex !== toIndex) {
        const [item] = openTabs.value.splice(fromIndex, 1);
        if (item) {
          const insertAt = Math.max(0, Math.min(toIndex, openTabs.value.length));
          openTabs.value.splice(insertAt, 0, item);
          openTabs.value = [...openTabs.value];
        }
      }
      dragState.value = null;
      placeholderIndex.value = null;
      isDropping.value = false;
    }, 60);
  } else {
    // Single click without drag: select tab
    selectTab(dragState.value.tabId);
    dragState.value = null;
    placeholderIndex.value = null;
  }
};

onBeforeUnmount(() => {
  window.removeEventListener('pointermove', onWindowPointerMove);
  window.removeEventListener('pointerup', onWindowPointerUp);
  window.removeEventListener('pointercancel', onWindowPointerUp);
  if (placeholderTimer) clearTimeout(placeholderTimer);
  if (dropTimer) clearTimeout(dropTimer);
});

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
      isManual: true,
    };
    setManualMeta(terminal.commandId, {
      name: 'Terminal',
      shellKind: terminal.shellKind,
      historyLevel: terminal.historyLevel,
    });
    openTabs.value.push(newTab);
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
  shellKind?: string,
) => {
  const existing = openTabs.value.find(t => t.commandId === commandId);
  if (existing) {
    if (runEventId) existing.runEventId = runEventId;
    activeTabId.value = existing.id;
    return;
  }

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
  openTabs.value.push(newTab);
  activeTabId.value = newTab.id;
};

const handleStopProcess = (commandId: number) => {
  emit('request-stop-process', commandId);
};

const handleRestartProcess = (commandId: number) => {
  void restartProcess(commandId);
};

const restartProcess = async (commandId: number) => {
  try {
    if (commandId <= 0) {
      await openEmptyTerminal();
      const oldTab = openTabs.value.find(item => item.commandId === commandId);
      if (oldTab) {
        openTabs.value = openTabs.value.filter(item => item.id !== oldTab.id);
      }
      return;
    }
    try {
      await ipcClient.stopProcess(commandId, false);
    } catch {
      // A completed process may already have disappeared from ProcessManager.
    }
    const result = await ipcClient.runCommand(commandId);
    const tab = openTabs.value.find(item => item.commandId === commandId);
    if (tab) tab.runEventId = result.runEventId;
    await refreshProcesses();
  } catch (error) {
    console.error('[DockHost] Không thể khởi động lại lệnh:', error);
  }
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
  position: relative;
  scrollbar-width: thin;
}

.terminal-tab {
  position: relative;
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

.tab-anim-move {
  transition: transform 0.22s cubic-bezier(0.16, 1, 0.3, 1);
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
  height: 30px;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0 12px;
  background-color: rgba(116, 71, 145, 0.2);
  border: 1.5px dashed var(--primary-accent);
  border-radius: var(--radius-sm) var(--radius-sm) 0 0;
  color: var(--primary-accent);
  font-size: 12px;
  user-select: none;
  white-space: nowrap;
  box-shadow: inset 0 0 12px rgba(116, 71, 145, 0.4);
  pointer-events: none;
  box-sizing: border-box;
  animation: placeholderFadeIn 0.12s cubic-bezier(0.16, 1, 0.3, 1);
}

@keyframes placeholderFadeIn {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}

.placeholder-dot {
  background-color: var(--primary-accent);
  box-shadow: 0 0 6px var(--primary-accent);
}

.terminal-tab > *:not(.tab-close-btn) {
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

.terminal-tab.active .tab-status-dot.active {
  transform: scale(1.15);
}

.tab-title {
  max-width: 140px;
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
  transition: background-color 0.15s ease, color 0.15s ease, transform 0.15s ease;
}

.tab-close-btn:hover {
  background-color: rgba(239, 68, 68, 0.2);
  color: #ef4444;
  transform: scale(1.1);
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
  animation: tabPaneFadeIn 0.2s cubic-bezier(0.16, 1, 0.3, 1) forwards;
}

@keyframes tabPaneFadeIn {
  from {
    opacity: 0.15;
    transform: translateY(3px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
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
