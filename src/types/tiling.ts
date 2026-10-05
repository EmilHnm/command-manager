export type SplitOrientation = 'horizontal' | 'vertical';

export type SyncInputMode = 'off' | 'session';

export type LayoutPreset =
  | 'single'
  | '2-columns'
  | '2-rows'
  | '2x2-grid'
  | '1+2-tiled'
  | '3-columns';

/**
 * A Panel node in the tiling tree.
 * A single panel can contain multiple tabs, each tab hosting 1 terminal.
 */
export interface TilingPanelNode {
  type: 'panel';
  id: string; // unique panel id
  tabIds: string[]; // references OpenTabItem.id
  activeTabId: string; // active OpenTabItem.id in this panel
}

/**
 * Alias for backwards compatibility if needed
 */
export type TilingLeafNode = TilingPanelNode;

export interface TilingSplitNode {
  type: 'split';
  id: string; // unique split node id
  orientation: SplitOrientation; // 'horizontal' = split side-by-side (vertical divider); 'vertical' = split stacked (horizontal divider)
  ratio: number; // percentage 0.15 - 0.85 (default 0.5)
  firstChild: TilingNode;
  secondChild: TilingNode;
}

export type TilingNode = TilingPanelNode | TilingSplitNode;

/**
 * Top-level Workspace Window representation.
 * Allows having multiple independent windows inside the workspace.
 */
export interface WorkspaceWindowItem {
  id: string;
  name: string;
  tilingRoot: TilingNode | null;
  activePanelId: string;
  zoomedPanelId: string | null;
  layoutPreset: LayoutPreset;
}

export interface WorkspaceSessionItem {
  id: string;
  name: string;
  preset: LayoutPreset;
  panelCount: number;
  runningCount: number;
  ramUsage?: string;
  tags?: string[];
  isActive?: boolean;
}
