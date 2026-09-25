/**
 * Tauri event names (match src-tauri/src/ipc/events.rs).
 * Live payloads still differ (run_event_id + b64 vs commandId + string) until IPC is wired.
 */

export const IPC_EVENTS = {
  PTY_DATA: 'pty://data',
  PROCESS_STATUS: 'process://status',
  SHUTDOWN_PROGRESS: 'app://shutdown-progress',
  SINGLE_INSTANCE: 'app://instance',
} as const;

export interface PtyDataEvent {
  commandId: number;
  data: string;
}

export interface ProcessStatusEvent {
  commandId: number;
  status: 'idle' | 'starting' | 'running' | 'completed' | 'failed' | 'stopped';
  pid?: number;
  exitCode?: number;
  sessionId?: number;
}

export interface SingleInstanceEvent {
  timestamp: string;
  focusWindow: boolean;
}
