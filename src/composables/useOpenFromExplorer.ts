import type { Router } from 'vue-router';
import { ipcClient } from '@/ipc/client';

/** "C:\Users\hoa\dự án" → "dự án"; a drive stays "C:\". */
const folderName = (path: string) => {
  const trimmed = path.replace(/[\\/]+$/, '');
  const name = trimmed.split(/[\\/]/).pop() ?? '';
  return name && !/^[A-Za-z]:$/.test(name) ? name : path;
};

/** Resolves once the workspace has taken the last terminal request (or after 3 s). */
const requestTaken = (router: Router) =>
  new Promise<void>((resolve) => {
    const started = Date.now();
    const check = () => {
      if (!router.currentRoute.value.query.runEventId || Date.now() - started > 3000) resolve();
      else window.setTimeout(check, 100);
    };
    check();
  });

let busy = false;

/**
 * File Explorer's "Open in Command Manager": opens a terminal tab in each
 * folder the backend holds, at startup and when a running app is asked.
 */
export function useOpenFromExplorer(router: Router) {
  const openPending = async () => {
    if (busy) return;
    busy = true;
    try {
      for (;;) {
        const dirs = await ipcClient.takeOpenDirs();
        if (dirs.length === 0) break;
        for (const dir of dirs) {
          try {
            const terminal = await ipcClient.openTerminal(dir);
            await router.push({
              path: '/workspace',
              query: {
                runEventId: terminal.runEventId,
                runCommandId: String(terminal.commandId),
                runCommandName: folderName(dir),
              },
            });
            await requestTaken(router);
          } catch (error) {
            console.error('Không mở được terminal cho', dir, error);
          }
        }
      }
    } finally {
      busy = false;
    }
  };

  const listen = async () => {
    const stop = await ipcClient.onOpenDir(() => void openPending());
    // Folders from the command line of this launch, or asked before the listener.
    void openPending();
    return stop;
  };

  return { listen };
}
