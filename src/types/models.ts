/**
 * Data models matching SQLite schema & in-memory state definitions
 * Reference: docs/plan.md, docs/checklist.md, docs/structure.md
 */

export interface CommandDefinition {
  id: number;
  name: string;
  execution_string: string;
  is_shell: boolean;
  shell_kind?: string;
  cwd?: string;
  quick_access?: boolean;
}

export interface CommandGroup {
  id: number;
  group_name: string;
  autostart: boolean;
  execution_mode: 'startup' | 'sequential';
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
  group_id?: number;
  group_name?: string;
  template_id?: number;
  template_name?: string;
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

export interface CommandHistory {
  id: string;
  command_line: string;
  shell_kind: string;
  cwd?: string;
  last_exit_code?: number | null;
  run_count: number;
  first_used_at: string;
  last_used_at: string;
  source: 'command' | 'template' | 'shell' | 'typed' | string;
}

/** A history file of the user's own shell (zsh, bash, PSReadLine). */
export interface OsHistorySource {
  shellKinds: string[];
  path: string;
  /** Distinct commands found in the file. */
  entries: number;
  error?: string;
}

export interface OsHistoryImportResult {
  shellKind: string;
  path: string;
  imported: number;
  /** Rejected by the privacy patterns. */
  skipped: number;
}

export interface ActiveProcessInfo {
  runEventId?: string;
  commandId: number;
  commandName: string;
  pid?: number;
  status: ProcessLifecycleStatus;
  startedAt?: string;
  exitCode?: number | null;
  sessionId?: number;
  groupId?: number;
  bufferBytes?: number;
  shellKind?: string;
}

export interface SystemSettings {
  autostartApp: boolean;
  ringBufferSizeBytes: number; // e.g. 1048576 (1MB) or 2097152 (2MB)
  shutdownTimeoutSec: number;  // 5..10 seconds
  fontSize: number;
  fontFamily: string;
  historyEnabled: boolean;
  historyMaxEntries: number;
  historyBlockPatterns: string;
  terminalShell: string;
  ghostTextEnabled: boolean;
  terminalLoadProfile: boolean;
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

// Command Template & Parameter Models (SCR-06, MOD-10, MOD-11)
export type TemplateParamKind = 'string' | 'number' | 'enum' | 'path' | 'bool';

export interface TemplateParam {
  template_id?: number;
  name: string;
  label: string;
  kind: TemplateParamKind;
  default_value?: string;
  required: boolean;
  options?: string[]; // Choice options for enum
  is_secret: boolean;
  param_order: number;
}

export interface CommandTemplate {
  id: number;
  name: string;
  template_string: string;
  is_shell: boolean;
  description?: string;
  params: TemplateParam[];
  last_run_at?: string;
  preset_count?: number;
  presets?: TemplatePreset[];
}

export interface TemplatePreset {
  id: number;
  template_id: number;
  name: string;
  values: Record<string, string>; // Secret params are strictly omitted
  last_used_at?: string;
}

export interface TemplatePreviewPayload {
  rendered: string;
  masked_rendered: string;
  is_shell: boolean;
  validation_errors?: Record<string, string>;
  warnings?: string[];
}

export interface TemplateRunPayload {
  template_id: number;
  param_values: Record<string, string>;
  save_preset_name?: string;
}
