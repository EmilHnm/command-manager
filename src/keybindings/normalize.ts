import type { KeyScope } from '@/types/keybindings';

export const MODIFIERS = ['mod', 'ctrl', 'shift', 'alt', 'meta'] as const;

export const NAMED_KEYS = new Set([
  'space',
  'enter',
  'escape',
  'tab',
  'backspace',
  'delete',
  'insert',
  'home',
  'end',
  'pageup',
  'pagedown',
  'up',
  'down',
  'left',
  'right',
  'contextmenu',
  'numpadadd',
  'numpadsubtract',
  'numpadmultiply',
  'numpaddivide',
  'numpaddecimal',
  'numpadenter',
]);

export const PUNCTUATION = new Set(['`', '-', '=', '[', ']', '\\', ';', "'", ',', '.', '/']);

const SHELL_CLASH_KEYS = new Set([
  'ctrl+c',
  'ctrl+d',
  'ctrl+z',
  'ctrl+l',
  'ctrl+r',
  'ctrl+a',
  'ctrl+e',
  'ctrl+u',
  'ctrl+w',
  'ctrl+k',
  'ctrl+left',
  'ctrl+right',
]);

const CODE_TO_KEY: Record<string, string> = {
  // Letters
  KeyA: 'a',
  KeyB: 'b',
  KeyC: 'c',
  KeyD: 'd',
  KeyE: 'e',
  KeyF: 'f',
  KeyG: 'g',
  KeyH: 'h',
  KeyI: 'i',
  KeyJ: 'j',
  KeyK: 'k',
  KeyL: 'l',
  KeyM: 'm',
  KeyN: 'n',
  KeyO: 'o',
  KeyP: 'p',
  KeyQ: 'q',
  KeyR: 'r',
  KeyS: 's',
  KeyT: 't',
  KeyU: 'u',
  KeyV: 'v',
  KeyW: 'w',
  KeyX: 'x',
  KeyY: 'y',
  KeyZ: 'z',

  // Digits
  Digit0: '0',
  Digit1: '1',
  Digit2: '2',
  Digit3: '3',
  Digit4: '4',
  Digit5: '5',
  Digit6: '6',
  Digit7: '7',
  Digit8: '8',
  Digit9: '9',

  // Punctuation
  Backquote: '`',
  Minus: '-',
  Equal: '=',
  BracketLeft: '[',
  BracketRight: ']',
  Backslash: '\\',
  Semicolon: ';',
  Quote: "'",
  Comma: ',',
  Period: '.',
  Slash: '/',

  // Controls & Navigation
  Space: 'space',
  Enter: 'enter',
  Escape: 'escape',
  Tab: 'tab',
  Backspace: 'backspace',
  Delete: 'delete',
  Insert: 'insert',
  Home: 'home',
  End: 'end',
  PageUp: 'pageup',
  PageDown: 'pagedown',
  ArrowUp: 'up',
  ArrowDown: 'down',
  ArrowLeft: 'left',
  ArrowRight: 'right',
  ContextMenu: 'contextmenu',

  // Numpad
  Numpad0: 'numpad0',
  Numpad1: 'numpad1',
  Numpad2: 'numpad2',
  Numpad3: 'numpad3',
  Numpad4: 'numpad4',
  Numpad5: 'numpad5',
  Numpad6: 'numpad6',
  Numpad7: 'numpad7',
  Numpad8: 'numpad8',
  Numpad9: 'numpad9',
  NumpadAdd: 'numpadadd',
  NumpadSubtract: 'numpadsubtract',
  NumpadMultiply: 'numpadmultiply',
  NumpadDivide: 'numpaddivide',
  NumpadDecimal: 'numpaddecimal',
  NumpadEnter: 'numpadenter',
};

// Add F1 to F24
for (let i = 1; i <= 24; i++) {
  CODE_TO_KEY[`F${i}`] = `f${i}`;
}

export function isImeComposing(event: KeyboardEvent): boolean {
  return event.isComposing || event.key === 'Process' || event.keyCode === 229;
}

export function isModifierCode(code: string): boolean {
  return (
    code.startsWith('Control') ||
    code.startsWith('Shift') ||
    code.startsWith('Alt') ||
    code.startsWith('Meta') ||
    code === 'OSLeft' ||
    code === 'OSRight'
  );
}

export function codeToMainKey(code: string): string | null {
  return CODE_TO_KEY[code] || null;
}

export function splitChord(chord: string): { modifiers: string[]; main: string } {
  const parts = chord.split('+').map((s) => s.trim().toLowerCase());
  const main = parts.pop() || '';
  return { modifiers: parts, main };
}

