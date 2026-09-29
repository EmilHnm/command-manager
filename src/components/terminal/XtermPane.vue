<template>
  <div class="xterm-pane-container">
    <!-- Pane Toolbar -->
    <div class="pane-toolbar">
      <div class="toolbar-left">
        <span class="cmd-badge" :class="processStatus">
          <span class="status-dot" :class="{ active: processStatus === 'running' }" />
          {{ commandName }}<span v-if="commandId > 0"> (ID: #{{ commandId }})</span>
        </span>
        <span v-if="pid" class="pid-tag">PID: {{ pid }}</span>
        <span v-if="processStatus === 'completed'" class="exit-tag success">Exit: 0</span>
        <span v-if="processStatus === 'failed'" class="exit-tag error">Exit: 1</span>
      </div>

      <div class="toolbar-right">
        <button class="btn btn-ghost btn-sm" title="Xả lại dữ liệu từ Ring Buffer in-memory" @click="handleReattach">
          <RefreshCw :size="13" />
          <span>Reattach</span>
        </button>
        <button class="btn btn-ghost btn-sm" title="Xóa màn hình cục bộ" @click="handleClear">
          <Trash2 :size="13" />
          <span>Clear</span>
        </button>
        <button
          v-if="processStatus === 'running'"
          class="btn btn-danger btn-sm"
          title="Dừng tiến trình (Gửi SIGTERM)"
          @click="$emit('stop-process', commandId)"
        >
          <Square :size="12" />
          <span>Dừng Lệnh</span>
        </button>
        <button
          v-else
          class="btn btn-primary btn-sm"
          title="Khởi động lại lệnh"
          :disabled="restarting"
          @click="$emit('restart-process', commandId)"
        >
          <LoaderCircle v-if="restarting" :size="12" class="spin" />
          <RotateCw v-else :size="12" />
          <span>{{ restarting ? 'Đang Khởi Động...' : 'Khởi Động Lại' }}</span>
        </button>
      </div>
    </div>

    <div v-if="terminalError" role="alert" class="terminal-error">
      Không thể kết nối terminal: {{ terminalError }}
    </div>
    <!-- Terminal Viewport -->
    <div
      ref="terminalElement"
      class="terminal-viewport"
      @click="focusTerminal"
    >
      <div v-if="suggestionText" class="ghost-suggestion" :style="[suggestionPosition, ghostFontStyle]" aria-hidden="true">{{ suggestionText }}</div>
      <div v-if="suggestionPopup" class="suggestion-popup" :style="popupPosition">
        <button
          v-for="item in suggestionItems"
          :key="item.id"
          type="button"
          class="suggestion-item"
          :class="{ selected: suggestionItems.indexOf(item) === suggestionIndex }"
          @mousedown.prevent="acceptSuggestion(item.command_line)"
        >
          <span>{{ item.command_line }}</span>
          <small>{{ item.shell_kind }} · {{ item.run_count }}×</small>
        </button>
        <span v-if="suggestionItems.length === 0" class="suggestion-empty">Chưa có lịch sử phù hợp</span>
      </div>
    </div>

    <!-- Link Hover Tooltip (Ctrl+Click hint) -->
    <Teleport to="body">
      <div
        v-if="linkTooltip.visible"
        class="terminal-link-tooltip"
        :class="{ 'ctrl-needed': linkTooltip.ctrlNeeded, 'ctrl-active': isCtrlPressed }"
        :style="tooltipStyle"
      >
        <ExternalLink :size="12" class="link-icon" />
        <span v-if="linkTooltip.ctrlNeeded" class="link-hint-text">
          Nhấn giữ <kbd class="key-badge warning">{{ isMac ? '⌘ Cmd' : 'Ctrl' }}</kbd> và click để mở
        </span>
        <span v-else-if="isCtrlPressed" class="link-hint-text">
          Click để mở: <span class="link-url">{{ linkTooltip.url }}</span>
        </span>
        <span v-else class="link-hint-text">
          <kbd class="key-badge">{{ isMac ? '⌘ Cmd' : 'Ctrl' }}</kbd> + click để mở liên kết
        </span>
      </div>
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted, onBeforeUnmount } from 'vue';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { WebLinksAddon } from '@xterm/addon-web-links';
import { RefreshCw, Trash2, Square, RotateCw, LoaderCircle, ExternalLink } from 'lucide-vue-next';
import { usePtyStream } from '@/composables/usePtyStream';
import type { ProcessLifecycleStatus } from '@/types/models';
import type { CommandHistory } from '@/types/models';
import { ipcClient } from '@/ipc/client';
import { useSuggestions } from '@/composables/useSuggestions';

const props = withDefaults(
  defineProps<{
    commandId: number;
    commandName: string;
    runEventId?: string;
    pid?: number;
    processStatus?: ProcessLifecycleStatus;
    shellKind?: string;
    historyLevel?: number;
    ghostTextEnabled?: boolean;
    fontFamily?: string;
    fontSize?: number;
    restarting?: boolean;
  }>(),
  {
    processStatus: 'idle',
    fontFamily: 'JetBrains Mono',
    fontSize: 13,
    restarting: false,
  }
);

