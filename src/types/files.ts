/**
 * Files pane and transfers (SCR-08). Mirrors src-tauri/src/files and
 * src-tauri/src/ipc/files.rs.
 */

export type FileLocation = { kind: 'local' } | { kind: 'ssh'; host_id: string };

export type EntryKind = 'dir' | 'file' | 'symlink' | 'other';

export interface FsEntry {
  name: string;
  path: string;
  kind: EntryKind;
  size: number;
  /** Unix seconds. */
  mtime?: number | null;
  mode?: number | null;
  link_target?: string | null;
  link_is_dir: boolean;
  hidden: boolean;
}

export interface FsListing {
  path: string;
  parent?: string | null;
  entries: FsEntry[];
}

export interface Endpoint {
  location: FileLocation;
  path: string;
}

export type TransferMode = 'copy' | 'move';
export type ConflictAction = 'overwrite' | 'overwrite_if_newer' | 'skip' | 'keep_both' | 'cancel_job';
export type JobStatus = 'scanning' | 'running' | 'done' | 'failed' | 'cancelled';
export type ItemStatus = 'pending' | 'running' | 'done' | 'skipped' | 'failed' | 'cancelled';

export interface ItemSnapshot {
  id: number;
  name: string;
  is_dir: boolean;
  size: number;
  done_bytes: number;
  status: ItemStatus;
  error?: string | null;
}

export interface JobSnapshot {
  id: string;
  mode: TransferMode;
  sources: Endpoint[];
  dest: Endpoint;
  status: JobStatus;
  total_bytes: number;
  done_bytes: number;
  files_total: number;
  files_done: number;
  files_skipped: number;
  files_failed: number;
  bytes_per_sec: number;
  started_at: number;
  finished_at?: number | null;
  error?: string | null;
  warnings: string[];
  /** Progress events carry only running items; done / get carry all. */
  items: ItemSnapshot[];
  items_truncated: boolean;
  /** Failed while a server was unreachable; the UI retries with a backoff. */
  connection_lost: boolean;
}

export interface ConflictPrompt {
  job_id: string;
  item_id: number;
  path: string;
  source_size: number;
  source_mtime?: number | null;
  target_size: number;
  target_mtime?: number | null;
  kind_mismatch: boolean;
  suggested: ConflictAction;
  keep_both_name: string;
}

export type EditStatus = 'opened' | 'uploading' | 'synced' | 'conflict' | 'error' | 'stopped';

/** A server file open in a local app, uploaded back on save. */
export interface EditInfo {
  local_path: string;
  location: FileLocation;
  remote_path: string;
  status: EditStatus;
  error?: string | null;
  synced_at?: number | null;
  uploads: number;
}

export const sameLocation = (a: FileLocation, b: FileLocation): boolean =>
  a.kind === b.kind && (a.kind === 'local' || (b.kind === 'ssh' && a.host_id === b.host_id));

export const formatBytes = (bytes: number): string => {
  if (!Number.isFinite(bytes) || bytes < 1024) return `${Math.max(0, Math.round(bytes || 0))} B`;
  const units = ['KB', 'MB', 'GB', 'TB'];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(value >= 100 ? 0 : 1)} ${units[unit]}`;
};
