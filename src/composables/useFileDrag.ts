import { ref } from 'vue';
import type { Endpoint, FileLocation } from '@/types/files';
import { sameLocation } from '@/types/files';
import { ipcClient } from '@/ipc/client';
import { formatDroppedPaths, hasControlCharacters } from './useTerminalDrop';
import { useTransfers } from './useTransfers';

/**
 * Dragging files between Files panes and onto terminals, and dropping
 * files from the OS.
 *
 * In-app drags use pointer events, like tab drags: with Tauri's native
 * drop handling on (needed to receive files from the OS), HTML5 drag and
 * drop does not fire inside the webview on Windows.
 *
 * Targets are found with `elementsFromPoint`: a Files pane marks itself
 * with `data-files-pane-id` (folder rows with `data-drop-dir`), a terminal
 * tab with `data-terminal-tab-id`.
 */

export interface DraggedEntry {
  path: string;
  name: string;
  isDir: boolean;
  size: number;
}

/** What DockHost tells us about a terminal tab under the pointer. */
export interface TerminalDropInfo {
  tabId: string;
  /** Set for SSH terminals. */
  hostId?: string;
  /** Folder the shell reported (OSC 633 / OSC 7), if any. */
  cwd?: string;
  shellKind?: string;
  label: string;
  running: boolean;
  paste: (text: string) => boolean;
}

export type DropTarget =
  | { kind: 'files'; paneId: string; location: FileLocation; dir: string }
  | { kind: 'terminal'; tabId: string; info: TerminalDropInfo };

interface PaneInfo {
  location: () => FileLocation;
  currentDir: () => string;
}

interface DragState {
  location: FileLocation;
  entries: DraggedEntry[];
  startX: number;
  startY: number;
  x: number;
  y: number;
  dragging: boolean;
  move: boolean;
  target: DropTarget | null;
  invalid: string | null;
}

interface OsDragState {
  x: number;
  y: number;
  count: number;
  target: DropTarget | null;
  invalid: string | null;
}

const panes = new Map<string, PaneInfo>();
let terminalResolver: ((tabId: string) => TerminalDropInfo | null) | null = null;
const drag = ref<DragState | null>(null);
const osDrag = ref<OsDragState | null>(null);

const targetAt = (x: number, y: number): DropTarget | null => {
  for (const element of document.elementsFromPoint(x, y)) {
    if (element.closest('.file-drag-ghost')) continue;
    const paneEl = element.closest<HTMLElement>('[data-files-pane-id]');
    if (paneEl) {
      const paneId = paneEl.dataset.filesPaneId!;
      const pane = panes.get(paneId);
      if (!pane) return null;
      const row = element.closest<HTMLElement>('[data-drop-dir]');
      return {
        kind: 'files',
        paneId,
        location: pane.location(),
        dir: row?.dataset.dropDir || pane.currentDir(),
      };
    }
    const terminalEl = element.closest<HTMLElement>('[data-terminal-tab-id]');
    if (terminalEl) {
      const info = terminalResolver?.(terminalEl.dataset.terminalTabId!);
      return info ? { kind: 'terminal', tabId: info.tabId, info } : null;
    }
  }
  return null;
};

const isWindowsClient = () => navigator.userAgent.includes('Windows');

const separatorOf = (location: FileLocation) =>
  location.kind === 'ssh' || !isWindowsClient() ? '/' : '\\';

const parentOf = (location: FileLocation, path: string) => {
  const sep = separatorOf(location);
  const trimmed = path.endsWith(sep) && path.length > 1 ? path.slice(0, -1) : path;
  const index = trimmed.lastIndexOf(sep);
  return index <= 0 ? sep : trimmed.slice(0, index);
};

const baseName = (path: string) => path.split(/[\\/]/).filter(Boolean).pop() ?? path;

const within = (location: FileLocation, child: string, dir: string) => {
  const sep = separatorOf(location);
  const base = dir.endsWith(sep) ? dir.slice(0, -1) : dir;
  return child === base || child.startsWith(base + sep);
};