// Nerd Font icons from prompt themes (oh-my-posh) fall back to the bundled
// glyph-only font, whatever terminal font the user picked.
const terminalFontFamily = computed(() =>
  `'${props.fontFamily.replace(/'/g, '')}', 'Symbols Nerd Font Mono', monospace`,
);

const ghostFontStyle = computed(() => ({
  fontFamily: terminalFontFamily.value,
  fontSize: `${props.fontSize}px`,
}));

// xterm measures the cell size from the font when it opens or when the font
// option changes; measuring a font that is still loading makes glyphs overlap.
const loadTerminalFonts = async () => {
  if (!document.fonts) return;
  const size = `${props.fontSize}px`;
  await Promise.all([
    document.fonts.load(`${size} '${props.fontFamily.replace(/'/g, '')}'`),
    document.fonts.load(`${size} 'Symbols Nerd Font Mono'`, ''),
  ]).catch(() => undefined);
};

watch(() => [props.fontFamily, props.fontSize], async () => {
  if (!term) return;
  await loadTerminalFonts();
  if (!term) return;
  term.options.fontFamily = terminalFontFamily.value;
  term.options.fontSize = props.fontSize;
  fitAddon?.fit();
  updateSuggestionPosition();
});

const emit = defineEmits<{
  (e: 'stop-process', id: number): void;
  (e: 'restart-process', id: number): void;
  (e: 'cwd-change', cwd: string): void;
}>();

const terminalElement = ref<HTMLDivElement | null>(null);
let term: Terminal | null = null;
let fitAddon: FitAddon | null = null;
let resizeObserver: ResizeObserver | null = null;
let unsubscribePty: (() => void) | null = null;
let renderDisposable: { dispose: () => void } | null = null;
let writeParsedDisposable: { dispose: () => void } | null = null;
let suggestionFrame: number | undefined;
// Output-driven refreshes run on every shell redraw; only move the popup
// selection back to the top when the typed line actually changed.
let lastSuggestionQuery: string | undefined;
// ↑/↓ on a partly typed line only change the ghost text (the typed text stays
// the command): the history entries of this shell that start with the typed
// text, newest first, and the one currently shown.
let ghostCandidates: CommandHistory[] = [];
let ghostIndex = 0;
let ghostQuery: string | undefined;
const terminalError = ref('');
const suggestionText = ref('');
const suggestionPopup = ref(false);
const suggestionItems = ref<CommandHistory[]>([]);
const suggestionIndex = ref(0);
const suggestionPosition = ref<{ left: string; top: string; height?: string; lineHeight?: string; maxWidth?: string }>({ left: '18px', top: '12px' });
const popupPosition = ref({ left: '14px', top: '12px' });
let promptMarker: ReturnType<Terminal['registerMarker']> | undefined;
let promptColumn = 0;
// Some prompts (oh-my-posh, starship right_format) paint status text on the
// same row, then restore the cursor to the input column. Remember where that
// pre-existing right-side text begins so it is not treated as user input.
let rightPromptStartColumn: number | undefined;
let currentCwd: string | undefined;
let shellPhase: 'prompt' | 'input' | 'running' = 'prompt';
let bracketedPaste = false;
let composing = false;
// Fallback cho level 2: trong browser mock hoặc khi shell chưa kịp echo,
// marker B có thể chưa chứa dòng vừa gõ lúc onData nhận Enter. Shadow này
// chỉ dùng khi marker không đọc được; native backend vẫn validate lại lệnh.
let inputShadow = '';
let inputScanTimer: number | undefined;
let compositionTarget: HTMLTextAreaElement | null = null;
let onCompositionStart: (() => void) | undefined;
let onCompositionEnd: (() => void) | undefined;
let onCompositionCancel: (() => void) | undefined;

const { subscribePty, sendInput, resize, reattachBuffer } = usePtyStream();
const { loadHistory, findSuggestions, findFuzzySuggestions, findHistoryEntries } = useSuggestions(() => {
  void refreshSuggestions();
});

let historyNavigationIndex = -1;
let historyNavigationPrefix = '';
let historyNavigationOriginal = '';
let historyNavigationSent = '';
let historyNavigationRows: CommandHistory[] = [];

const isMac = typeof navigator !== 'undefined' && /Mac|iPod|iPhone|iPad/.test(navigator.platform);
const linkTooltip = ref<{
  visible: boolean;
  url: string;
  x: number;
  y: number;
  ctrlNeeded: boolean;
}>({
  visible: false,
  url: '',
  x: 0,
  y: 0,
  ctrlNeeded: false,
});
const isCtrlPressed = ref(false);
let ctrlNoticeTimer: number | undefined;
let webLinksAddon: WebLinksAddon | null = null;

const tooltipStyle = computed(() => {
  const x = Math.min(Math.max(linkTooltip.value.x, 120), window.innerWidth - 120);
  const y = Math.max(linkTooltip.value.y - 12, 32);
  return {
    left: `${x}px`,
    top: `${y}px`,
  };
});

