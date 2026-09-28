import { ref, computed } from 'vue';

export type SplitMode = 'single' | 'horizontal' | 'vertical';
export type ActivePane = 'paneA' | 'paneB';

export interface OpenTabItem {
  id: string; // e.g. `cmd-1-timestamp`
  commandId: number;
  name: string;
  runEventId?: string;
  shellKind?: string;
  historyLevel?: number;
  isManual?: boolean;
}

const SPLIT_MODE_KEY = 'cm_split_mode_v1';
const SPLIT_RATIO_KEY = 'cm_split_ratio_v1';

export function useSplitLayout(initialTabs: OpenTabItem[] = []) {
  // Saved split preferences
  const savedMode = (typeof window !== 'undefined' ? localStorage.getItem(SPLIT_MODE_KEY) : null) as SplitMode | null;
  const savedRatio = typeof window !== 'undefined' ? Number(localStorage.getItem(SPLIT_RATIO_KEY)) : 50;

  const splitMode = ref<SplitMode>(savedMode === 'horizontal' || savedMode === 'vertical' ? savedMode : 'single');
  const splitRatio = ref<number>(!isNaN(savedRatio) && savedRatio >= 20 && savedRatio <= 80 ? savedRatio : 50);
  const activePane = ref<ActivePane>('paneA');

  const paneATabs = ref<OpenTabItem[]>([...initialTabs]);
  const paneBTabs = ref<OpenTabItem[]>([]);

  const activeTabIdA = ref<string>(paneATabs.value[0]?.id || '');
  const activeTabIdB = ref<string>('');

  // Save changes to localStorage
  const persistPreferences = () => {
    if (typeof window === 'undefined') return;
    try {
      localStorage.setItem(SPLIT_MODE_KEY, splitMode.value);
      localStorage.setItem(SPLIT_RATIO_KEY, String(splitRatio.value));
    } catch {}
  };

  const allOpenTabs = computed<OpenTabItem[]>(() => {
    return [...paneATabs.value, ...paneBTabs.value];
  });

  const allOpenTabCommandIds = computed<number[]>(() => {
    return allOpenTabs.value.map(t => t.commandId);
  });

  const activeTabA = computed<OpenTabItem | undefined>(() => {
    return paneATabs.value.find(t => t.id === activeTabIdA.value) || paneATabs.value[0];
  });

  const activeTabB = computed<OpenTabItem | undefined>(() => {
    return paneBTabs.value.find(t => t.id === activeTabIdB.value) || paneBTabs.value[0];
  });

  // Chuyển đổi chế độ Split View
  const setSplitMode = (mode: SplitMode) => {
    if (splitMode.value === mode) return;

    if (mode === 'single') {
      // Gộp toàn bộ tabs từ Pane B sang Pane A
      if (paneBTabs.value.length > 0) {
        paneATabs.value.push(...paneBTabs.value);
        paneBTabs.value = [];
      }
      activePane.value = 'paneA';
      if (!activeTabIdA.value && paneATabs.value.length > 0) {
        activeTabIdA.value = paneATabs.value[0].id;
      }
    } else {
      // Chuyển sang split horizontal hoặc vertical
      // Nếu Pane B đang rỗng và Pane A có từ 2 tab trở lên, chuyển tab hiện tại sang Pane B
      if (paneBTabs.value.length === 0 && paneATabs.value.length > 1) {
        const curTabId = activeTabIdA.value;
        const idx = paneATabs.value.findIndex(t => t.id === curTabId);
        const tabToMove = idx !== -1 ? paneATabs.value.splice(idx, 1)[0] : paneATabs.value.pop();
        if (tabToMove) {
          paneBTabs.value.push(tabToMove);
          activeTabIdB.value = tabToMove.id;
        }
        if (paneATabs.value.length > 0) {
          activeTabIdA.value = paneATabs.value[0].id;
        }
      }
    }

    splitMode.value = mode;
    persistPreferences();
  };

  const toggleSplitHorizontal = () => {
    if (splitMode.value === 'horizontal') {
      setSplitMode('single');
    } else {
      setSplitMode('horizontal');
    }
  };

  const toggleSplitDirection = () => {
    if (splitMode.value === 'horizontal') {
      setSplitMode('vertical');
    } else if (splitMode.value === 'vertical') {
      setSplitMode('horizontal');
    } else {
      setSplitMode('horizontal');
    }
  };

  const resetSplitRatio = () => {
    splitRatio.value = 50;
    persistPreferences();
  };

  const setSplitRatio = (ratio: number) => {
    splitRatio.value = Math.max(20, Math.min(80, ratio));
    persistPreferences();
  };

  const swapPanes = () => {
    const tempTabs = [...paneATabs.value];
    paneATabs.value = [...paneBTabs.value];
    paneBTabs.value = tempTabs;

    const tempActiveId = activeTabIdA.value;
    activeTabIdA.value = activeTabIdB.value;
    activeTabIdB.value = tempActiveId;

    activePane.value = activePane.value === 'paneA' ? 'paneB' : 'paneA';
  };

  const selectTab = (pane: ActivePane, tabId: string) => {
    activePane.value = pane;
    if (pane === 'paneA') {
      activeTabIdA.value = tabId;
    } else {
      activeTabIdB.value = tabId;
    }
  };

  // Di chuyển tab sang Pane đối diện
  const moveTabToOppositePane = (tabId: string) => {
    const inA = paneATabs.value.findIndex(t => t.id === tabId);
    if (inA !== -1) {
      // Tab đang ở Pane A -> chuyển sang Pane B
      const [tab] = paneATabs.value.splice(inA, 1);
      if (splitMode.value === 'single') {
        splitMode.value = 'horizontal';
      }
      paneBTabs.value.push(tab);
      activeTabIdB.value = tab.id;
      activePane.value = 'paneB';

      // Cập nhật active tab của Pane A
      if (activeTabIdA.value === tabId) {
        activeTabIdA.value = paneATabs.value[Math.min(inA, paneATabs.value.length - 1)]?.id || '';
      }

      // Nếu Pane A rỗng sau khi chuyển và Pane B có tab, auto-collapse về Pane A
      if (paneATabs.value.length === 0) {
        paneATabs.value = [...paneBTabs.value];
        paneBTabs.value = [];
        activeTabIdA.value = activeTabIdB.value;
        splitMode.value = 'single';
        activePane.value = 'paneA';
      }
      persistPreferences();
      return;
    }

    const inB = paneBTabs.value.findIndex(t => t.id === tabId);
    if (inB !== -1) {
      // Tab đang ở Pane B -> chuyển sang Pane A
      const [tab] = paneBTabs.value.splice(inB, 1);
      paneATabs.value.push(tab);
      activeTabIdA.value = tab.id;
      activePane.value = 'paneA';

      // Cập nhật active tab của Pane B
      if (activeTabIdB.value === tabId) {
        activeTabIdB.value = paneBTabs.value[Math.min(inB, paneBTabs.value.length - 1)]?.id || '';
      }

      // Auto-collapse nếu Pane B hết tab
      if (paneBTabs.value.length === 0) {
        splitMode.value = 'single';
      }
      persistPreferences();
    }
  };

  // Đóng tab trong một Pane cụ thể
  const closeTab = (pane: ActivePane, tabId: string): OpenTabItem | undefined => {
    const list = pane === 'paneA' ? paneATabs : paneBTabs;
    const activeIdRef = pane === 'paneA' ? activeTabIdA : activeTabIdB;

    const idx = list.value.findIndex(t => t.id === tabId);
    if (idx === -1) return undefined;

    const [closedTab] = list.value.splice(idx, 1);

    if (activeIdRef.value === tabId) {
      activeIdRef.value = list.value[Math.min(idx, list.value.length - 1)]?.id || '';
    }

    // Auto-collapse logic:
    if (splitMode.value !== 'single') {
      if (paneBTabs.value.length === 0) {
        // Pane B hết tab -> chuyển về single
        splitMode.value = 'single';
        activePane.value = 'paneA';
      } else if (paneATabs.value.length === 0) {
        // Pane A hết tab -> chuyển toàn bộ Pane B sang Pane A và về single
        paneATabs.value = [...paneBTabs.value];
        paneBTabs.value = [];
        activeTabIdA.value = activeTabIdB.value;
        splitMode.value = 'single';
        activePane.value = 'paneA';
      }
    }

    persistPreferences();
    return closedTab;
  };

  // Đóng toàn bộ tab của một Pane cụ thể và auto-collapse nếu cần
  const closePane = (pane: ActivePane): OpenTabItem[] => {
    let closedTabs: OpenTabItem[] = [];
    if (pane === 'paneB') {
      closedTabs = [...paneBTabs.value];
      paneBTabs.value = [];
      activeTabIdB.value = '';
      if (splitMode.value !== 'single') {
        splitMode.value = 'single';
        activePane.value = 'paneA';
      }
    } else {
      // paneA
      closedTabs = [...paneATabs.value];
      if (splitMode.value !== 'single' && paneBTabs.value.length > 0) {
        paneATabs.value = [...paneBTabs.value];
        paneBTabs.value = [];
        activeTabIdA.value = activeTabIdB.value;
        activeTabIdB.value = '';
        splitMode.value = 'single';
        activePane.value = 'paneA';
      } else {
        paneATabs.value = [];
        activeTabIdA.value = '';
        splitMode.value = 'single';
        activePane.value = 'paneA';
      }
    }
    persistPreferences();
    return closedTabs;
  };

  const closeAllTabs = () => {
    paneATabs.value = [];
    paneBTabs.value = [];
    activeTabIdA.value = '';
    activeTabIdB.value = '';
    splitMode.value = 'single';
    activePane.value = 'paneA';
    persistPreferences();
  };

  // Mở thêm một tab mới
  const addTab = (tab: OpenTabItem, targetPane?: ActivePane) => {
    // Kiểm tra xem tab đã tồn tại ở Pane nào chưa
    const existingA = paneATabs.value.find(t => t.commandId === tab.commandId);
    if (existingA) {
      activePane.value = 'paneA';
      activeTabIdA.value = existingA.id;
      if (tab.runEventId) existingA.runEventId = tab.runEventId;
      return existingA;
    }

    const existingB = paneBTabs.value.find(t => t.commandId === tab.commandId);
    if (existingB) {
      activePane.value = 'paneB';
      activeTabIdB.value = existingB.id;
      if (tab.runEventId) existingB.runEventId = tab.runEventId;
      return existingB;
    }

    // Chưa tồn tại -> mở vào targetPane hoặc activePane
    const dest = targetPane || (splitMode.value === 'single' ? 'paneA' : activePane.value);

    if (dest === 'paneB' && splitMode.value !== 'single') {
      paneBTabs.value.push(tab);
      activeTabIdB.value = tab.id;
      activePane.value = 'paneB';
    } else {
      paneATabs.value.push(tab);
      activeTabIdA.value = tab.id;
      activePane.value = 'paneA';
    }

    return tab;
  };

  // Di chuyển hoặc sắp xếp lại tab giữa 2 khung hoặc nội bộ 1 khung
  const transferTab = (
    fromPane: ActivePane,
    toPane: ActivePane,
    tabId: string,
    targetIndex?: number,
  ) => {
    if (fromPane === toPane) {
      // Sắp xếp lại trong cùng một pane
      const list = fromPane === 'paneA' ? paneATabs : paneBTabs;
      const fromIdx = list.value.findIndex(t => t.id === tabId);
      if (fromIdx !== -1 && targetIndex !== undefined && targetIndex !== fromIdx) {
        const [tab] = list.value.splice(fromIdx, 1);
        const insertAt = Math.max(0, Math.min(targetIndex, list.value.length));
        list.value.splice(insertAt, 0, tab);
        list.value = [...list.value];
      }
      return;
    }

    // Di chuyển giữa 2 pane khác nhau
    const sourceList = fromPane === 'paneA' ? paneATabs : paneBTabs;
    const destList = toPane === 'paneA' ? paneATabs : paneBTabs;
    const sourceActiveId = fromPane === 'paneA' ? activeTabIdA : activeTabIdB;
    const destActiveId = toPane === 'paneA' ? activeTabIdA : activeTabIdB;

    const fromIdx = sourceList.value.findIndex(t => t.id === tabId);
    if (fromIdx === -1) return;

    const [tab] = sourceList.value.splice(fromIdx, 1);

    if (targetIndex !== undefined && targetIndex >= 0) {
      const insertAt = Math.max(0, Math.min(targetIndex, destList.value.length));
      destList.value.splice(insertAt, 0, tab);
    } else {
      destList.value.push(tab);
    }
    destList.value = [...destList.value];

    // Cập nhật active tab cho Pane đích
    destActiveId.value = tab.id;
    activePane.value = toPane;

    // Cập nhật active tab cho Pane nguồn nếu tab vừa chuyển là active tab
    if (sourceActiveId.value === tabId) {
      sourceActiveId.value = sourceList.value[Math.min(fromIdx, sourceList.value.length - 1)]?.id || '';
    }

    // Auto-collapse nếu một pane rỗng
    if (splitMode.value !== 'single') {
      if (paneBTabs.value.length === 0) {
        splitMode.value = 'single';
        activePane.value = 'paneA';
      } else if (paneATabs.value.length === 0) {
        paneATabs.value = [...paneBTabs.value];
        paneBTabs.value = [];
        activeTabIdA.value = activeTabIdB.value;
        splitMode.value = 'single';
        activePane.value = 'paneA';
      }
    }

    persistPreferences();
  };

  return {
    splitMode,
    splitRatio,
    activePane,
    paneATabs,
    paneBTabs,
    activeTabIdA,
    activeTabIdB,
    activeTabA,
    activeTabB,
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
    closeTab,
    closePane,
    closeAllTabs,
    addTab,
  };
}
