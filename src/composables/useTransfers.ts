import { computed, ref } from 'vue';
import { ipcClient } from '@/ipc/client';
import type {
  ConflictAction,
  ConflictPrompt,
  Endpoint,
  JobSnapshot,
  TransferMode,
} from '@/types/files';

// One store for the whole app: the Transfers drawer, the status bar
// indicator, MOD-20 and every Files pane read from it.
const jobs = ref<JobSnapshot[]>([]);
const conflicts = ref<ConflictPrompt[]>([]);
const drawerOpen = ref(false);
/** Bumped when a job finishes, so Files panes showing its target refresh. */
const finishedTick = ref(0);
const lastFinished = ref<JobSnapshot | null>(null);
/** One short message at a time, shown by the Transfers toast. */
const notice = ref<{ id: number; kind: 'info' | 'error'; text: string } | null>(null);
let noticeTimer: number | undefined;

const notify = (text: string, kind: 'info' | 'error' = 'info') => {
  notice.value = { id: Date.now(), kind, text };
  window.clearTimeout(noticeTimer);
  noticeTimer = window.setTimeout(() => (notice.value = null), kind === 'error' ? 6000 : 3500);
};

const doneMessage = (job: JobSnapshot): { text: string; kind: 'info' | 'error' } => {
  const verb = job.mode === 'move' ? 'Đã di chuyển' : 'Đã chép';
  if (job.status === 'cancelled') return { text: 'Đã hủy lượt truyền', kind: 'info' };
  if (job.error) return { text: `Truyền thất bại: ${job.error}`, kind: 'error' };
  if (job.files_failed > 0) {
    return { text: `${job.files_failed} tệp lỗi, mở ⇅ để xem chi tiết`, kind: 'error' };
  }
  const skipped = job.files_skipped > 0 ? ` (bỏ qua ${job.files_skipped})` : '';
  return { text: `${verb} ${job.files_done} tệp${skipped}`, kind: 'info' };
};
let listening: Promise<() => void> | null = null;
const waiters = new Map<string, ((job: JobSnapshot) => void)[]>();

/** A dropped connection: try again after 3s, 10s, 30s, then leave it to the user. */
const RETRY_DELAYS = [3000, 10000, 30000];
const retryAttempts = new Map<string, number>();

const finished = (job: JobSnapshot) => job.status !== 'running' && job.status !== 'scanning';

const upsert = (job: JobSnapshot) => {
  const index = jobs.value.findIndex((existing) => existing.id === job.id);
  if (index === -1) {
    jobs.value = [job, ...jobs.value];
    return;
  }
  const previous = jobs.value[index];
  const next = [...jobs.value];
  // Progress events carry only running items: keep the full list from the
  // last full snapshot and refresh the running ones.
  if (job.status === 'scanning' || job.status === 'running') {
    const running = new Map(job.items.map((item) => [item.id, item]));
    const items = previous.items.length
      ? previous.items.map((item) =>
          running.get(item.id) ?? item
        )
      : job.items;
    for (const item of job.items) {
      if (!items.some((existing) => existing.id === item.id)) items.push(item);
    }
    next[index] = { ...job, items };
  } else {
    next[index] = job;
  }
  jobs.value = next;
};

export function useTransfers() {
  /** Subscribes once; AppShell calls it on mount. */
  const listen = () => {
    if (listening) return listening;
    listening = (async () => {
      try {
        jobs.value = await ipcClient.transferList();
      } catch (e) {
        console.error('[useTransfers] list failed:', e);
      }
      return ipcClient.onTransferEvents({
        progress: upsert,
        conflict: (prompt) => {
          conflicts.value = [...conflicts.value, prompt];
        },
        done: (job) => {
          upsert(job);
          const pending = waiters.get(job.id);
          waiters.delete(job.id);
          pending?.forEach((resolve) => resolve(job));
          if (job.connection_lost) {
            const attempt = retryAttempts.get(job.id) ?? 0;
            if (attempt < RETRY_DELAYS.length) {
              retryAttempts.set(job.id, attempt + 1);
              const delay = RETRY_DELAYS[attempt];
              notify(`Mất kết nối khi truyền; tự thử lại sau ${delay / 1000}s (lần ${attempt + 1}/${RETRY_DELAYS.length})`, 'error');
              window.setTimeout(() => {
                void ipcClient
                  .transferRetry(job.id)
                  .then(() => ipcClient.transferGet(job.id))
                  .then(upsert)
                  .catch((error) => notify(`Không thử lại được: ${String(error)}`, 'error'));
              }, delay);
              return;
            }
          } else if (job.status === 'done') {
            retryAttempts.delete(job.id);
          }
          conflicts.value = conflicts.value.filter((prompt) => prompt.job_id !== job.id);
          lastFinished.value = job;
          finishedTick.value += 1;
          if (!drawerOpen.value) {
            const message = doneMessage(job);
            notify(message.text, message.kind);
          }
        },
      });
    })();
    return listening;
  };

  const start = async (sources: Endpoint[], dest: Endpoint, mode: TransferMode) => {
    const id = await ipcClient.transferStart(sources, dest, mode);
    try {
      upsert(await ipcClient.transferGet(id));
    } catch {
      // The progress event will bring it.
    }
    return id;
  };

  /** Resolves when the job finishes (immediately if it already has). */
  const whenFinished = (id: string): Promise<JobSnapshot> => {
    const known = jobs.value.find((job) => job.id === id);
    if (known && finished(known)) return Promise.resolve(known);
    return new Promise((resolve) => {
      waiters.set(id, [...(waiters.get(id) ?? []), resolve]);
    });
  };

  const cancel = (id: string) => {
    // A user cancel also stops the automatic retries.
    retryAttempts.set(id, RETRY_DELAYS.length);
    return ipcClient.transferCancel(id);
  };

  const retry = async (id: string) => {
    retryAttempts.delete(id);
    await ipcClient.transferRetry(id);
    upsert(await ipcClient.transferGet(id));
  };

  const clearFinished = async () => {
    await ipcClient.transferClearFinished();
    jobs.value = jobs.value.filter((job) => job.status === 'scanning' || job.status === 'running');
  };

  /** Full item list (progress events only carry the running ones). */
  const loadDetails = async (id: string) => {
    upsert(await ipcClient.transferGet(id));
  };

  const resolveConflict = async (action: ConflictAction, applyToRest: boolean) => {
    const prompt = conflicts.value[0];
    if (!prompt) return;
    conflicts.value = conflicts.value.slice(1);
    await ipcClient.transferResolve(prompt.job_id, prompt.item_id, action, applyToRest);
  };

  const activeJobs = computed(() =>
    jobs.value.filter((job) => job.status === 'scanning' || job.status === 'running')
  );
  const activeTotals = computed(() => {
    const active = activeJobs.value;
    const total = active.reduce((sum, job) => sum + job.total_bytes, 0);
    const done = active.reduce((sum, job) => sum + job.done_bytes, 0);
    return {
      count: active.length,
      percent: total > 0 ? Math.min(100, Math.round((done / total) * 100)) : 0,
      bytesPerSec: active.reduce((sum, job) => sum + job.bytes_per_sec, 0),
    };
  });

  return {
    jobs,
    conflicts,
    drawerOpen,
    finishedTick,
    lastFinished,
    notice,
    notify,
    activeJobs,
    activeTotals,
    listen,
    start,
    whenFinished,
    cancel,
    retry,
    clearFinished,
    loadDetails,
    resolveConflict,
    toggleDrawer: () => {
      drawerOpen.value = !drawerOpen.value;
    },
  };
}