const showLinkTooltip = (event: MouseEvent, text: string) => {
  if (ctrlNoticeTimer) clearTimeout(ctrlNoticeTimer);
  isCtrlPressed.value = event.ctrlKey || event.metaKey;
  linkTooltip.value = {
    visible: true,
    url: text,
    x: event.clientX,
    y: event.clientY,
    ctrlNeeded: false,
  };
};

const hideLinkTooltip = () => {
  if (ctrlNoticeTimer) clearTimeout(ctrlNoticeTimer);
  linkTooltip.value.visible = false;
  linkTooltip.value.ctrlNeeded = false;
};

const handleLinkClick = (event: MouseEvent, uri: string) => {
  if (event.ctrlKey || event.metaKey) {
    hideLinkTooltip();
    void ipcClient.openUrl(uri);
  } else {
    if (ctrlNoticeTimer) clearTimeout(ctrlNoticeTimer);
    linkTooltip.value = {
      visible: true,
      url: uri,
      x: event.clientX,
      y: event.clientY,
      ctrlNeeded: true,
    };
    ctrlNoticeTimer = window.setTimeout(() => {
      if (linkTooltip.value.ctrlNeeded) {
        linkTooltip.value.ctrlNeeded = false;
      }
    }, 2000);
  }
};

const handleWindowKeyDown = (e: KeyboardEvent) => {
  if (e.key === 'Control' || e.key === 'Meta') {
    isCtrlPressed.value = true;
  }
};

const handleWindowKeyUp = (e: KeyboardEvent) => {
  if (e.key === 'Control' || e.key === 'Meta') {
    isCtrlPressed.value = false;
  }
};

