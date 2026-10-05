import { ref, computed } from 'vue';

export interface TabDragItem {
  id: string;
  name: string;
  commandId: number;
  title?: string;
  cwd?: string;
}

export interface TabPointerDragState {
  tab: TabDragItem;
  fromPanelId: string;
  hoverPanelId: string | null;
  hoverSlotIndex: number | null;
  startX: number;
  startY: number;
  currentX: number;
  currentY: number;
  grabOffsetX: number;
  grabOffsetY: number;
  width: number;
  height: number;
  isDragging: boolean;
}

export type TabTransferCallback = (payload: {
  sourcePanelId: string;
  targetPanelId: string;
  tabId: string;
  targetIndex?: number;
}) => void;

export type TabSelectCallback = (panelId: string, tabId: string) => void;

// Singleton drag state shared across all panels and the main DockHost
const dragState = ref<TabPointerDragState | null>(null);
let globalTransferHandler: TabTransferCallback | null = null;
let globalSelectHandler: TabSelectCallback | null = null;

const onPointerMove = (e: PointerEvent) => {
  if (!dragState.value) return;

  const dx = e.clientX - dragState.value.startX;
  const dy = e.clientY - dragState.value.startY;

  if (!dragState.value.isDragging) {
    if (Math.hypot(dx, dy) > 4) {
      dragState.value.isDragging = true;
    } else {
      return;
    }
  }

  dragState.value.currentX = e.clientX;
  dragState.value.currentY = e.clientY;

  // Find panel underneath the cursor
  const elements = document.elementsFromPoint(e.clientX, e.clientY);
  let targetPanelEl: HTMLElement | null = null;

  for (const el of elements) {
    // Ignore the floating drag tab and drag guard themselves
    if (el.classList.contains('floating-drag-tab') || el.classList.contains('tab-drag-guard')) {
      continue;
    }
    const match = el.closest<HTMLElement>('[data-tiling-panel-id]');
    if (match) {
      targetPanelEl = match;
      break;
    }
  }

  if (targetPanelEl && targetPanelEl.dataset.tilingPanelId) {
    const targetPanelId = targetPanelEl.dataset.tilingPanelId;
    dragState.value.hoverPanelId = targetPanelId;

    const tabsListEl = targetPanelEl.querySelector('.panel-tabs-list') as HTMLElement | null;
    if (tabsListEl) {
      const tabElements = Array.from(
        tabsListEl.querySelectorAll<HTMLElement>('.panel-tab-item:not(.tab-placeholder)')
      ).filter(el => el.dataset.tabId !== dragState.value?.tab.id);

      const midpoints = tabElements.map(el => {
        const r = el.getBoundingClientRect();
        return r.left + r.width / 2;
      });

      let slot = 0;
      for (let i = 0; i < midpoints.length; i++) {
        if (e.clientX > midpoints[i]) {
          slot = i + 1;
        }
      }
      dragState.value.hoverSlotIndex = slot;
    } else {
      dragState.value.hoverSlotIndex = null;
    }
  } else {
    dragState.value.hoverPanelId = null;
    dragState.value.hoverSlotIndex = null;
  }
};

const onPointerUp = (_e: PointerEvent) => {
  window.removeEventListener('pointermove', onPointerMove);
  window.removeEventListener('pointerup', onPointerUp);
  window.removeEventListener('pointercancel', onPointerUp);

  if (!dragState.value) return;

  const { isDragging, fromPanelId, hoverPanelId, tab, hoverSlotIndex } = dragState.value;
  dragState.value = null;

  if (isDragging) {
    if (hoverPanelId && globalTransferHandler) {
      globalTransferHandler({
        sourcePanelId: fromPanelId,
        targetPanelId: hoverPanelId,
        tabId: tab.id,
        targetIndex: hoverSlotIndex ?? undefined,
      });
    }
  } else {
    // Was a quick click without drag threshold
    if (globalSelectHandler) {
      globalSelectHandler(fromPanelId, tab.id);
    }
  }
};

export function useTabDrag() {
  const isDragging = computed(() => dragState.value?.isDragging ?? false);

  const startTabDrag = (
    e: PointerEvent,
    tab: TabDragItem,
    fromPanelId: string
  ) => {
    // Only respond to main/left mouse button
    if (e.button !== 0) return;
    e.preventDefault();

    const targetEl = (e.currentTarget as HTMLElement).closest('.panel-tab-item') as HTMLElement | null;
    if (!targetEl) return;

    const rect = targetEl.getBoundingClientRect();

    dragState.value = {
      tab,
      fromPanelId,
      hoverPanelId: fromPanelId,
      hoverSlotIndex: null,
      startX: e.clientX,
      startY: e.clientY,
      currentX: e.clientX,
      currentY: e.clientY,
      grabOffsetX: e.clientX - rect.left,
      grabOffsetY: e.clientY - rect.top,
      width: rect.width,
      height: rect.height,
      isDragging: false,
    };

    window.addEventListener('pointermove', onPointerMove, { passive: false });
    window.addEventListener('pointerup', onPointerUp);
    window.addEventListener('pointercancel', onPointerUp);
  };

  const registerTransferHandler = (fn: TabTransferCallback) => {
    globalTransferHandler = fn;
  };

  const registerSelectHandler = (fn: TabSelectCallback) => {
    globalSelectHandler = fn;
  };

  return {
    dragState,
    isDragging,
    startTabDrag,
    registerTransferHandler,
    registerSelectHandler,
  };
}
