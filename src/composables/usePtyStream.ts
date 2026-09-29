import { ipcClient } from '@/ipc/client';

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

  const sendInput = async (commandId: number, text: string, runEventId?: string) => {
    await ipcClient.writePty(commandId, text, runEventId);
  };

  const resize = async (commandId: number, cols: number, rows: number, runEventId?: string) => {
    await ipcClient.resizePty(commandId, cols, rows, runEventId);
  };

  const reattachBuffer = async (commandId: number, runEventId?: string): Promise<string> => {
    return await ipcClient.reattachPty(commandId, runEventId);
  };

  const initIpcListener = async () => {
    try {
      unlistenPty = await ipcClient.onPtyData((commandId, data) => {
        const set = listeners.get(commandId);
        if (set) set.forEach((cb) => cb(data));
      });
    } catch (err) {
      console.warn('[usePtyStream] Không thể đăng ký Tauri event listener:', err);
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
