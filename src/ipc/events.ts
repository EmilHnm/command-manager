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
  COMMANDS_CHANGED: 'commands://changed',
  SSH_STATUS: 'ssh://status',
  SSH_HOSTKEY_REQUEST: 'ssh://hostkey-request',
  SSH_AUTH_REQUEST: 'ssh://auth-request',
  TRANSFER_PROGRESS: 'files://transfer-progress',
  TRANSFER_CONFLICT: 'files://transfer-conflict',
  TRANSFER_DONE: 'files://transfer-done',
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

export interface SshHostkeyRequestEvent {
  request_id: string;
  prompt: import('@/types/ssh').HostKeyPrompt;
}

export interface SshAuthRequestEvent {
  request_id: string;
  prompt: import('@/types/ssh').AuthPrompt;
}

export interface SshStatusEventPayload {
  host_id: string;
  state: import('@/types/ssh').SshConnState;
  error?: string | null;
}
