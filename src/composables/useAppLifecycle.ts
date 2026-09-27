import { ref } from 'vue';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { IPC_EVENTS, type ShutdownProgressEvent } from '@/ipc/events';

const isShuttingDown = ref(false);
const showCloseConfirm = ref(false);
const shutdownCountdown = ref(8);
const shutdownTimeout = ref(8);
const shutdownPhase = ref<'stopping' | 'forcing'>('stopping');
const shutdownError = ref('');
const showSingleInstanceAlert = ref(false);
let progressListener: Promise<UnlistenFn> | undefined;
let closeRequestListener: Promise<UnlistenFn> | undefined;
let requestPending = false;
const isTauri = () => typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

const requestClose = () => {
  if (isShuttingDown.value) return;
  showCloseConfirm.value = true;
};

const cancelClose = () => {
  showCloseConfirm.value = false;
};

const confirmClose = () => {
  showCloseConfirm.value = false;
  void requestShutdown(false);
};

const initLifecycleListener = async () => {
  if (!isTauri()) return;
  progressListener ??= import('@tauri-apps/api/event').then(({ listen }) =>
    listen<ShutdownProgressEvent>(IPC_EVENTS.SHUTDOWN_PROGRESS, ({ payload }) => {
      isShuttingDown.value = true;
      shutdownCountdown.value = payload.remaining_secs;
      shutdownTimeout.value = payload.timeout_secs;
      shutdownPhase.value = payload.phase;
      shutdownError.value = '';
    }),
  ).catch((error) => {
    progressListener = undefined;
    throw error;
  });

  closeRequestListener ??= import('@tauri-apps/api/event').then(({ listen }) =>
    listen(IPC_EVENTS.CLOSE_REQUESTED, () => {
      requestClose();
    }),
  ).catch((error) => {
    closeRequestListener = undefined;
    throw error;
  });

  await Promise.all([progressListener, closeRequestListener]);
};

const disposeLifecycleListener = () => {
  const pListener = progressListener;
  progressListener = undefined;
  void pListener?.then((unlisten) => unlisten()).catch(console.error);

  const cListener = closeRequestListener;
  closeRequestListener = undefined;
  void cListener?.then((unlisten) => unlisten()).catch(console.error);
};

const requestShutdown = async (force: boolean) => {
  if (requestPending) return;
  // Browser preview has no native application to exit.
  if (!isTauri()) return;
  requestPending = true;
  isShuttingDown.value = true;
  shutdownError.value = '';
  try {
    // An unavailable event listener must not prevent the backend from closing the app.
    await initLifecycleListener().catch(console.error);
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('app_shutdown', { force });
  } catch (error) {
    shutdownError.value = `Không thể đóng ứng dụng: ${String(error)}. Hãy thử lại.`;
  } finally {
    requestPending = false;
  }
};

export function useAppLifecycle() {
  return {
    isShuttingDown,
    showCloseConfirm,
    shutdownCountdown,
    shutdownTimeout,
    shutdownPhase,
    shutdownError,
    showSingleInstanceAlert,
    requestClose,
    cancelClose,
    confirmClose,
    initLifecycleListener,
    disposeLifecycleListener,
    triggerGracefulShutdown: () => requestShutdown(false),
    forceExitApp: () => requestShutdown(true),
  };
}
