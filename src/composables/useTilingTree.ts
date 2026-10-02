import type {
  TilingNode,
  TilingLeafNode,
  TilingSplitNode,
  SplitOrientation,
  LayoutPreset,
} from '@/types/tiling';

let idCounter = 1;
const genId = (prefix: string) => `${prefix}-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 6)}-${idCounter++}`;

export function createLeaf(tabId: string): TilingLeafNode {
  return {
    type: 'leaf',
    id: genId('leaf'),
    tabId,
  };
}

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
 * Builds a preset tree layout from an array of tabIds.
 * If not enough tabIds are provided, creates dummy empty leaf placeholders.
 */
export function buildPresetTree(
  preset: LayoutPreset,
  tabIds: string[],
  createFallbackTabId: () => string
): TilingNode | null {
  const getTab = (index: number): string => {
    return tabIds[index] || createFallbackTabId();
  };

  if (tabIds.length === 0 && preset === 'single') {
    return null;
  }

  switch (preset) {
    case 'single':
      return createLeaf(getTab(0));

    case '2-columns':
      return createSplit(
        'horizontal',
        createLeaf(getTab(0)),
        createLeaf(getTab(1)),
        0.5
      );

    case '2-rows':
      return createSplit(
        'vertical',
        createLeaf(getTab(0)),
        createLeaf(getTab(1)),
        0.5
      );

    case '2x2-grid': {
      const leftCol = createSplit(
        'vertical',
        createLeaf(getTab(0)),
        createLeaf(getTab(2)),
        0.5
      );
      const rightCol = createSplit(
        'vertical',
        createLeaf(getTab(1)),
        createLeaf(getTab(3)),
        0.5
      );
      return createSplit('horizontal', leftCol, rightCol, 0.5);
    }

    case '1+2-tiled': {
      // Tilix iconic layout: Main left 60%, 2 stacked right 40% (top & bottom 50/50)
      const mainLeft = createLeaf(getTab(0));
      const rightStacked = createSplit(
        'vertical',
        createLeaf(getTab(1)),
        createLeaf(getTab(2)),
        0.5
      );
      return createSplit('horizontal', mainLeft, rightStacked, 0.6);
    }

    case '3-columns': {
      const col1 = createLeaf(getTab(0));
      const col2And3 = createSplit(
        'horizontal',
        createLeaf(getTab(1)),
        createLeaf(getTab(2)),
        0.5
      );
      return createSplit('horizontal', col1, col2And3, 0.33);
    }

    default:
      return createLeaf(getTab(0));
  }
}

/**
 * Finds target leaf and splits it into a split container with the old leaf and a new leaf.
 */
export function splitLeafInTree(
  node: TilingNode,
  targetTabId: string,
  orientation: SplitOrientation,
  newTabId: string
): TilingNode {
  if (node.type === 'leaf') {
    if (node.tabId === targetTabId) {
      const newLeaf = createLeaf(newTabId);
      return createSplit(orientation, node, newLeaf, 0.5);
    }
    return node;
  }

  // Split node: recurse into children
  const newFirst = splitLeafInTree(node.firstChild, targetTabId, orientation, newTabId);
  const newSecond = splitLeafInTree(node.secondChild, targetTabId, orientation, newTabId);

  return {
    ...node,
    firstChild: newFirst,
    secondChild: newSecond,
  };
}

/**
 * Removes target leaf from tree. If a split node loses one child, the remaining child replaces the split node.
 */
export function removeLeafFromTree(
  node: TilingNode,
  targetTabId: string
): TilingNode | null {
  if (node.type === 'leaf') {
    return node.tabId === targetTabId ? null : node;
  }

  // If first child is the leaf to remove, replace current split with second child
  if (node.firstChild.type === 'leaf' && node.firstChild.tabId === targetTabId) {
    return node.secondChild;
  }

  // If second child is the leaf to remove, replace current split with first child
  if (node.secondChild.type === 'leaf' && node.secondChild.tabId === targetTabId) {
    return node.firstChild;
  }

  // Recurse into both branches
  const updatedFirst = removeLeafFromTree(node.firstChild, targetTabId);
  const updatedSecond = removeLeafFromTree(node.secondChild, targetTabId);

  if (!updatedFirst && !updatedSecond) {
    return null;
  }
  if (!updatedFirst) {
    return updatedSecond;
  }
  if (!updatedSecond) {
    return updatedFirst;
  }

  return {
    ...node,
    firstChild: updatedFirst,
    secondChild: updatedSecond,
  };
}

/**
 * Updates ratio for a specific split node.
 */
export function updateRatioInTree(
  node: TilingNode,
  splitId: string,
  newRatio: number
): TilingNode {
  if (node.type === 'leaf') {
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
 * Traverses tree and collects all leaf tab IDs in visual order.
 */
export function collectLeafTabIds(node: TilingNode | null): string[] {
  if (!node) return [];
  if (node.type === 'leaf') return [node.tabId];
  return [
    ...collectLeafTabIds(node.firstChild),
    ...collectLeafTabIds(node.secondChild),
  ];
}

/**
 * Finds a leaf by tabId.
 */
export function findLeafByTabId(node: TilingNode | null, tabId: string): TilingLeafNode | null {
  if (!node) return null;
  if (node.type === 'leaf') {
    return node.tabId === tabId ? node : null;
  }
  return findLeafByTabId(node.firstChild, tabId) || findLeafByTabId(node.secondChild, tabId);
}

/**
 * Deep clones a tiling tree.
 */
export function cloneTree(node: TilingNode | null): TilingNode | null {
  if (!node) return null;
  if (node.type === 'leaf') {
    return { ...node };
  }
  return {
    ...node,
    firstChild: cloneTree(node.firstChild)!,
    secondChild: cloneTree(node.secondChild)!,
  };
}
