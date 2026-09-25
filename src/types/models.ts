/**
 * Data models matching SQLite schema & in-memory state definitions
 * Reference: docs/plan.md, docs/checklist.md, docs/structure.md
 */

export interface CommandDefinition {
  id: number;
  name: string;
  execution_string: string;
  is_shell: boolean;
}

export interface CommandGroup {
  id: number;
  group_name: string;
  autostart: boolean;
}

export interface GroupMembership {
  group_id: number;
  command_id: number;
  execution_order: number;
}

export interface CommandGroupWithCommands extends CommandGroup {
  commands: (CommandDefinition & { execution_order: number })[];
}

export type RunSessionStatus = 'running' | 'completed' | 'failed' | 'stopped';

export interface RunSession {
  id: number;
  group_id: number;
  group_name?: string;
  started_at: string;
  ended_at?: string;
  status: RunSessionStatus;
}

export type ProcessLifecycleStatus = 'idle' | 'starting' | 'running' | 'completed' | 'failed' | 'stopped';

export interface RunEvent {
  id: number;
  session_id: number;
  command_id: number;
  command_name?: string;
  started_at: string;
  ended_at?: string;
  status: ProcessLifecycleStatus;
  exit_code?: number | null;
  pid?: number | null;
}

export interface ActiveProcessInfo {
  commandId: number;
  commandName: string;
  pid?: number;
  status: ProcessLifecycleStatus;
  startedAt?: string;
  exitCode?: number | null;
  sessionId?: number;
}

export interface SystemSettings {
  autostartApp: boolean;
  ringBufferSizeBytes: number; // e.g. 1048576 (1MB) or 2097152 (2MB)
  shutdownTimeoutSec: number;  // 5..10 seconds
  fontSize: number;
  fontFamily: string;
}

export interface BackupIntegrityResult {
  valid: boolean;
  integrityOk: boolean;
  schemaVersion: string;
  commandCount: number;
  groupCount: number;
  historyCount: number;
  message?: string;
}

export interface ShutdownProgressPayload {
  activePids: number[];
  remainingProcesses: number;
  timeoutSeconds: number;
  elapsedSeconds: number;
  status: 'graceful' | 'force_killing' | 'done';
}
