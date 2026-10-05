import type {
  TilingNode,
  TilingPanelNode,
  TilingSplitNode,
  SplitOrientation,
  LayoutPreset,
} from '@/types/tiling';

let idCounter = 1;
const genId = (prefix: string) =>
  `${prefix}-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 6)}-${idCounter++}`;

/**
 * Creates a Panel leaf node containing 1 or more tabs.
 */
export function createPanel(
  tabIds: string[],
  activeTabId?: string,
  id?: string
): TilingPanelNode {
  return {
    type: 'panel',
    id: id || genId('panel'),
    tabIds: [...tabIds],
    activeTabId: activeTabId || tabIds[0] || '',
  };
}

// Backwards compatibility alias
export const createLeaf = (tabId: string, id?: string): TilingPanelNode =>
  createPanel([tabId], tabId, id);

/**
 * Creates a Split node containing 2 children.
 */
export function createSplit(
  orientation: SplitOrientation,
  firstChild: TilingNode,
  secondChild: TilingNode,
  ratio = 0.5
): TilingSplitNode {
  return {
    type: 'split',
    id: genId('split'),
    orientation,
    ratio: Math.max(0.15, Math.min(0.85, ratio)),
    firstChild,
    secondChild,
  };
}

/**
 * Builds a preset tree layout from an array of panel tab lists.
 */
export function buildPresetTree(
  preset: LayoutPreset,
  panelTabLists: string[][],
  createFallbackTabId: () => string
): TilingNode | null {
  const getPanelTabs = (index: number): string[] => {
    if (panelTabLists[index] && panelTabLists[index].length > 0) {
      return panelTabLists[index];
    }
    return [createFallbackTabId()];
  };

  if (panelTabLists.length === 0 && preset === 'single') {
    return null;
  }

  switch (preset) {
    case 'single':
      return createPanel(getPanelTabs(0));

    case '2-columns':
      return createSplit(
        'horizontal',
        createPanel(getPanelTabs(0)),
        createPanel(getPanelTabs(1)),
        0.5
      );

    case '2-rows':
      return createSplit(
        'vertical',
        createPanel(getPanelTabs(0)),
        createPanel(getPanelTabs(1)),
        0.5
      );

    case '2x2-grid': {
      const leftCol = createSplit(
        'vertical',
        createPanel(getPanelTabs(0)),
        createPanel(getPanelTabs(2)),
        0.5
      );
      const rightCol = createSplit(
        'vertical',
        createPanel(getPanelTabs(1)),
        createPanel(getPanelTabs(3)),
        0.5
      );
      return createSplit('horizontal', leftCol, rightCol, 0.5);
    }

    case '1+2-tiled': {
      const mainLeft = createPanel(getPanelTabs(0));
      const rightStacked = createSplit(
        'vertical',
        createPanel(getPanelTabs(1)),
        createPanel(getPanelTabs(2)),
        0.5
      );
      return createSplit('horizontal', mainLeft, rightStacked, 0.6);
    }

    case '3-columns': {
      const col1 = createPanel(getPanelTabs(0));
      const col2And3 = createSplit(
        'horizontal',
        createPanel(getPanelTabs(1)),
        createPanel(getPanelTabs(2)),
        0.5
      );
      return createSplit('horizontal', col1, col2And3, 0.33);
    }

    default:
      return createPanel(getPanelTabs(0));
  }
}

/**
 * Splits target panel in the tree into a split container with the old panel and a new panel.
 */
export function splitPanelInTree(
  node: TilingNode,
  targetPanelId: string,
  orientation: SplitOrientation,
  newPanel: TilingPanelNode
): TilingNode {
  if (node.type === 'panel') {
    if (node.id === targetPanelId) {
      return createSplit(orientation, node, newPanel, 0.5);
    }
    return node;
  }

  return {
    ...node,
    firstChild: splitPanelInTree(node.firstChild, targetPanelId, orientation, newPanel),
    secondChild: splitPanelInTree(node.secondChild, targetPanelId, orientation, newPanel),
  };
}

/**
 * Removes target panel from tree. If a split node loses one child, the remaining child replaces the split node.
 */
