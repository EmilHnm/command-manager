export type SplitOrientation = 'horizontal' | 'vertical';

export type SyncInputMode = 'off' | 'session';

export type LayoutPreset =
  | 'single'
  | '2-columns'
  | '2-rows'
  | '2x2-grid'
  | '1+2-tiled'
  | '3-columns';

export interface TilingLeafNode {
  type: 'leaf';
  id: string; // unique node id
  tabId: string; // references OpenTabItem.id
}

export interface TilingSplitNode {
  type: 'split';
  id: string; // unique split node id
  orientation: SplitOrientation; // 'horizontal' = split side-by-side (vertical divider); 'vertical' = split stacked (horizontal divider)
  ratio: number; // percentage 0.15 - 0.85 (default 0.5)
  firstChild: TilingNode;
  secondChild: TilingNode;
}

export type TilingNode = TilingLeafNode | TilingSplitNode;

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
