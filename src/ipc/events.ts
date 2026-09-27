/**
 * Tauri event names and payloads (match src-tauri/src/ipc/events.rs).
 */

export const IPC_EVENTS = {
  PTY_DATA: 'pty://data',
  PROCESS_STATUS: 'process://status',
  SHUTDOWN_PROGRESS: 'app://shutdown-progress',
  SINGLE_INSTANCE: 'app://instance',
  HISTORY_ADDED: 'history://added',
  CLOSE_REQUESTED: 'app://close-requested',
} as const;

export interface PtyDataEvent {
  run_event_id: string;
  b64: string;
}

export interface ProcessStatusEvent {
  run_event_id: string;
  command_id: string;
  session_id: string;
  status: string;
  exit_code?: number | null;
  pid?: number | null;
}

export interface SingleInstanceEvent {
  timestamp: string;
  focusWindow: boolean;
}

export interface ShutdownProgressEvent {
  remaining_secs: number;
  timeout_secs: number;
  phase: 'stopping' | 'forcing';
}

export interface HistoryAddedEvent {
  id: string;
}
