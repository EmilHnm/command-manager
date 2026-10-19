import { ref, onMounted, onBeforeUnmount, computed } from 'vue';
import type { KeyScope, ResolvedKeybinding } from '@/types/keybindings';
import { COMMAND_REGISTRY } from './registry';
import { eventToChord, isImeComposing, formatChordString } from './normalize';

type CommandHandler = (event: KeyboardEvent) => void;

const SCOPE_PRIORITIES: Record<KeyScope, number> = {
  modal: 5,
  suggest: 4,
  terminal: 3,
  explorer: 3,
  files: 3,
  workspace: 2,
  global: 1,
};

// Dispatcher state
const isPaused = ref(false);
const modalCount = ref(0);
const suggestElements = new Set<HTMLElement>();
const activeBindings = ref<ResolvedKeybinding[]>([]);
const handlers = new Map<string, Set<CommandHandler>>();
const currentRoutePath = ref('/');

export function setDispatcherRoute(path: string) {
  currentRoutePath.value = path;
}

export function pauseDispatcher() {
  isPaused.value = true;
}

export function resumeDispatcher() {
  isPaused.value = false;
}

export function openModalScope() {
  modalCount.value++;
}

export function closeModalScope() {
  if (modalCount.value > 0) {
    modalCount.value--;
  }
}

export function setSuggestScope(el: HTMLElement | null | undefined, active: boolean) {
  if (!el) return;
  if (active) {
    suggestElements.add(el);
  } else {
    suggestElements.delete(el);
  }
}

export function updateEffectiveBindings(bindings: ResolvedKeybinding[]) {
  activeBindings.value = bindings;
}