/** Quotes a path for a POSIX shell (remote servers). */
const posixQuote = (path: string) =>
  /^[A-Za-z0-9_./~@%+=:,-]+$/.test(path) ? path : `'${path.replace(/'/g, `'\\''`)}'`;

const pasteRemotePaths = (info: TerminalDropInfo, paths: string[]) => {
  const safe = paths.filter((path) => !hasControlCharacters(path));
  if (safe.length > 0) info.paste(safe.map(posixQuote).join(' ') + ' ');
};

/** Why dropping `location` items on `target` makes no sense, or null. */
const whyInvalid = (
  location: FileLocation,
  paths: string[],
  target: DropTarget
): string | null => {
  if (target.kind === 'terminal') {
    if (!target.info.running) return 'Terminal đã dừng';
    if (!target.info.hostId && location.kind === 'ssh') {
      return 'Kéo tệp trên máy chủ vào terminal local chưa được hỗ trợ; hãy thả vào Files pane local';
    }
    return null;
  }
  if (!sameLocation(location, target.location)) return null;
  if (paths.some((path) => within(target.location, target.dir, path))) {
    return 'Không thể thả một thư mục vào chính nó';
  }
  if (paths.every((path) => parentOf(target.location, path) === target.dir)) {
    return 'Mục đã nằm trong thư mục này';
  }
  return null;
};

const errorText = (error: unknown) => (error instanceof Error ? error.message : String(error));

/**
 * Terminal drops:
 * - local terminal ← local files: insert the quoted paths;
 * - SSH terminal ← files already on that server: insert their paths;
 * - SSH terminal ← anything else: upload to the shell's folder (or home),
 *   then insert the uploaded paths.
 */
const dropOnTerminal = async (info: TerminalDropInfo, location: FileLocation, paths: string[]) => {
  const transfers = useTransfers();
  if (!info.hostId) {
    const { text, skipped } = formatDroppedPaths(paths, info.shellKind);
    if (text) info.paste(text);
    if (skipped.length > 0) transfers.notify(`Bỏ qua ${skipped.length} đường dẫn có ký tự điều khiển`, 'error');
    return;
  }
  if (location.kind === 'ssh' && location.host_id === info.hostId) {
    pasteRemotePaths(info, paths);
    return;
  }
  const remote: FileLocation = { kind: 'ssh', host_id: info.hostId };
  const dir = info.cwd || (await ipcClient.fsHome(remote));
  const sources = paths.map((path) => ({ location, path }));
  const id = await transfers.start(sources, { location: remote, path: dir }, 'copy');
  transfers.notify(`Đang tải ${paths.length} mục lên ${dir}…`);
  const job = await transfers.whenFinished(id);
  if (job.status === 'done' && job.files_failed === 0) {
    const sep = dir.endsWith('/') ? '' : '/';
    pasteRemotePaths(
      info,
      paths.map((path) => `${dir}${sep}${baseName(path)}`)
    );
  } else if (job.status !== 'cancelled') {
    transfers.notify('Tải lên chưa xong nên chưa chèn đường dẫn; xem ⇅ để biết chi tiết', 'error');
  }
};

const dropOn = (target: DropTarget, location: FileLocation, paths: string[], move: boolean) => {
  const transfers = useTransfers();
  const run =
    target.kind === 'terminal'
      ? dropOnTerminal(target.info, location, paths)
      : transfers.start(
          paths.map((path): Endpoint => ({ location, path })),
          { location: target.location, path: target.dir },
          move ? 'move' : 'copy'
        );
  void Promise.resolve(run).catch((error) => transfers.notify(errorText(error), 'error'));
};

const endDrag = () => {
  window.removeEventListener('pointermove', onMove, true);
  window.removeEventListener('pointerup', onUp, true);
  window.removeEventListener('keydown', onKey, true);
  drag.value = null;
};

function onMove(event: PointerEvent) {
  const state = drag.value;
  if (!state) return;
  if (!state.dragging && Math.hypot(event.clientX - state.startX, event.clientY - state.startY) < 4) {
    return;
  }
  state.dragging = true;
  state.x = event.clientX;
  state.y = event.clientY;
  state.move = event.shiftKey;
  state.target = targetAt(event.clientX, event.clientY);
  state.invalid = state.target
    ? whyInvalid(
        state.location,
        state.entries.map((entry) => entry.path),
        state.target
      )
    : null;
}

function onUp(event: PointerEvent) {
  const state = drag.value;
  endDrag();
  if (!state?.dragging || !state.target || state.invalid) return;
  event.preventDefault();
  event.stopPropagation();
  dropOn(
    state.target,
    state.location,
    state.entries.map((entry) => entry.path),
    event.shiftKey
  );
}

function onKey(event: KeyboardEvent) {
  if (event.key === 'Escape' && drag.value) {
    event.preventDefault();
    event.stopPropagation();
    endDrag();
  } else if (event.key === 'Shift' && drag.value) {
    drag.value.move = true;
  }
}

let osListening: Promise<() => void> | null = null;

export function useFileDrag() {
  const registerPane = (paneId: string, info: PaneInfo) => {
    panes.set(paneId, info);
    return () => panes.delete(paneId);
  };

  /** DockHost answers "what terminal is this tab?" for drops. */
  const setTerminalResolver = (resolver: typeof terminalResolver) => {
    terminalResolver = resolver;
  };

  /** Call from a row's pointerdown; the drag starts after 4px. */
  const startDrag = (event: PointerEvent, location: FileLocation, entries: DraggedEntry[]) => {
    if (event.button !== 0 || entries.length === 0) return;
    endDrag();
    drag.value = {
      location,
      entries,
      startX: event.clientX,
      startY: event.clientY,
      x: event.clientX,
      y: event.clientY,
      dragging: false,
      move: false,
      target: null,
      invalid: null,
    };
    window.addEventListener('pointermove', onMove, true);
    window.addEventListener('pointerup', onUp, true);
    window.addEventListener('keydown', onKey, true);
  };

  /**
   * Files dragged in from the OS file manager. Once per app (AppShell).
   * Tauri reports physical pixels; the DOM works in CSS pixels.
   */
  const listenOsDrops = () => {
    if (osListening) return osListening;
    osListening = (async () => {
      try {
        const { getCurrentWebview } = await import('@tauri-apps/api/webview');
        let paths: string[] = [];
        return await getCurrentWebview().onDragDropEvent((event) => {
          const payload = event.payload;
          if (payload.type === 'leave') {
            osDrag.value = null;
            return;
          }
          if (payload.type === 'enter') paths = payload.paths;
          const scale = window.devicePixelRatio || 1;
          const x = payload.position.x / scale;
          const y = payload.position.y / scale;
          const target = targetAt(x, y);
          const local: FileLocation = { kind: 'local' };
          if (payload.type === 'drop') {
            osDrag.value = null;
            const dropped = payload.paths.length > 0 ? payload.paths : paths;
            if (!target || dropped.length === 0 || whyInvalid(local, dropped, target)) return;
            dropOn(target, local, dropped, false);
            return;
          }
          osDrag.value = {
            x,
            y,
            count: paths.length,
            target,
            invalid: target ? whyInvalid(local, paths, target) : null,
          };
        });
      } catch (error) {
        console.warn('[useFileDrag] OS drops unavailable:', error);
        return () => {};
      }
    })();
    return osListening;
  };

  return { drag, osDrag, registerPane, setTerminalResolver, startDrag, listenOsDrops };
}