onMounted(async () => {
  if (!terminalElement.value) return;

  await loadTerminalFonts();
  if (!terminalElement.value) return;

  // Khởi tạo xterm.js với Dark Theme đồng bộ primary #744791
  term = new Terminal({
    fontFamily: terminalFontFamily.value,
    fontSize: props.fontSize,
    lineHeight: 1.4,
    cursorBlink: true,
    cursorStyle: 'block',
    cursorInactiveStyle: 'block',
    convertEol: true,
    theme: {
      background: '#0b0d13',
      foreground: '#f1f5f9',
      cursor: '#8956aa',
      cursorAccent: '#ffffff',
      selectionBackground: 'rgba(116, 71, 145, 0.4)',
      black: '#1a1e2b',
      red: '#ef4444',
      green: '#10b981',
      yellow: '#f59e0b',
      blue: '#3b82f6',
      magenta: '#a855f7',
      cyan: '#06b6d4',
      white: '#f8fafc',
      brightBlack: '#475569',
      brightRed: '#f87171',
      brightGreen: '#34d399',
      brightYellow: '#fbbf24',
      brightBlue: '#60a5fa',
      brightMagenta: '#c084fc',
      brightCyan: '#22d3ee',
      brightWhite: '#ffffff',
    },
  });

  fitAddon = new FitAddon();
  term.loadAddon(fitAddon);

  webLinksAddon = new WebLinksAddon(
    (event: MouseEvent, uri: string) => {
      handleLinkClick(event, uri);
    },
    {
      hover: (event: MouseEvent, text: string) => {
        showLinkTooltip(event, text);
      },
      leave: () => {
        hideLinkTooltip();
      },
    },
  );
  term.loadAddon(webLinksAddon);

  term.open(terminalElement.value);
  fitAddon.fit();
  term.focus();
  window.addEventListener('keydown', handleWindowKeyDown, { passive: true });
  window.addEventListener('keyup', handleWindowKeyUp, { passive: true });
  window.setTimeout(() => {
    term?.focus();
  }, 50);
  compositionTarget = term.element?.querySelector<HTMLTextAreaElement>('.xterm-helper-textarea') || null;
  if (compositionTarget) {
    onCompositionStart = () => {
      composing = true;
      suggestionText.value = '';
      suggestionPopup.value = false;
    };
    onCompositionEnd = () => {
      composing = false;
      if (inputScanTimer) clearTimeout(inputScanTimer);
      inputScanTimer = window.setTimeout(() => {
        void refreshSuggestions();
      }, 0);
    };
    onCompositionCancel = onCompositionEnd;
    compositionTarget.addEventListener('compositionstart', onCompositionStart);
    compositionTarget.addEventListener('compositionend', onCompositionEnd);
    compositionTarget.addEventListener('compositioncancel', onCompositionCancel);
  }
  renderDisposable = term.onRender(() => updateSuggestionPosition());
  // Like zsh-autosuggestions, derive the suggestion from the line the shell
  // actually drew. Refreshing on a timer after a keystroke could read the
  // line before the shell echoed it and paint a stale suggestion over it.
  writeParsedDisposable = term.onWriteParsed(() => scheduleSuggestionRefresh());

  await loadHistory();

  term.parser.registerOscHandler(633, (data) => {
    const separator = data.indexOf(';');
    const kind = separator === -1 ? data : data.slice(0, separator);
    const payload = separator === -1 ? '' : data.slice(separator + 1);
    if (kind === 'A') {
      shellPhase = 'prompt';
      inputShadow = '';
      rightPromptStartColumn = undefined;
      resetHistoryNavigation();
      resetGhostCycle();
      suggestionText.value = '';
      suggestionPopup.value = false;
    } else if (kind === 'B') {
      promptMarker?.dispose();
      promptMarker = term?.registerMarker(0);
      promptColumn = term?.buffer.active.cursorX ?? 0;
      rightPromptStartColumn = findRightPromptStartColumn(promptMarker?.line, promptColumn);
      inputShadow = '';
      resetHistoryNavigation();
      shellPhase = 'input';
      term?.write('\x1b[?25h');
    } else if (kind === 'C') {
      shellPhase = 'running';
      suggestionText.value = '';
      suggestionPopup.value = false;
    } else if (kind === 'D') {
      shellPhase = 'prompt';
    } else if (kind === 'P' && payload.startsWith('Cwd=')) {
      currentCwd = decodeOsc(payload.slice(4));
      emit('cwd-change', currentCwd);
    }
    return true;
  });

  // xterm emits onData after onKey. preventDefault() alone therefore still
  // forwards popup shortcuts (or Ctrl+Space) to the shell. Returning false
  // here prevents xterm from translating those browser events into PTY data;
  // the custom handler also owns the action because xterm will not emit
  // onKey for an event it is told not to process.
  term.attachCustomKeyEventHandler((event) => {
    if (event.type !== 'keydown') return true;
    // Ctrl+N opens a new terminal (handled by DockHost on window); keep it
    // from reaching the shell as ^N.
    if (event.ctrlKey && !event.shiftKey && !event.altKey && !event.metaKey
      && event.key.toLowerCase() === 'n') {
      return false;
    }
    if (event.ctrlKey && (event.code === 'Space' || event.key === ' ')) {
      event.preventDefault();
      event.stopPropagation();
      suggestionPopup.value = !suggestionPopup.value;
      lastSuggestionQuery = undefined;
      if (suggestionPopup.value) void refreshSuggestions();
      return false;
    }
    if (suggestionPopup.value && event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      suggestionPopup.value = false;
      return false;
    }
    if (suggestionPopup.value && (event.key === 'ArrowDown' || event.key === 'ArrowUp')) {
      event.preventDefault();
      event.stopPropagation();
      const delta = event.key === 'ArrowDown' ? 1 : -1;
      suggestionIndex.value = Math.max(0, Math.min(
        Math.max(0, suggestionItems.value.length - 1),
        suggestionIndex.value + delta,
      ));
      return false;
    }
    if (suggestionPopup.value && event.key === 'Enter') {
      event.preventDefault();
      event.stopPropagation();
      if (suggestionItems.value.length) {
        acceptSuggestion(suggestionItems.value[suggestionIndex.value]?.command_line || '');
      }
      return false;
    }
    if ((event.key === 'ArrowUp' || event.key === 'ArrowDown')
      && !event.ctrlKey && !event.altKey && !event.metaKey
      && (props.historyLevel === 1 || props.historyLevel === 2)
      && cursorIsOnPromptLogicalLine()
      && cursorIsAtInputEnd()) {
      const direction = event.key === 'ArrowUp' ? 1 : -1;
      if (props.ghostTextEnabled && historyNavigationIndex === -1 && currentInput().trim()) {
        // Like zsh: "rc" + ↑ shows the next older entry starting with "rc" as
        // ghost text; ↓ goes back towards the newest. The line is untouched
        // and the key never reaches the shell, whose ↑ would replace it.
        event.preventDefault();
        event.stopPropagation();
        cycleGhost(direction);
        return false;
      }
      if (navigateHistory(direction)) {
        event.preventDefault();
        event.stopPropagation();
        return false;
      }
    }
    if (suggestionText.value && (
      event.key === 'End'
      || event.key === 'ArrowRight'
      || (event.ctrlKey && event.key === 'ArrowRight')
    )) {
      event.preventDefault();
      event.stopPropagation();
      if (event.key === 'ArrowRight' && event.ctrlKey) {
        const word = suggestionText.value.match(/^\s*\S+\s*/)?.[0] || suggestionText.value;
        acceptSuggestion(currentInput() + word);
      } else {
        acceptSuggestion(currentInput() + suggestionText.value);
      }
      return false;
    }
    return true;
  });

  // Gửi input bàn phím tới Rust PTY
  term.onData((data) => {
    // ConPTY focus reporting is terminal protocol traffic, not user editing.
    // Do not lose the ↑/↓ position when the app window is refocused.
    if (!isFocusReport(data)) resetHistoryNavigation();
    if (data.includes('\x1b[200~')) bracketedPaste = true;
    if (data.includes('\x1b[201~')) bracketedPaste = false;
    const levelTwoEnter = props.historyLevel === 2
      && shellPhase === 'input'
      && !composing
      && /[\r\n]/.test(data);
    const shadowAtEnter = levelTwoEnter
      ? inputShadow
      : '';
    if (props.historyLevel === 2 && !composing) {
      updateInputShadow(data);
    }
    const typedLineAtEnter = levelTwoEnter
      ? readInputFromMarker(true)
      : '';
    if (levelTwoEnter) {
      // Level 2 has no shell-side C marker. Once Enter is sent, suspend
      // recording until the next prompt boundary; this avoids saving every
      // Enter pressed inside a running REPL or password prompt as a command.
      shellPhase = 'running';
      window.setTimeout(() => {
        // The shell may have emitted D/A/B and replaced promptMarker before
        // this delayed callback runs. Prefer the line captured before Enter;
        // only fall back to the current marker when xterm had not echoed yet.
        const commandLine = typedLineAtEnter || shadowAtEnter || readInputFromMarker(true);
        if (commandLine.trim()) {
          void ipcClient.recordTypedHistory(props.runEventId || '', commandLine, currentCwd).then(() => {
            void refreshSuggestions();
          }).catch(() => undefined);
        }
      }, 75);
    }
    void sendInput(props.commandId, data, props.runEventId).catch((error) => {
      terminalError.value = String(error);
      console.error('[XtermPane] Không thể gửi input tới PTY:', error);
    });
  });

  // Đăng ký nhận luồng byte từ PTY
  unsubscribePty = subscribePty(props.commandId, (chunk) => {
    term?.write(chunk);
  });

  // Xả dữ liệu gần nhất từ Ring Buffer in-memory
  await handleReattach();
  if (!terminalElement.value || !term) return;
  focusTerminal();

  // Tự động căn kích thước và đồng bộ cols/rows với PTY
  resizeObserver = new ResizeObserver(() => {
    if (fitAddon && term) {
      fitAddon.fit();
      updateSuggestionPosition();
      void resize(props.commandId, term.cols, term.rows, props.runEventId).catch((error) => {
        terminalError.value = String(error);
      });
    }
  });
  resizeObserver.observe(terminalElement.value);
});

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleWindowKeyDown);
  window.removeEventListener('keyup', handleWindowKeyUp);
  if (ctrlNoticeTimer) clearTimeout(ctrlNoticeTimer);
  webLinksAddon?.dispose();
  if (inputScanTimer) clearTimeout(inputScanTimer);
  if (compositionTarget) {
    if (onCompositionStart) compositionTarget.removeEventListener('compositionstart', onCompositionStart);
    if (onCompositionEnd) compositionTarget.removeEventListener('compositionend', onCompositionEnd);
    if (onCompositionCancel) compositionTarget.removeEventListener('compositioncancel', onCompositionCancel);
  }
  renderDisposable?.dispose();
  writeParsedDisposable?.dispose();
  if (suggestionFrame !== undefined) cancelAnimationFrame(suggestionFrame);
  promptMarker?.dispose();
  if (unsubscribePty) unsubscribePty();
  if (resizeObserver) resizeObserver.disconnect();
  if (term) term.dispose();
});

