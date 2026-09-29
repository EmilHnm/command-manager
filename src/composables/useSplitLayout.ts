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
  cwd?: string;
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

  const setSplitMode = (mode: SplitMode) => {
    if (splitMode.value === mode) return;

    if (mode === 'single') {
      if (paneBTabs.value.length > 0) {
        paneATabs.value.push(...paneBTabs.value);
        paneBTabs.value = [];
      }
      activePane.value = 'paneA';
      if (!activeTabIdA.value && paneATabs.value.length > 0) {
        activeTabIdA.value = paneATabs.value[0].id;
      }
    } else {
      // When splitting with an empty Pane B and Pane A has multiple tabs, move current active tab to Pane B
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

  // Split pane horizontally or vertically with a designated tab
  const splitWithTab = (mode: 'horizontal' | 'vertical', tabId: string) => {
    const inA = paneATabs.value.findIndex(t => t.id === tabId);
    if (inA !== -1) {
      if (paneATabs.value.length > 1) {
        const [tab] = paneATabs.value.splice(inA, 1);
        paneBTabs.value.push(tab);
        splitMode.value = mode;
        activeTabIdB.value = tab.id;
        activePane.value = 'paneB';

        if (activeTabIdA.value === tabId) {
          activeTabIdA.value = paneATabs.value[Math.min(inA, paneATabs.value.length - 1)]?.id || '';
        }
      } else {
        // If Pane A has only one tab, keep it in Pane A and open Pane B in ready state
        splitMode.value = mode;
        activePane.value = 'paneB';
      }

      persistPreferences();
      return;
    }

    const inB = paneBTabs.value.findIndex(t => t.id === tabId);
    if (inB !== -1) {
      splitMode.value = mode;
      activeTabIdB.value = tabId;
      activePane.value = 'paneB';
      persistPreferences();
      return;
    }
  };

  const moveTabToOppositePane = (tabId: string) => {
    const inA = paneATabs.value.findIndex(t => t.id === tabId);
    if (inA !== -1) {
      const [tab] = paneATabs.value.splice(inA, 1);
      if (splitMode.value === 'single') {
        splitMode.value = 'horizontal';
      }
      paneBTabs.value.push(tab);
      activeTabIdB.value = tab.id;
      activePane.value = 'paneB';

      if (activeTabIdA.value === tabId) {
        activeTabIdA.value = paneATabs.value[Math.min(inA, paneATabs.value.length - 1)]?.id || '';
      }

      // If Pane A is empty after moving, auto-collapse back to single pane in Pane A
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
      const [tab] = paneBTabs.value.splice(inB, 1);
      paneATabs.value.push(tab);
      activeTabIdA.value = tab.id;
      activePane.value = 'paneA';

      if (activeTabIdB.value === tabId) {
        activeTabIdB.value = paneBTabs.value[Math.min(inB, paneBTabs.value.length - 1)]?.id || '';
      }

      // Auto-collapse if Pane B is empty
      if (paneBTabs.value.length === 0) {
        splitMode.value = 'single';
      }
      persistPreferences();
    }
  };

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
        // Pane B is empty -> collapse to single pane
        splitMode.value = 'single';
        activePane.value = 'paneA';
      } else if (paneATabs.value.length === 0) {
        // Pane A is empty -> move all Pane B tabs to Pane A and collapse to single pane
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

  const addTab = (tab: OpenTabItem, targetPane?: ActivePane) => {
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

  const transferTab = (
    fromPane: ActivePane,
    toPane: ActivePane,
    tabId: string,
    targetIndex?: number,
  ) => {
    if (fromPane === toPane) {
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

    destActiveId.value = tab.id;
    activePane.value = toPane;

    if (sourceActiveId.value === tabId) {
      sourceActiveId.value = sourceList.value[Math.min(fromIdx, sourceList.value.length - 1)]?.id || '';
    }

    // Auto-collapse if either pane becomes empty
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
    splitWithTab,
    transferTab,
    closeTab,
    closePane,
    closeAllTabs,
    addTab,
  };
}
