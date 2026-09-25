import { ref } from 'vue';
import { useRunSession } from './useRunSession';

const isShuttingDown = ref(false);
const shutdownCountdown = ref(8);
const showSingleInstanceAlert = ref(false);

export function useAppLifecycle() {
  const { activeProcesses, stopCommandProcess } = useRunSession();

  const triggerGracefulShutdown = async () => {
    isShuttingDown.value = true;
    shutdownCountdown.value = 8;

    // Giả lập đếm lùi và gửi tín hiệu dừng tới các tiến trình
    const timer = setInterval(() => {
      if (shutdownCountdown.value > 1) {
        shutdownCountdown.value--;
      } else {
        clearInterval(timer);
        // Force kill nếu hết thời gian
        forceExitApp();
      }
    }, 1000);

    try {
      // Dừng tất cả tiến trình
      for (const [cmdId] of activeProcesses.value) {
        await stopCommandProcess(cmdId, false);
      }
      setTimeout(() => {
        clearInterval(timer);
        forceExitApp();
      }, 2000);
    } catch (e) {
      console.error('Lỗi khi dừng tiến trình:', e);
    }
  };

  const forceExitApp = async () => {
    if (typeof window !== 'undefined' && ('__TAURI_INTERNALS__' in window || '__TAURI__' in window)) {
      try {
        const { getCurrentWindow } = await import('@tauri-apps/api/window');
        await getCurrentWindow().destroy();
      } catch (err) {
        console.warn('Lỗi destroy cửa sổ:', err);
      }
    } else {
      isShuttingDown.value = false;
      console.log('[Dev Web Mode] Đã hoàn thành Graceful Shutdown simulation.');
    }
  };

  return {
    isShuttingDown,
    shutdownCountdown,
    showSingleInstanceAlert,
    triggerGracefulShutdown,
    forceExitApp,
  };
}