const decodeOsc = (value: string) => value.replace(/\\x([0-9a-fA-F]{2})/g, (_, hex: string) =>
  String.fromCharCode(Number.parseInt(hex, 16))).replace(/\\\\/g, '\\');

const updateInputShadow = (data: string) => {
  // Bracketed paste contains the actual payload between two escape markers.
  // Do not feed the control sequence itself into the history fallback.
  const pasted = data.match(/\x1b\[200~([\s\S]*?)\x1b\[201~/);
  if (pasted) {
    inputShadow += pasted[1].replace(/[\r\n]+/g, '\n');
    return;
  }
  if (data.includes('\x1b')) return;

  for (const character of data) {
    if (character === '\r' || character === '\n') {
      inputShadow = '';
    } else if (character === '\u007f' || character === '\b') {
      inputShadow = Array.from(inputShadow).slice(0, -1).join('');
    } else if (character === '\u0015') {
      inputShadow = '';
    } else if (character === '\u0017') {
      inputShadow = inputShadow.replace(/\s+$/, '').replace(/\S+$/, '');
    } else if (character === '\u0003' || character === '\u0004') {
      inputShadow = '';
    } else if (character >= ' ') {
      inputShadow += character;
    }
  }
};

const readInputFromMarker = (readToLineEnd = false) => {
  if (!term || !promptMarker) return '';
  const buffer = term.buffer.active;
  const end = buffer.baseY + buffer.cursorY;
  const start = promptMarker.line;
  if (start < 0 || end < start) return '';
  const lines: string[] = [];
  for (let line = start; line <= end; line += 1) {
    const startColumn = line === start ? promptColumn : 0;
    const cursorEnd = line === end && !readToLineEnd ? buffer.cursorX : term.cols;
    // If the cursor has not crossed the right prompt, cap the first line at
    // the column where that prompt was painted. Once input overwrites it, the
    // cursor position becomes the authoritative end of the input.
    const endColumn = line === start
      && rightPromptStartColumn !== undefined
      && buffer.cursorX <= rightPromptStartColumn
      ? Math.min(cursorEnd, rightPromptStartColumn)
      : cursorEnd;
    const text = buffer.getLine(line)?.translateToString(true, startColumn, endColumn) || '';
    lines.push(text);
  }
  return lines.join('').trimEnd();
};

const currentInput = () => {
  const markerInput = readInputFromMarker();
  // Level 2 shells can echo a character after the key event that caused the
  // suggestion refresh. Keep the shadow only as a fallback so shell-side
  // readline edits still remain authoritative whenever xterm has content.
  return markerInput || (props.historyLevel === 2 ? inputShadow : '');
};

const resetGhostCycle = () => {
  ghostCandidates = [];
  ghostIndex = 0;
  ghostQuery = undefined;
};

const resetHistoryNavigation = () => {
  historyNavigationIndex = -1;
  historyNavigationPrefix = '';
  historyNavigationOriginal = '';
  historyNavigationSent = '';
  historyNavigationRows = [];
};

const replaceCurrentInput = (replacement: string, currentOverride?: string) => {
  const current = currentOverride ?? currentInput();
  const erase = '\x7f'.repeat(Array.from(current).length);
  historyNavigationSent = replacement;
  if (props.historyLevel === 2) inputShadow = replacement;
  void sendInput(props.commandId, `${erase}${replacement}`, props.runEventId);
  suggestionText.value = '';
  suggestionPopup.value = false;
};

const navigateHistory = (direction: 1 | -1) => {
  if (composing || bracketedPaste || shellPhase !== 'input') return false;
  if (inputScanTimer) clearTimeout(inputScanTimer);

  if (historyNavigationIndex === -1) {
    if (direction < 0) return false;
    historyNavigationOriginal = currentInput();
    historyNavigationPrefix = historyNavigationOriginal;
    historyNavigationRows = findHistoryEntries(historyNavigationPrefix, props.shellKind);
    if (!historyNavigationRows.length) return false;
    historyNavigationIndex = 0;
    replaceCurrentInput(historyNavigationRows[historyNavigationIndex].command_line, historyNavigationOriginal);
  } else {
    const nextIndex = historyNavigationIndex + direction;
    if (nextIndex < 0) {
      historyNavigationIndex = -1;
      replaceCurrentInput(historyNavigationOriginal, historyNavigationSent);
      return true;
    }
    historyNavigationIndex = Math.min(nextIndex, historyNavigationRows.length - 1);
    replaceCurrentInput(historyNavigationRows[historyNavigationIndex].command_line, historyNavigationSent);
  }

  return true;
};

const isFocusReport = (data: string) => data.length > 0
  && data.replace(/\x1b\[I|\x1b\[O/g, '') === '';

const cursorIsOnPromptLogicalLine = () => {
  if (!term || !promptMarker) return false;
  const buffer = term.buffer.active;
  const cursorLine = buffer.baseY + buffer.cursorY;
  for (let line = promptMarker.line + 1; line <= cursorLine; line += 1) {
    if (!buffer.getLine(line)?.isWrapped) return false;
  }
  return cursorLine >= promptMarker.line;
};

const cycleGhost = (direction: 1 | -1) => {
  if (!ghostCandidates.length || suggestionPopup.value) return;
  ghostIndex = Math.max(0, Math.min(ghostIndex + direction, ghostCandidates.length - 1));
  showGhost(currentInput());
};

const showGhost = (current: string) => {
  const entry = ghostCandidates[ghostIndex];
  suggestionText.value = entry ? entry.command_line.slice(current.length) : '';
  updateSuggestionPosition();
};

const scheduleSuggestionRefresh = () => {
  if (suggestionFrame !== undefined || shellPhase !== 'input') return;
  suggestionFrame = requestAnimationFrame(() => {
    suggestionFrame = undefined;
    void refreshSuggestions();
  });
};

const refreshSuggestions = async () => {
  // zsh clears the suggestion while ↑/↓ walk the history and shows it again
  // once the user edits the recalled line.
  if (historyNavigationIndex !== -1) {
    suggestionText.value = '';
    return;
  }
  if (!term || !props.ghostTextEnabled || shellPhase !== 'input' || composing || bracketedPaste || term.buffer.active.type === 'alternate') {
    suggestionText.value = '';
    suggestionItems.value = [];
    return;
  }
  if (!cursorIsAtInputEnd()) {
    suggestionText.value = '';
    suggestionItems.value = [];
    return;
  }
  const current = currentInput();
  const candidates = suggestionPopup.value
    ? findFuzzySuggestions(current, currentCwd, props.shellKind)
    : findSuggestions(current, currentCwd, props.shellKind);
  suggestionItems.value = candidates
    // A different cwd lowers the rank; it must not hide otherwise useful
    // global history. Template definitions remain discoverable only when the
    // command is concrete, since terminal input cannot fill {{params}}.
    .filter((row) => !(row.source === 'template' && /\{\{[^}]+\}\}/.test(row.command_line)))
    .slice(0, 8);
  if (current !== lastSuggestionQuery) suggestionIndex.value = 0;
  lastSuggestionQuery = current;
  // The ghost text is what ↑ would recall first: the newest entry of this
  // shell's history with the typed prefix (zsh-autosuggestions "history"
  // strategy). Other shells and saved commands are only a fallback.
  if (current !== ghostQuery) {
    ghostQuery = current;
    ghostIndex = 0;
    ghostCandidates = current.trim()
      ? findHistoryEntries(current, props.shellKind).filter((row) => row.command_line !== current)
      : [];
  }
  if (ghostCandidates.length && !suggestionPopup.value) {
    showGhost(current);
    return;
  }
  const match = suggestionItems.value.find((row) => row.command_line.startsWith(current) && row.command_line !== current);
  suggestionText.value = match ? match.command_line.slice(current.length) : '';
  updateSuggestionPosition();
};

const cursorIsAtInputEnd = () => {
  if (!term) return false;
  const buffer = term.buffer.active;
  const cursorLine = buffer.baseY + buffer.cursorY;
  const currentLine = buffer.getLine(cursorLine);
  if (!currentLine) return false;

  const rightPromptBoundary = cursorLine === promptMarker?.line
    && rightPromptStartColumn !== undefined
    && buffer.cursorX <= rightPromptStartColumn
    ? rightPromptStartColumn
    : currentLine.length;
  for (let column = buffer.cursorX; column < rightPromptBoundary; column += 1) {
    // Prompts such as oh-my-posh pad the gap before a right prompt with real
    // space cells; only visible characters count as input after the cursor.
    if (/\S/.test(currentLine.getCell(column)?.getChars() || '')) return false;
  }
  // A wrapped command can continue on later buffer lines. Treat the cursor as
  // being at the end only when there is no input content after it anywhere in
  // the active buffer.
  for (let line = cursorLine + 1; line < buffer.length; line += 1) {
    if (buffer.getLine(line)?.translateToString(true)) return false;
  }
  return true;
};

const findRightPromptStartColumn = (lineNumber: number | undefined, fromColumn: number) => {
  if (!term || lineNumber === undefined) return undefined;
  const line = term.buffer.active.getLine(lineNumber);
  if (!line) return undefined;
  for (let column = fromColumn; column < line.length; column += 1) {
    const chars = line.getCell(column)?.getChars() || '';
    if (/\S/.test(chars)) return column;
  }
  return undefined;
};

const updateSuggestionPosition = () => {
  if (!term || !terminalElement.value || !promptMarker) return;
  const screen = term.element?.querySelector<HTMLElement>('.xterm-screen');
  if (!screen || term.cols <= 0 || term.rows <= 0) return;

  const activeBuffer = term.buffer.active;
  const viewportY = activeBuffer.viewportY;
  const cursorLine = activeBuffer.baseY + activeBuffer.cursorY;
  const row = cursorLine - viewportY;
  if (row < 0 || row >= term.rows) return;

  const screenRect = screen.getBoundingClientRect();
  const containerRect = terminalElement.value.getBoundingClientRect();

  const cellDimensions = (term as any)._core?._renderService?.dimensions?.css?.cell;
  const cellWidth = cellDimensions?.width || (screen.clientWidth / term.cols);
  const cellHeight = cellDimensions?.height || (screen.clientHeight / term.rows);

  const left = screenRect.left - containerRect.left + activeBuffer.cursorX * cellWidth;
  const top = screenRect.top - containerRect.top + row * cellHeight;
  if (
    cursorLine === promptMarker?.line
    && rightPromptStartColumn !== undefined
    && activeBuffer.cursorX <= rightPromptStartColumn
  ) {
    const availableColumns = Math.max(0, rightPromptStartColumn - activeBuffer.cursorX);
    suggestionPosition.value = {
      left: `${left}px`,
      top: `${top}px`,
      height: `${cellHeight}px`,
      lineHeight: `${cellHeight}px`,
      maxWidth: `${availableColumns * cellWidth}px`,
    };
  } else {
    suggestionPosition.value = {
      left: `${left}px`,
      top: `${top}px`,
      height: `${cellHeight}px`,
      lineHeight: `${cellHeight}px`,
      maxWidth: `${Math.max(0, term.cols - activeBuffer.cursorX) * cellWidth}px`,
    };
  }
  popupPosition.value = { left: `${left}px`, top: `${top}px` };
};

const acceptSuggestion = (commandLine: string) => {
  const current = currentInput();
  const remainder = commandLine.startsWith(current)
    ? commandLine.slice(current.length)
    : `\x7f`.repeat(Array.from(current).length) + commandLine;
  if (!remainder) return;
  if (props.historyLevel === 2) inputShadow = commandLine;
  void sendInput(props.commandId, remainder, props.runEventId);
  suggestionText.value = '';
  suggestionPopup.value = false;
  resetGhostCycle();
};

const handleClear = () => {
  term?.clear();
};

const focusTerminal = () => {
  term?.focus();
  term?.write('\x1b[?25h');
};

const handleReattach = async () => {
  try {
    const data = await reattachBuffer(props.commandId, props.runEventId);
    if (term && data) {
      term.clear();
      term.write(data);
      term.write('\x1b[?25h');
    }
    terminalError.value = '';
  } catch (error) {
    terminalError.value = String(error);
  }
};
</script>

<style scoped>
.terminal-error {
  padding: 8px;
  color: #f87171;
  overflow-wrap: anywhere;
}

.xterm-pane-container {
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  background-color: var(--bg-terminal);
  overflow: hidden;
}

.pane-toolbar {
  height: 34px;
  background-color: var(--bg-surface);
  border-bottom: 1px solid var(--border-subtle);
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 10px;
  flex-shrink: 0;
}

.toolbar-left, .toolbar-right {
  display: flex;
  align-items: center;
  gap: 8px;
}

.cmd-badge {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 11.5px;
  font-weight: 600;
  color: var(--text-primary);
}

.status-dot {
  width: 7px;
  height: 7px;
  border-radius: 9999px;
  background-color: var(--status-idle);
}

.status-dot.active {
  background-color: var(--status-running);
  box-shadow: 0 0 6px var(--status-running);
}

.pid-tag {
  background-color: var(--bg-app-base);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  padding: 1px 5px;
  font-size: 10px;
  font-family: var(--font-mono);
  color: var(--text-secondary);
}

.exit-tag {
  padding: 1px 5px;
  border-radius: var(--radius-sm);
  font-size: 10px;
  font-family: var(--font-mono);
  font-weight: 600;
}

.exit-tag.success {
  background: var(--status-running-bg);
  color: var(--status-running);
}

.exit-tag.error {
  background: var(--status-failed-bg);
  color: var(--status-failed);
}

.terminal-viewport {
  flex: 1;
  padding: 8px;
  overflow: hidden;
  background-color: var(--bg-terminal);
  position: relative;
}

.ghost-suggestion {
  position: absolute;
  z-index: 2;
  color: rgba(148, 163, 184, 0.55);
  pointer-events: none;
  white-space: pre;
  max-width: 100%;
  overflow: hidden;
  display: flex;
  align-items: center;
}

.suggestion-popup {
  position: absolute;
  z-index: 3;
  min-width: 360px;
  max-width: min(80%, 680px);
  padding: 4px;
  background: #171b27;
  border: 1px solid var(--border-subtle);
  border-radius: 6px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, .35);
  transform: translateY(calc(-100% - 4px));
}

.suggestion-item {
  display: flex;
  justify-content: space-between;
  gap: 16px;
  width: 100%;
  padding: 6px 8px;
  color: var(--text-primary);
  background: transparent;
  border: 0;
  text-align: left;
  font: 12px var(--font-mono);
  cursor: pointer;
}

.suggestion-item:hover { background: var(--bg-surface-hover); }
.suggestion-item.selected { background: var(--bg-surface-hover); }
.suggestion-item small { color: var(--text-muted); white-space: nowrap; }
.suggestion-empty { display: block; padding: 8px; color: var(--text-muted); font-size: 12px; }

.spin {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

.terminal-link-tooltip {
  position: fixed;
  z-index: 9999;
  pointer-events: none;
  background: #171b27;
  border: 1px solid var(--border-subtle, rgba(255, 255, 255, 0.12));
  backdrop-filter: blur(10px);
  color: #f1f5f9;
  padding: 4px 10px;
  border-radius: 6px;
  font-size: 11.5px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.45);
  white-space: nowrap;
  max-width: 450px;
  overflow: hidden;
  text-overflow: ellipsis;
  display: flex;
  align-items: center;
  gap: 6px;
  transform: translate(-50%, -100%);
  transition: border-color 0.15s ease, background-color 0.15s ease;
  user-select: none;
}

.terminal-link-tooltip.ctrl-active {
  border-color: #a855f7;
  background: #1e1932;
}

.terminal-link-tooltip.ctrl-needed {
  border-color: #f59e0b;
  background: #271f14;
  animation: pulse-border 0.3s ease;
}

.key-badge {
  display: inline-block;
  padding: 1px 5px;
  font-size: 10px;
  font-family: inherit;
  font-weight: 600;
  background: rgba(255, 255, 255, 0.1);
  border: 1px solid rgba(255, 255, 255, 0.2);
  border-radius: 3px;
  color: #e2e8f0;
}

.key-badge.warning {
  background: rgba(245, 158, 11, 0.2);
  border-color: #f59e0b;
  color: #fbbf24;
}

.link-url {
  color: #38bdf8;
  text-decoration: underline;
  max-width: 250px;
  overflow: hidden;
  text-overflow: ellipsis;
  display: inline-block;
  vertical-align: bottom;
}

@keyframes pulse-border {
  0% { transform: translate(-50%, -100%) scale(0.96); }
  50% { transform: translate(-50%, -100%) scale(1.03); }
  100% { transform: translate(-50%, -100%) scale(1); }
}
</style>