// Initial default bindings setup
function computeInitialDefaults(): ResolvedKeybinding[] {
  const result: ResolvedKeybinding[] = [];
  for (const cmd of COMMAND_REGISTRY) {
    for (const d of cmd.defaults) {
      result.push({
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
  return result;
}

activeBindings.value = computeInitialDefaults();

export function getCurrentScopes(): Set<KeyScope> {
  const scopes = new Set<KeyScope>();

  // If a modal is open (either via modalCount or via .modal-backdrop in DOM), ONLY modal scope is allowed
  const hasModalInDom =
    typeof document !== 'undefined' &&
    Boolean(document.querySelector('.modal-backdrop'));

  if (modalCount.value > 0 || hasModalInDom) {
    scopes.add('modal');
    return scopes;
  }

  scopes.add('global');

  if (currentRoutePath.value.startsWith('/workspace')) {
    scopes.add('workspace');
  }

  if (typeof document !== 'undefined' && document.activeElement) {
    const active = document.activeElement;
    if (active.closest('.xterm') || active.closest('.xterm-screen') || active.tagName === 'CANVAS') {
      scopes.add('terminal');
    }
    if (active.closest('.group-tree-container') || active.closest('.explorer-sidebar')) {
      scopes.add('explorer');
    }
    // A Files pane, but not while typing a name or a path inside it.
    if (active.closest('.files-pane') && active.tagName !== 'INPUT' && active.tagName !== 'TEXTAREA') {
      scopes.add('files');
    }
    for (const el of suggestElements) {
      if (el === active || el.contains(active)) {
        scopes.add('suggest');
        break;
      }
    }
  }

  return scopes;
}

function chordMatches(bindingChord: string, eventChord: string): boolean {
  if (bindingChord === eventChord) return true;

  // Handle 'mod' on Mac vs Linux/Win
  const isMac = typeof navigator !== 'undefined' && /Mac|iPod|iPhone|iPad/.test(navigator.platform);
  const primaryMod = isMac ? 'meta' : 'ctrl';

  const expandedBinding = bindingChord
    .split('+')
    .map((m) => (m === 'mod' ? primaryMod : m))
    .sort()
    .join('+');

  const normalizedEvent = eventChord.split('+').sort().join('+');
  return expandedBinding === normalizedEvent;
}

// Ctrl+K may close the palette, but must not open it over another dialog.
function onlyPaletteIsOpen(): boolean {
  if (typeof document === 'undefined') return false;
  const backdrops = document.querySelectorAll('.modal-backdrop');
  return backdrops.length === 1 && backdrops[0].classList.contains('palette-backdrop');
}

function findMatchingBinding(
  event: KeyboardEvent
): { binding: ResolvedKeybinding } | null {
  if (isImeComposing(event)) return null;

  const parsed = eventToChord(event);
  if (!parsed || parsed.isModifierOnly) return null;

  const activeScopes = getCurrentScopes();

  const candidates: { binding: ResolvedKeybinding; priority: number }[] = [];

  for (const b of activeBindings.value) {
    if (b.isRemoved) continue;
    const isAllowedInScope =
      activeScopes.has(b.when) ||
      (b.commandId === 'app.commandPalette' && onlyPaletteIsOpen());
    if (!isAllowedInScope) continue;

    if (chordMatches(b.key, parsed.chord)) {
      // If binding has skipInEditable, skip when focus is inside input, textarea, contenteditable, or .xterm
      if (b.skipInEditable && typeof document !== 'undefined') {
        const active = document.activeElement;
        const isInEditable =
          active &&
          (active.tagName === 'INPUT' ||
            active.tagName === 'TEXTAREA' ||
            (active as HTMLElement).isContentEditable ||
            Boolean(active.closest('.xterm')));
        if (isInEditable) {
          continue;
        }
      }

      const cmdHandlers = handlers.get(b.commandId);
      if (cmdHandlers && cmdHandlers.size > 0) {
        const priority = SCOPE_PRIORITIES[b.when] || 0;
        candidates.push({ binding: b, priority });
      }
    }
  }

  if (candidates.length === 0) return null;

  // Sort candidates by:
  // 1. Scope priority descending
  // 2. User override > Default
  candidates.sort((a, b) => {
    if (b.priority !== a.priority) {
      return b.priority - a.priority;
    }
    const aIsUser = a.binding.source === 'user' ? 1 : 0;
    const bIsUser = b.binding.source === 'user' ? 1 : 0;
    return bIsUser - aIsUser;
  });

  return candidates[0];
}

/**
 * Checks if the event matches any keybinding in the current active scope.
 * Used by XtermPane to decide if `attachCustomKeyEventHandler` should return false.
 */
export function matchesBinding(event: KeyboardEvent): boolean {
  if (isPaused.value) return false;
  return findMatchingBinding(event) !== null;
}

/**
 * Registers a handler for a command ID.
 * Returns an unregister function.
 */
export function registerCommandHandler(commandId: string, handler: CommandHandler): () => void {
  let set = handlers.get(commandId);
  if (!set) {
    set = new Set();
    handlers.set(commandId, set);
  }
  set.add(handler);

  return () => {
    const current = handlers.get(commandId);
    if (current) {
      current.delete(handler);
      if (current.size === 0) {
        handlers.delete(commandId);
      }
    }
  };
}

/**
 * Vue composable to register a command handler tied to component lifecycle.
 */
export function useKeyCommand(commandId: string, handler: CommandHandler) {
  let unregister: (() => void) | null = null;

  onMounted(() => {
    unregister = registerCommandHandler(commandId, handler);
  });

  onBeforeUnmount(() => {
    if (unregister) {
      unregister();
      unregister = null;
    }
  });
}

/**
 * Returns dynamic key label for a given command, e.g. "(Ctrl+Alt+R)" or "".
 */
export function keyLabel(commandId: string, parentheses = true): string {
  const binding = activeBindings.value.find((b) => b.commandId === commandId && !b.isRemoved);
  if (!binding || !binding.key) return '';
  const formatted = formatChordString(binding.key);
  return parentheses ? `(${formatted})` : formatted;
}

// Global Single Listener
function handleKeyDown(event: KeyboardEvent) {
  if (isPaused.value) return;

  const match = findMatchingBinding(event);
  if (match) {
    event.preventDefault();
    event.stopPropagation();
    const cmdHandlers = handlers.get(match.binding.commandId);
    if (cmdHandlers) {
      for (const handler of Array.from(cmdHandlers).reverse()) {
        handler(event);
      }
    }
  }
}

let listenerInitialized = false;

export function initGlobalKeybindingsListener() {
  if (listenerInitialized || typeof window === 'undefined') return;
  window.addEventListener('keydown', handleKeyDown, { capture: true });
  listenerInitialized = true;
}

export function disposeGlobalKeybindingsListener() {
  if (!listenerInitialized || typeof window === 'undefined') return;
  window.removeEventListener('keydown', handleKeyDown, { capture: true });
  listenerInitialized = false;
}