export function removePanelFromTree(
  node: TilingNode,
  targetPanelId: string
): TilingNode | null {
  if (node.type === 'panel') {
    return node.id === targetPanelId ? null : node;
  }

  // If first child is the panel to remove, replace current split with second child
  if (node.firstChild.type === 'panel' && node.firstChild.id === targetPanelId) {
    return node.secondChild;
  }

  // If second child is the panel to remove, replace current split with first child
  if (node.secondChild.type === 'panel' && node.secondChild.id === targetPanelId) {
    return node.firstChild;
  }

  const updatedFirst = removePanelFromTree(node.firstChild, targetPanelId);
  const updatedSecond = removePanelFromTree(node.secondChild, targetPanelId);

  if (!updatedFirst && !updatedSecond) return null;
  if (!updatedFirst) return updatedSecond;
  if (!updatedSecond) return updatedFirst;

  return {
    ...node,
    firstChild: updatedFirst,
    secondChild: updatedSecond,
  };
}

/**
 * Adds a new tab ID to a specific panel and makes it active.
 */
export function addTabToPanel(
  node: TilingNode,
  targetPanelId: string,
  newTabId: string
): TilingNode {
  if (node.type === 'panel') {
    if (node.id === targetPanelId) {
      return {
        ...node,
        tabIds: [...node.tabIds, newTabId],
        activeTabId: newTabId,
      };
    }
    return node;
  }

  return {
    ...node,
    firstChild: addTabToPanel(node.firstChild, targetPanelId, newTabId),
    secondChild: addTabToPanel(node.secondChild, targetPanelId, newTabId),
  };
}

/**
 * Removes a tab ID from whichever panel contains it.
 * If that panel has no tabs left, removes the panel entirely.
 */
export function removeTabFromPanel(
  node: TilingNode,
  targetTabId: string
): { newTree: TilingNode | null; removedPanelId?: string } {
  const panel = findPanelByTabId(node, targetTabId);
  if (!panel) {
    return { newTree: node };
  }

  if (panel.tabIds.length <= 1) {
    // Panel becomes empty, remove the panel from tree
    return {
      newTree: removePanelFromTree(node, panel.id),
      removedPanelId: panel.id,
    };
  }

  // Panel has remaining tabs
  const newTabIds = panel.tabIds.filter(id => id !== targetTabId);
  let newActiveTabId = panel.activeTabId;
  if (panel.activeTabId === targetTabId) {
    const oldIdx = panel.tabIds.indexOf(targetTabId);
    newActiveTabId = newTabIds[Math.min(oldIdx, newTabIds.length - 1)] || newTabIds[0];
  }

  const updateTabInTree = (curr: TilingNode): TilingNode => {
    if (curr.type === 'panel') {
      if (curr.id === panel.id) {
        return {
          ...curr,
          tabIds: newTabIds,
          activeTabId: newActiveTabId,
        };
      }
      return curr;
    }
    return {
      ...curr,
      firstChild: updateTabInTree(curr.firstChild),
      secondChild: updateTabInTree(curr.secondChild),
    };
  };

  return {
    newTree: updateTabInTree(node),
  };
}

/**
 * Sets the active tab ID for a specific panel.
 */
export function setActiveTabInPanel(
  node: TilingNode,
  targetPanelId: string,
  tabId: string
): TilingNode {
  if (node.type === 'panel') {
    if (node.id === targetPanelId) {
      return {
        ...node,
        activeTabId: tabId,
      };
    }
    return node;
  }

  return {
    ...node,
    firstChild: setActiveTabInPanel(node.firstChild, targetPanelId, tabId),
    secondChild: setActiveTabInPanel(node.secondChild, targetPanelId, tabId),
  };
}

/**
 * Finds which panel contains the given tabId.
 */
export function findPanelByTabId(
  node: TilingNode | null,
  tabId: string
): TilingPanelNode | null {
  if (!node) return null;
  if (node.type === 'panel') {
    return node.tabIds.includes(tabId) ? node : null;
  }
  return findPanelByTabId(node.firstChild, tabId) || findPanelByTabId(node.secondChild, tabId);
}

/**
 * Finds a panel by its panel id.
 */
export function findPanelById(
  node: TilingNode | null,
  panelId: string
): TilingPanelNode | null {
  if (!node) return null;
  if (node.type === 'panel') {
    return node.id === panelId ? node : null;
  }
  return findPanelById(node.firstChild, panelId) || findPanelById(node.secondChild, panelId);
}

/**
 * Collects all panels from the tree into a flat array.
 */
export function collectAllPanels(node: TilingNode | null): TilingPanelNode[] {
  if (!node) return [];
  if (node.type === 'panel') {
    return [node];
  }
  return [...collectAllPanels(node.firstChild), ...collectAllPanels(node.secondChild)];
}

/**
 * Collects all tab IDs across all panels in the tree.
 */
export function collectAllTabIds(node: TilingNode | null): string[] {
  if (!node) return [];
  if (node.type === 'panel') {
    return [...node.tabIds];
  }
  return [...collectAllTabIds(node.firstChild), ...collectAllTabIds(node.secondChild)];
}

