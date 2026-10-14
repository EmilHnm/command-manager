import { ref, computed, watch } from 'vue';
import type {
  KeyScope,
  KeybindingOverride,
  ResolvedKeybinding,
  ParsedKeybindings,
} from '@/types/keybindings';
import { COMMAND_REGISTRY, COMMAND_MAP } from '@/keybindings/registry';
import { normalizeKey } from '@/keybindings/normalize';
import { updateEffectiveBindings } from '@/keybindings/dispatcher';
import { ipcClient } from '@/ipc/client';

const overrides = ref<KeybindingOverride[]>([]);
const isLoaded = ref(false);
const isLoading = ref(false);
const error = ref<string | null>(null);

export function useKeybindings() {
  const fetchKeybindings = async () => {
    isLoading.value = true;
    error.value = null;
    try {
      const data = await ipcClient.getKeybindings();
      overrides.value = data || [];
      isLoaded.value = true;
    } catch (err: any) {
      console.error('[Keybindings] Fetch failed:', err);
      error.value = err?.message || 'Không thể tải danh sách phím tắt';
    } finally {
      isLoading.value = false;
    }
  };

  const effectiveBindings = computed<ResolvedKeybinding[]>(() => {
    const list: ResolvedKeybinding[] = [];

    // 1. Initialize from default commands
    for (const cmd of COMMAND_REGISTRY) {
      for (const d of cmd.defaults) {
        list.push({
          commandId: cmd.id,
          title: cmd.title,
          category: cmd.category,
          categoryTitle: cmd.categoryTitle,
          key: d.key,
          when: d.when,
          scopes: cmd.scopes,
          source: cmd.locked ? 'fixed' : 'default',
          isOverride: false,
          isRemoved: false,
          locked: cmd.locked,
          skipInEditable: cmd.skipInEditable,
        });
      }
    }

    // 2. Apply overrides
    for (const ov of overrides.value) {
      const isRemoval = ov.command.startsWith('-');
      const targetId = isRemoval ? ov.command.slice(1) : ov.command;
      const cmd = COMMAND_MAP.get(targetId);
      if (!cmd) continue;

      if (isRemoval) {
        // Mark matching default as removed
        const target = list.find(
          (b) =>
            b.commandId === targetId &&
            b.key.toLowerCase() === ov.key.toLowerCase() &&
            (!ov.when || b.when === ov.when) &&
            !b.isRemoved
        );
        if (target) {
          target.isRemoved = true;
          target.source = 'removed';
          target.isOverride = true;
        }
      } else {
        // Custom user binding
        const targetScope = (ov.when as KeyScope) || cmd.defaults[0]?.when || 'global';
        list.push({
          commandId: targetId,
          title: cmd.title,
          category: cmd.category,
          categoryTitle: cmd.categoryTitle,
          key: ov.key,
          when: targetScope,
          scopes: cmd.scopes,
          source: 'user',
          isOverride: true,
          isRemoved: false,
          locked: false,
          skipInEditable: cmd.skipInEditable,
        });
      }
    }

    // 3. Conflict Detection (§3.6.5)
    // Two active bindings with identical key and overlapping scopes (same scope, or one is global)
    for (let i = 0; i < list.length; i++) {
      const a = list[i];
      if (a.isRemoved) continue;

      const conflicts: string[] = [];
      for (let j = 0; j < list.length; j++) {
        if (i === j) continue;
        const b = list[j];
        if (b.isRemoved) continue;

        if (a.key.toLowerCase() === b.key.toLowerCase()) {
          const overlap =
            a.when === b.when ||
            a.when === 'global' ||
            b.when === 'global' ||
            (a.when === 'workspace' && b.when === 'terminal') ||
            (a.when === 'terminal' && b.when === 'workspace');

          if (overlap && a.commandId !== b.commandId) {
            conflicts.push(`${b.title} (${b.when})`);
          }
        }
      }

      if (conflicts.length > 0) {
        a.hasConflict = true;
        a.conflictWith = conflicts;
      } else {
        a.hasConflict = false;
        a.conflictWith = undefined;
      }
    }

    return list;
  });

  // Keep dispatcher's active bindings in sync
  watch(
    effectiveBindings,
    (bindings) => {
      updateEffectiveBindings(bindings);
    },
    { immediate: true }
  );

  const conflictCount = computed(() => {
    const seen = new Set<string>();
    for (const b of effectiveBindings.value) {
      if (b.hasConflict && !b.isRemoved) {
        seen.add(b.key.toLowerCase());
      }
    }
    return seen.size;
  });

  const customizedCount = computed(() => {
    return overrides.value.length;
  });

  const saveOverrides = async (newOverrides: KeybindingOverride[]): Promise<boolean> => {
    isLoading.value = true;
    error.value = null;
    const backup = [...overrides.value];
    try {
      const normalized = await ipcClient.setKeybindings(newOverrides);
      overrides.value = normalized;
      return true;
    } catch (err: any) {
      console.error('[Keybindings] Save failed:', err);
      overrides.value = backup;
      error.value = err?.message || 'Lưu phím tắt thất bại';
      throw err;
    } finally {
      isLoading.value = false;
    }
  };

  const setBinding = async (
    commandId: string,
    newKey: string,
    when: KeyScope,
    oldKey?: string,
    oldWhen?: KeyScope
  ): Promise<boolean> => {
    const normalizedNewKey = normalizeKey(newKey);
    let nextList = [...overrides.value];

    // If there was an old binding
    if (oldKey) {
      const normalizedOldKey = normalizeKey(oldKey);
      // Remove any existing user override for this command + oldKey
      nextList = nextList.filter(
        (o) =>
          !(
            o.command === commandId &&
            o.key.toLowerCase() === normalizedOldKey.toLowerCase()
          )
      );

      // Check if oldKey was a default binding. If so, add removal entry `"-commandId"`
      const cmd = COMMAND_MAP.get(commandId);
      const wasDefault = cmd?.defaults.some(
        (d) =>
          d.key.toLowerCase() === normalizedOldKey.toLowerCase() &&
          (!oldWhen || d.when === oldWhen)
      );

      if (wasDefault) {
        const hasRemoval = nextList.some(
          (o) =>
            o.command === `-${commandId}` &&
            o.key.toLowerCase() === normalizedOldKey.toLowerCase()
        );
        if (!hasRemoval) {
          nextList.push({
            command: `-${commandId}`,
            key: normalizedOldKey,
            when: oldWhen,
          });
        }
      }
    }

    // If newKey was previously marked as removed for this command, remove the removal entry
    nextList = nextList.filter(
      (o) =>
        !(
          o.command === `-${commandId}` &&
          o.key.toLowerCase() === normalizedNewKey.toLowerCase()
        )
    );

    // Add new user override
    nextList.push({
      command: commandId,
      key: normalizedNewKey,
      when,
    });

    return saveOverrides(nextList);
  };

  const removeBinding = async (
    commandId: string,
    key: string,
    when?: KeyScope
  ): Promise<boolean> => {
    const normalized = normalizeKey(key);
    let nextList = [...overrides.value];

    // If this was a user override, remove it
    const wasUserOverride = nextList.some(
      (o) =>
        o.command === commandId && o.key.toLowerCase() === normalized.toLowerCase()
    );

    if (wasUserOverride) {
      nextList = nextList.filter(
        (o) =>
          !(
            o.command === commandId &&
            o.key.toLowerCase() === normalized.toLowerCase()
          )
      );
    } else {
      // It was a default binding: add removal entry
      const hasRemoval = nextList.some(
        (o) =>
          o.command === `-${commandId}` &&
          o.key.toLowerCase() === normalized.toLowerCase()
      );
      if (!hasRemoval) {
        nextList.push({
          command: `-${commandId}`,
          key: normalized,
          when,
        });
      }
    }

    return saveOverrides(nextList);
  };

  const resetBinding = async (commandId: string, key?: string): Promise<boolean> => {
    let nextList = [...overrides.value];
    if (key) {
      const normalized = normalizeKey(key);
      nextList = nextList.filter(
        (o) =>
          !(
            (o.command === commandId || o.command === `-${commandId}`) &&
            o.key.toLowerCase() === normalized.toLowerCase()
          )
      );
    } else {
      nextList = nextList.filter(
        (o) => o.command !== commandId && o.command !== `-${commandId}`
      );
    }
    return saveOverrides(nextList);
  };

  const resetAll = async (confirmed: boolean): Promise<boolean> => {
    if (!confirmed) return false;
    isLoading.value = true;
    error.value = null;
    try {
      await ipcClient.resetKeybindings(true);
      overrides.value = [];
      return true;
    } catch (err: any) {
      console.error('[Keybindings] Reset all failed:', err);
      error.value = err?.message || 'Khôi phục mặc định thất bại';
      throw err;
    } finally {
      isLoading.value = false;
    }
  };

  const parseJson = async (json: string): Promise<ParsedKeybindings> => {
    return ipcClient.parseKeybindings(json);
  };

  const exportJson = async (dest?: string): Promise<string> => {
    return ipcClient.exportKeybindings(dest);
  };

  const importOverrides = async (newEntries: KeybindingOverride[]): Promise<boolean> => {
    return saveOverrides(newEntries);
  };

  return {
    overrides,
    effectiveBindings,
    conflictCount,
    customizedCount,
    isLoaded,
    isLoading,
    error,
    fetchKeybindings,
    setBinding,
    removeBinding,
    resetBinding,
    resetAll,
    parseJson,
    exportJson,
    importOverrides,
  };
}
