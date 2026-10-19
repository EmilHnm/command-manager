export type KeyScope = 'global' | 'workspace' | 'explorer' | 'terminal' | 'files' | 'suggest' | 'modal';

export type KeyCategory = 'app' | 'nav' | 'workspace' | 'terminal' | 'files' | 'suggest';

export interface KeyDefaultBinding {
  key: string;
  when: KeyScope;
}

export interface KeyCommand {
  id: string;
  title: string;
  category: KeyCategory;
  categoryTitle: string;
  defaults: KeyDefaultBinding[];
  scopes?: KeyScope[];
  locked?: boolean;
  skipInEditable?: boolean;
}

export interface KeybindingOverride {
  key: string;
  command: string;
  when?: string;
}

export type KeybindingSource = 'default' | 'user' | 'removed' | 'fixed';

export interface ResolvedKeybinding {
  commandId: string;
  title: string;
  category: KeyCategory;
  categoryTitle: string;
  key: string;
  when: KeyScope;
  scopes?: KeyScope[];
  source: KeybindingSource;
  isOverride: boolean;
  isRemoved: boolean;
  locked?: boolean;
  hasConflict?: boolean;
  conflictWith?: string[];
  skipInEditable?: boolean;
}

export interface SkippedEntry {
  index: number;
  reason: string;
}

export interface ParsedKeybindings {
  entries: KeybindingOverride[];
  skipped: SkippedEntry[];
}