export function normalizeKey(raw: string): string {
  const lowered = raw.trim().toLowerCase();
  if (!lowered) throw new Error('Phím tắt trống');
  if (lowered.includes(' ')) {
    throw new Error(`Chưa hỗ trợ phím tắt nhiều bước: ${raw}`);
  }

  const parts = lowered.split('+').map((s) => s.trim());
  const main = parts.pop();
  if (!main) throw new Error(`Phím không hợp lệ: ${raw}`);

  const singleAlnum = /^[a-z0-9]$/.test(main);
  const functionKey = /^f([1-9]|1[0-9]|2[0-4])$/.test(main);
  const numpadDigit = /^numpad[0-9]$/.test(main);

  if (!singleAlnum && !functionKey && !numpadDigit && !NAMED_KEYS.has(main) && !PUNCTUATION.has(main)) {
    throw new Error(`Phím không hợp lệ: ${raw}`);
  }

  const present = new Map<string, boolean>();
  for (const mod of parts) {
    if (!MODIFIERS.includes(mod as any)) {
      throw new Error(`Phím bổ trợ không hợp lệ: ${raw}`);
    }
    if (present.get(mod)) {
      throw new Error(`Phím bổ trợ bị lặp: ${raw}`);
    }
    present.set(mod, true);
  }

  if (present.get('mod') && (present.get('ctrl') || present.get('meta'))) {
    throw new Error(`Không dùng \`mod\` cùng ctrl/meta: ${raw}`);
  }

  const chordParts: string[] = [];
  for (const mod of MODIFIERS) {
    if (present.get(mod)) {
      chordParts.push(mod);
    }
  }
  chordParts.push(main);
  return chordParts.join('+');
}

export function eventToChord(event: KeyboardEvent): { chord: string; isModifierOnly: boolean } | null {
  if (isImeComposing(event)) return null;

  const isMod = isModifierCode(event.code);
  const main = isMod ? '' : codeToMainKey(event.code);

  const modifiers: string[] = [];
  if (event.ctrlKey) modifiers.push('ctrl');
  if (event.shiftKey) modifiers.push('shift');
  if (event.altKey) modifiers.push('alt');
  if (event.metaKey) modifiers.push('meta');

  if (isMod) {
    return {
      chord: modifiers.join('+'),
      isModifierOnly: true,
    };
  }

  if (!main) return null;

  const chord = [...modifiers, main].join('+');
  return {
    chord,
    isModifierOnly: false,
  };
}

export function getLockedReason(chord: string): string | null {
  const { modifiers, main } = splitChord(chord);
  const ctrl = modifiers.includes('ctrl') || modifiers.includes('mod');
  const alt = modifiers.includes('alt');
  const onlyShift = modifiers.every((m) => m === 'shift');

  if (main === 'tab' && onlyShift) {
    return 'Tab và Shift+Tab dùng để di chuyển focus';
  }
  if (main === 'escape' && modifiers.length === 0) {
    return 'Esc dùng để đóng modal và popup';
  }
  if (ctrl && alt && main === 'delete') {
    return 'Ctrl+Alt+Delete do hệ điều hành giữ';
  }
  if (ctrl && alt && /^f([1-9]|1[0-2])$/.test(main)) {
    return 'Ctrl+Alt+F1..F12 dùng để đổi TTY trên Linux';
  }
  return null;
}

export function stealsTyping(chord: string): boolean {
  const { modifiers, main } = splitChord(chord);
  const hasCommandModifier = modifiers.some((m) => m !== 'shift');
  const functionKey = /^f([1-9]|1[0-9]|2[0-4])$/.test(main);
  return !hasCommandModifier && !functionKey;
}

export function getShellWarning(chord: string, scope?: string): string | null {
  if (!scope || ['global', 'workspace', 'terminal'].includes(scope)) {
    const normalized = chord.toLowerCase();
    if (SHELL_CLASH_KEYS.has(normalized)) {
      return 'Shell và các chương trình như vim, htop đang dùng phím này (ví dụ ^C = dừng lệnh). Gán sẽ khiến terminal không nhận được phím đó nữa.';
    }
  }
  return null;
}

export function formatChordForDisplay(
  chord: string,
  isMac: boolean = typeof navigator !== 'undefined' && /Mac|iPod|iPhone|iPad/.test(navigator.platform)
): string[] {
  if (!chord) return [];
  const parts = chord.split('+').map((s) => s.trim().toLowerCase());
  return parts.map((part) => {
    switch (part) {
      case 'mod':
        return isMac ? 'Cmd' : 'Ctrl';
      case 'ctrl':
        return 'Ctrl';
      case 'shift':
        return 'Shift';
      case 'alt':
        return 'Alt';
      case 'meta':
        return isMac ? 'Cmd' : 'Meta';
      case 'space':
        return 'Space';
      case 'enter':
        return 'Enter';
      case 'escape':
        return 'Esc';
      case 'tab':
        return 'Tab';
      case 'backspace':
        return 'Backspace';
      case 'delete':
        return 'Delete';
      case 'insert':
        return 'Insert';
      case 'home':
        return 'Home';
      case 'end':
        return 'End';
      case 'pageup':
        return 'PageUp';
      case 'pagedown':
        return 'PageDown';
      case 'up':
        return '↑';
      case 'down':
        return '↓';
      case 'left':
        return '←';
      case 'right':
        return '→';
      default:
        if (part.startsWith('f') && /^f\d+$/.test(part)) {
          return part.toUpperCase();
        }
        if (part.startsWith('numpad')) {
          return `Num ${part.replace('numpad', '')}`;
        }
        return part.length === 1 ? part.toUpperCase() : part;
    }
  });
}

export function formatChordString(chord: string, isMac?: boolean): string {
  return formatChordForDisplay(chord, isMac).join('+');
}

export function getScopeLabel(scope: KeyScope | string): string {
  switch (scope) {
    case 'global':
      return 'Toàn ứng dụng';
    case 'workspace':
      return 'Workspace';
    case 'explorer':
      return 'Cây nhóm lệnh';
    case 'terminal':
      return 'Terminal đang gõ';
    case 'suggest':
      return 'Terminal có gợi ý';
    case 'modal':
      return 'Modal';
    default:
      return scope;
  }
}
