import { onMounted, onUnmounted } from 'vue';
import { ipcClient } from '@/ipc/client';
import { IPC_EVENTS, type PtyDataEvent } from '@/ipc/events';

type DataListener = (chunk: string) => void;

const listeners = new Map<number, Set<DataListener>>();
let unlistenPty: (() => void) | null = null;

export function usePtyStream() {
  const subscribePty = (commandId: number, callback: DataListener) => {
    if (!listeners.has(commandId)) {
      listeners.set(commandId, new Set());
    }
    listeners.get(commandId)!.add(callback);

    return () => {
      listeners.get(commandId)?.delete(callback);
    };
  };

  const sendInput = async (commandId: number, text: string) => {
    await ipcClient.writePty(commandId, text);
  };

  const resize = async (commandId: number, cols: number, rows: number) => {
    await ipcClient.resizePty(commandId, cols, rows);
  };

  const reattachBuffer = async (commandId: number): Promise<string> => {
    return await ipcClient.reattachPty(commandId);
  };

  // Khởi tạo listener cho Tauri IPC
  const initIpcListener = async () => {
    if (typeof window !== 'undefined' && ('__TAURI_INTERNALS__' in window || '__TAURI__' in window)) {
      try {
        const { listen } = await import('@tauri-apps/api/event');
        unlistenPty = await listen<PtyDataEvent>(IPC_EVENTS.PTY_DATA, (event) => {
          const { commandId, data } = event.payload;
          const set = listeners.get(commandId);
          if (set) {
            set.forEach((cb) => cb(data));
          }
        });
      } catch (err) {
        console.warn('[usePtyStream] Không thể đăng ký Tauri event listener:', err);
      }
    }
  };

  return {
    subscribePty,
    sendInput,
    resize,
    reattachBuffer,
    initIpcListener,
  };
}