// Backwards compatibility alias
export const collectLeafTabIds = collectAllTabIds;

/**
 * Updates ratio for a specific split node.
 */
export function updateRatioInTree(
  node: TilingNode,
  splitId: string,
  newRatio: number
): TilingNode {
  if (node.type === 'panel') {
    return node;
  }

  const clampedRatio = Math.max(0.15, Math.min(0.85, newRatio));

  if (node.id === splitId) {
    return {
      ...node,
      ratio: clampedRatio,
    };
  }

  return {
    ...node,
    firstChild: updateRatioInTree(node.firstChild, splitId, clampedRatio),
    secondChild: updateRatioInTree(node.secondChild, splitId, clampedRatio),
  };
}

/**
 * Transfers a tab from a source panel to a target panel at an optional index.
 * Handles auto-collapsing of empty source panel.
 */
export function transferTabBetweenPanels(
  node: TilingNode,
  sourcePanelId: string,
  targetPanelId: string,
  tabId: string,
  targetIndex?: number
): TilingNode | null {
  // Case 1: Reordering within the same panel
  if (sourcePanelId === targetPanelId) {
    const reorderInTree = (curr: TilingNode): TilingNode => {
      if (curr.type === 'panel') {
        if (curr.id === sourcePanelId) {
          const fromIdx = curr.tabIds.indexOf(tabId);
          if (fromIdx === -1) return curr;
          const newTabIds = [...curr.tabIds];
          newTabIds.splice(fromIdx, 1);
          const insertAt = targetIndex !== undefined
            ? Math.max(0, Math.min(targetIndex, newTabIds.length))
            : newTabIds.length;
          newTabIds.splice(insertAt, 0, tabId);
          return {
            ...curr,
            tabIds: newTabIds,
            activeTabId: tabId,
          };
        }
        return curr;
      }
      return {
        ...curr,
        firstChild: reorderInTree(curr.firstChild),
        secondChild: reorderInTree(curr.secondChild),
      };
    };
    return reorderInTree(node);
  }

  // Case 2: Moving tab from sourcePanel to a different targetPanel
  const sourcePanel = findPanelById(node, sourcePanelId);
  const targetPanel = findPanelById(node, targetPanelId);
  if (!sourcePanel || !targetPanel || !sourcePanel.tabIds.includes(tabId)) {
    return node;
  }

  let intermediateTree: TilingNode | null = node;

  // Step 2a: Remove tab from source panel (or collapse source panel if it had only 1 tab)
  if (sourcePanel.tabIds.length <= 1) {
    intermediateTree = removePanelFromTree(node, sourcePanelId);
  } else {
    const newSourceTabIds = sourcePanel.tabIds.filter(id => id !== tabId);
    let newSourceActiveTabId = sourcePanel.activeTabId;
    if (sourcePanel.activeTabId === tabId) {
      const oldIdx = sourcePanel.tabIds.indexOf(tabId);
      newSourceActiveTabId = newSourceTabIds[Math.min(oldIdx, newSourceTabIds.length - 1)] || newSourceTabIds[0];
    }
    const updateSourceInTree = (curr: TilingNode): TilingNode => {
      if (curr.type === 'panel') {
        if (curr.id === sourcePanelId) {
          return {
            ...curr,
            tabIds: newSourceTabIds,
            activeTabId: newSourceActiveTabId,
          };
        }
        return curr;
      }
      return {
        ...curr,
        firstChild: updateSourceInTree(curr.firstChild),
        secondChild: updateSourceInTree(curr.secondChild),
      };
    };
    intermediateTree = updateSourceInTree(node);
  }

  if (!intermediateTree) {
    return createPanel([tabId], tabId, targetPanelId);
  }

  // Step 2b: Add tab to target panel at targetIndex
  const addTargetInTree = (curr: TilingNode): TilingNode => {
    if (curr.type === 'panel') {
      if (curr.id === targetPanelId) {
        const newTabIds = [...curr.tabIds];
        const insertAt = targetIndex !== undefined
          ? Math.max(0, Math.min(targetIndex, newTabIds.length))
          : newTabIds.length;
        newTabIds.splice(insertAt, 0, tabId);
        return {
          ...curr,
          tabIds: newTabIds,
          activeTabId: tabId,
        };
      }
      return curr;
    }
    return {
      ...curr,
      firstChild: addTargetInTree(curr.firstChild),
      secondChild: addTargetInTree(curr.secondChild),
    };
  };

  return addTargetInTree(intermediateTree);
}
