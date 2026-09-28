import type {
  ActiveProcessInfo,
  CommandDefinition,
  CommandGroup,
  CommandGroupWithCommands,
  CommandTemplate,
  TemplateParam,
  TemplatePreset,
  ProcessLifecycleStatus,
  RunSession,
  RunEvent,
  CommandHistory,
  OsHistorySource,
  OsHistoryImportResult,
  RunSessionStatus,
  SystemSettings,
  BackupIntegrityResult,
} from '@/types/models';
import { IPC_EVENTS } from '@/ipc/events';

/** Browser preview keeps the mock store; the Tauri runtime always uses the Rust backend. */
const USE_MOCK_IPC = false;

type BackendCommand = {
  id: string;
  name: string;
  execution_string: string;
  is_shell: boolean;
  shell_kind?: string | null;
};

type BackendGroup = {
  id: string;
  group_name: string;
  autostart: boolean;
  execution_mode: 'startup' | 'sequential';
};

type BackendMembership = {
  group_id: string;
  command_id: string;
  execution_order: number;
};

type BackendSession = {
  id: string;
  group_id?: string | null;
  template_id?: string | null;
  template_name?: string | null;
  started_at: string;
  status: string;
};

type BackendEvent = {
  id: string;
  session_id: string;
  command_id: string;
  command_name?: string | null;
  started_at: string;
  ended_at?: string | null;
  status: string;
  exit_code?: number | null;
  pid?: number | null;
};

type BackendHistory = CommandHistory;

type BackendProcess = {
  run_event_id: string;
  command_id: string;
  session_id: string;
  group_id: string;
  pid: number;
  buffer_bytes: number;
  shell_kind?: string;
};

type BackendProcessStatus = {
  run_event_id: string;
  command_id?: string;
  session_id?: string;
  status: string;
  exit_code?: number | null;
  pid?: number | null;
};

type BackendTerminal = {
  run_event_id: string;
  command_id: string;
  session_id: string;
  pid: number;
  shell_kind: string;
  history_level: number;
};

type BackendTemplateParam = {
  name: string;
  label: string;
  kind: TemplateParam['kind'];
  default_value?: string | null;
  required: boolean;
  options?: string[] | null;
  is_secret: boolean;
  param_order: number;
};

type BackendTemplatePreset = {
  id: string;
  template_id: string;
  name: string;
  values: Record<string, string>;
  last_used_at?: string | null;
};

type BackendTemplate = {
  id: string;
  name: string;
  description?: string | null;
  template_string: string;
  is_shell: boolean;
  last_run_at?: string | null;
  params: BackendTemplateParam[];
  presets: BackendTemplatePreset[];
};

const commandUiToBackend = new Map<number, string>();
const commandBackendToUi = new Map<string, number>();
const commandNames = new Map<number, string>();
const groupUiToBackend = new Map<number, string>();
const groupBackendToUi = new Map<string, number>();
const groupNames = new Map<number, string>();
const sessionUiToBackend = new Map<number, string>();
const sessionBackendToUi = new Map<string, number>();
const runEventUiToBackend = new Map<number, string>();
const runEventBackendToUi = new Map<string, number>();
const runEventToCommand = new Map<string, number>();
const runEventToSession = new Map<string, number>();
const terminalBackendToUi = new Map<string, number>();

const TERMINAL_NAMES_STORAGE_KEY = 'cm_terminal_names_v1';

function loadStoredTerminalNames(): Map<number, string> {
  const map = new Map<number, string>();
  if (typeof window === 'undefined') return map;
  try {
    const raw = localStorage.getItem(TERMINAL_NAMES_STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw);
      for (const [k, v] of Object.entries(parsed)) {
        if (typeof v === 'string') map.set(Number(k), v);
      }
    }
  } catch {}
  return map;
}

function persistStoredTerminalNames(map: Map<number, string>) {
  if (typeof window === 'undefined') return;
  try {
    const obj: Record<string, string> = {};
    map.forEach((v, k) => {
      obj[String(k)] = v;
    });
    localStorage.setItem(TERMINAL_NAMES_STORAGE_KEY, JSON.stringify(obj));
  } catch {}
}

const terminalNames = loadStoredTerminalNames();
const templateNames = new Map<string, string>();
const templateUiToBackend = new Map<number, string>();
const templateBackendToUi = new Map<string, number>();
const templatePresetUiToBackend = new Map<number, string>();
const templatePresetBackendToUi = new Map<string, number>();
let nextUiId = 1;
let nextTerminalUiId = -1;
let nextTemplateUiId = 1;
let nextTemplatePresetUiId = 1;

export const isTauriRuntime = () =>
  typeof window !== 'undefined' && ('__TAURI_INTERNALS__' in window || '__TAURI__' in window);

const usingNativeIpc = () => isTauriRuntime() && !USE_MOCK_IPC;

function uiIdFor<T extends Map<number, string>>(forward: T, reverse: Map<string, number>, backendId: string) {
  const existing = reverse.get(backendId);
  if (existing !== undefined) return existing;
  const id = nextUiId++;
  forward.set(id, backendId);
  reverse.set(backendId, id);
  return id;
}

function backendIdFor(map: Map<number, string>, uiId: number, label: string) {
  const backendId = map.get(uiId);
  if (!backendId) throw new Error(`${label} #${uiId} chưa được đồng bộ với backend`);
  return backendId;
}

function isTerminalBackendId(backendId: string) {
  return backendId.startsWith('terminal:') || backendId.startsWith('template:');
}

function terminalUiIdFor(backendId: string) {
  const existing = terminalBackendToUi.get(backendId);
  if (existing !== undefined) return existing;
  const id = nextTerminalUiId--;
  terminalBackendToUi.set(backendId, id);
  if (!terminalNames.has(id)) {
    terminalNames.set(id, 'Terminal');
  }
  return id;
}

function commandUiIdForBackend(backendId: string) {
  return isTerminalBackendId(backendId)
    ? terminalUiIdFor(backendId)
    : uiIdFor(commandUiToBackend, commandBackendToUi, backendId);
}

function commandNameForUi(commandId: number) {
  return terminalNames.get(commandId) || commandNames.get(commandId) || `Lệnh #${commandId}`;
}

function runEventIdForCommand(commandId: number) {
  // Map giữ insertion order; phần tử cuối là lần chạy gần nhất của command.
  return [...runEventToCommand.entries()]
    .reverse()
    .find(([, id]) => id === commandId)?.[0];
}

function rememberCommand(command: BackendCommand) {
  const id = uiIdFor(commandUiToBackend, commandBackendToUi, command.id);
  commandNames.set(id, command.name);
  return id;
}

function rememberTemplate(template: BackendTemplate): CommandTemplate {
  let id = templateBackendToUi.get(template.id);
  if (id === undefined) {
    id = nextTemplateUiId++;
    templateBackendToUi.set(template.id, id);
    templateUiToBackend.set(id, template.id);
  }
  templateNames.set(template.id, template.name);

  const params: TemplateParam[] = template.params.map((param) => ({
    name: param.name,
    label: param.label,
    kind: param.kind,
    default_value: param.default_value ?? undefined,
    required: param.required,
    options: param.options ?? undefined,
    is_secret: param.is_secret,
    param_order: param.param_order,
  }));
  const mappedPresets: TemplatePreset[] = template.presets.map((preset) => {
    let presetId = templatePresetBackendToUi.get(preset.id);
    if (presetId === undefined) {
      presetId = nextTemplatePresetUiId++;
      templatePresetBackendToUi.set(preset.id, presetId);
      templatePresetUiToBackend.set(presetId, preset.id);
    }
    return {
      id: presetId,
      template_id: id!,
      name: preset.name,
      values: { ...preset.values },
      last_used_at: preset.last_used_at ?? undefined,
    };
  });

  return {
    id,
    name: template.name,
    description: template.description ?? undefined,
    template_string: template.template_string,
    is_shell: template.is_shell,
    last_run_at: template.last_run_at ?? undefined,
    preset_count: mappedPresets.length,
    params,
    presets: mappedPresets,
  };
}

function rememberGroup(group: BackendGroup) {
  const id = uiIdFor(groupUiToBackend, groupBackendToUi, group.id);
  groupNames.set(id, group.group_name);
  return id;
}

function rememberSession(session: BackendSession) {
  return uiIdFor(sessionUiToBackend, sessionBackendToUi, session.id);
}

function rememberRunEvent(runEventId: string) {
  return uiIdFor(runEventUiToBackend, runEventBackendToUi, runEventId);
}

function toProcessStatus(status: string): ProcessLifecycleStatus {
  if (status === 'success' || status === 'ended') return 'completed';
  if (status === 'starting') return 'starting';
  if (status === 'failed') return 'failed';
  if (status === 'stopped') return 'stopped';
  if (status === 'running') return 'running';
  return 'idle';
}

function toSessionStatus(status: string): RunSessionStatus {
  if (status === 'success' || status === 'ended' || status === 'completed') return 'completed';
  if (status === 'failed') return 'failed';
  if (status === 'stopped') return 'stopped';
  return 'running';
}

function encodeBase64(value: string) {
  const bytes = new TextEncoder().encode(value);
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
}

function decodeBase64(value: string) {
  const binary = atob(value);
  const bytes = Uint8Array.from(binary, char => char.charCodeAt(0));
  return new TextDecoder().decode(bytes);
}

async function invokeTauri<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (usingNativeIpc()) {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<T>(cmd, args);
  }
  return mockInvoke<T>(cmd, args);
}

// ----------------------------------------------------
// Dữ liệu mẫu khởi tạo cho Mock Store
// ----------------------------------------------------
let mockCommands: CommandDefinition[] = [
  { id: 1, name: 'Vite Frontend Dev', execution_string: 'pnpm --filter web dev --port 3000', is_shell: true },
  { id: 2, name: 'NestJS Backend API', execution_string: 'node dist/main.js --env=local', is_shell: false },
  { id: 3, name: 'Docker PostgreSQL', execution_string: 'docker compose up -d postgres', is_shell: true },
  { id: 4, name: 'Cloudflare Tunnel', execution_string: 'cloudflared tunnel run dev-tunnel', is_shell: false },
  { id: 5, name: 'Redis Cache Server', execution_string: 'redis-server --port 6379', is_shell: false },
  { id: 6, name: 'Prune Docker Data', execution_string: 'docker system prune -af --volumes', is_shell: true },
];

let mockGroups: CommandGroup[] = [
  { id: 1, group_name: 'Web Platform Development', autostart: true, execution_mode: 'startup' },
  { id: 2, group_name: 'Cloudflare Remote Tunnel', autostart: false, execution_mode: 'startup' },
  { id: 3, group_name: 'Database Migration & Seed', autostart: false, execution_mode: 'sequential' },
];

let mockMemberships: { group_id: number; command_id: number; execution_order: number }[] = [
  { group_id: 1, command_id: 3, execution_order: 1 },
  { group_id: 1, command_id: 2, execution_order: 2 },
  { group_id: 1, command_id: 1, execution_order: 3 },
  { group_id: 2, command_id: 4, execution_order: 1 },
];

let mockSessions: RunSession[] = [
  { id: 108, group_id: 1, group_name: 'Web Platform Development', started_at: '2026-09-25 14:20:05', status: 'running' },
  { id: 107, group_id: 3, group_name: 'Database Migration & Seed', started_at: '2026-09-25 11:05:12', ended_at: '2026-09-25 11:05:57', status: 'completed' },
  { id: 106, group_id: 2, group_name: 'Cloudflare Remote Tunnel', started_at: '2026-09-25 09:15:00', ended_at: '2026-09-25 09:15:04', status: 'failed' },
];

let mockEvents: RunEvent[] = [
  { id: 1, session_id: 108, command_id: 3, command_name: 'Docker PostgreSQL', started_at: '14:20:05', ended_at: '14:20:10', status: 'completed', exit_code: 0, pid: 2830 },
  { id: 2, session_id: 108, command_id: 2, command_name: 'NestJS Backend API', started_at: '14:20:10', status: 'running', exit_code: null, pid: 2845 },
  { id: 3, session_id: 108, command_id: 1, command_name: 'Vite Frontend Dev', started_at: '14:20:12', status: 'running', exit_code: null, pid: 2841 },
];

let mockSettings: SystemSettings = {
  autostartApp: true,
  ringBufferSizeBytes: 2097152, // 2MB
  shutdownTimeoutSec: 8,
  fontSize: 13,
  fontFamily: 'JetBrains Mono',
  historyEnabled: true,
  historyMaxEntries: 5000,
  historyBlockPatterns: 'password=\ntoken\n-p\\S+\nauthorization:',
  terminalShell: '',
  ghostTextEnabled: true,
  terminalLoadProfile: true,
};

// Mock buffer logs cho terminal
const mockLogBuffers = new Map<number, string>();
const mockTerminalInput = new Map<number, string>();
const mockPtyDataListeners = new Set<(commandId: number, data: string) => void>();
const mockHistoryAddedListeners = new Set<() => void>();
let mockCommandHistories: CommandHistory[] = [];
const mockOsHistory = ['git status', 'git log --oneline -10', 'pnpm run build', 'cargo test --lib'];
mockLogBuffers.set(1, '\x1b[35m[vite]\x1b[0m connecting...\n\x1b[32m  ➜  Local:   http://localhost:3000/\x1b[0m\n\x1b[32m  ➜  Network: http://192.168.1.15:3000/\x1b[0m\n\x1b[90m  ➜  press h + enter to show help\x1b[0m\n');
mockLogBuffers.set(2, '\x1b[32m[Nest]\x1b[0m 2845  - 09/25/2026, 2:20:10 PM     LOG [NestFactory] Starting Nest application...\n\x1b[32m[Nest]\x1b[0m 2845  - 09/25/2026, 2:20:11 PM     LOG [RoutesResolver] AppController {/api}: +4ms\n\x1b[32m[Nest]\x1b[0m 2845  - 09/25/2026, 2:20:11 PM     LOG [NestApplication] Nest application successfully started on port 4000\n');
mockLogBuffers.set(3, 'Starting postgresql container...\npostgres: Container postgresql running.\nExited with code 0\n');

async function mockInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  // Giả lập network delay nhẹ
  if (cmd !== 'pty_write') await new Promise(r => setTimeout(r, 60));

  switch (cmd) {
    // Commands CRUD
    case 'commands_list':
      return [...mockCommands] as unknown as T;

    case 'commands_save': {
      const payload = args?.command as CommandDefinition;
      if (payload.id) {
        mockCommands = mockCommands.map(c => c.id === payload.id ? { ...payload } : c);
      } else {
        const newId = Math.max(...mockCommands.map(c => c.id), 0) + 1;
        mockCommands.push({ ...payload, id: newId });
      }
      return true as unknown as T;
    }

    case 'commands_delete': {
      const id = args?.id as number;
      mockCommands = mockCommands.filter(c => c.id !== id);
      mockMemberships = mockMemberships.filter(m => m.command_id !== id);
      return true as unknown as T;
    }

    // Groups CRUD
    case 'groups_list': {
      const result: CommandGroupWithCommands[] = mockGroups.map(g => {
        const memberships = mockMemberships
          .filter(m => m.group_id === g.id)
          .sort((a, b) => a.execution_order - b.execution_order);

        const groupCmds = memberships.map(m => {
          const cmdDef = mockCommands.find(c => c.id === m.command_id);
          return {
            ...(cmdDef || { id: m.command_id, name: 'Unknown', execution_string: '', is_shell: false }),
            execution_order: m.execution_order,
          };
        });

        return { ...g, commands: groupCmds };
      });
      return result as unknown as T;
    }

    case 'groups_save': {
      const payload = args?.group as CommandGroup & { commandIds?: number[] };
      let groupId = payload.id;
      if (groupId) {
        mockGroups = mockGroups.map(g => g.id === groupId ? {
          ...g,
          group_name: payload.group_name,
          autostart: payload.autostart,
          execution_mode: payload.execution_mode || 'startup',
        } : g);
      } else {
        groupId = Math.max(...mockGroups.map(g => g.id), 0) + 1;
        mockGroups.push({
          id: groupId,
          group_name: payload.group_name,
          autostart: payload.autostart,
          execution_mode: payload.execution_mode || 'startup',
        });
      }
      if (payload.commandIds) {
        mockMemberships = mockMemberships.filter(m => m.group_id !== groupId);
        payload.commandIds.forEach((cId, idx) => {
          mockMemberships.push({ group_id: groupId, command_id: cId, execution_order: idx + 1 });
        });
      }
      return groupId as unknown as T;
    }

    case 'groups_delete': {
      const id = args?.id as number;
      mockGroups = mockGroups.filter(g => g.id !== id);
      mockMemberships = mockMemberships.filter(m => m.group_id !== id);
      return true as unknown as T;
    }

    // Process & Session
    case 'session_start': {
      const groupId = args?.groupId as number;
      const group = mockGroups.find(g => g.id === groupId);
      const newSessionId = Math.max(...mockSessions.map(s => s.id), 100) + 1;
      const session: RunSession = {
        id: newSessionId,
        group_id: groupId,
        group_name: group?.group_name || 'Group',
        started_at: new Date().toLocaleTimeString(),
        status: 'running',
      };
      mockSessions.unshift(session);
      return session as unknown as T;
    }

    case 'session_stop': {
      const sessionId = args?.sessionId as number;
      mockSessions = mockSessions.map(s => s.id === sessionId ? { ...s, status: 'stopped', ended_at: new Date().toLocaleTimeString() } : s);
      return true as unknown as T;
    }

    case 'process_stop': {
      return true as unknown as T;
    }

    case 'command_run': {
      const commandId = args?.commandId as number;
      const runEventId = `mock-run:${commandId}:${Date.now()}`;
      runEventToCommand.set(runEventId, commandId);
      return {
        run_event_id: runEventId,
        command_id: String(commandId),
        session_id: `mock-command-session:${commandId}`,
        pid: 3000 + commandId,
      } as unknown as T;
    }

    case 'pty_write': {
      const commandId = args?.commandId as number;
      const data = args?.data as string;
      if (data.includes('\x1b')) return true as unknown as T;
      if (commandId < 0) {
        let input = mockTerminalInput.get(commandId) || '';
        let output = '';
        for (const character of data) {
          if (character === '\r' || character === '\n') {
            output += `\r\n\x1b]633;C\x07[Mock] ${input}\r\n\x1b]633;D;0\x07\x1b]633;P;Cwd=C:\\\\Users\\\\HOA\x07\x1b]633;A\x07\x1b]633;B\x07`;
            input = '';
          } else if (character === '\u007f') {
            input = input.slice(0, -1);
            output += '\b \b';
          } else if (character >= ' ') {
            input += character;
            output += character;
          }
        }
        mockTerminalInput.set(commandId, input);
        const current = mockLogBuffers.get(commandId) || '';
        mockLogBuffers.set(commandId, current + output);
        mockPtyDataListeners.forEach(listener => listener(commandId, output));
      } else {
        const current = mockLogBuffers.get(commandId) || '';
        mockLogBuffers.set(commandId, current + data);
      }
      return true as unknown as T;
    }

    case 'pty_reattach': {
      const commandId = args?.commandId as number;
      return (mockLogBuffers.get(commandId) || `[Reattached buffer for command #${commandId}]\n`) as unknown as T;
    }

    case 'pty_resize':
      return true as unknown as T;

    // History
    case 'history_sessions':
      return [...mockSessions] as unknown as T;

    case 'history_events': {
      const sessionId = args?.sessionId as number;
      return mockEvents.filter(e => !sessionId || e.session_id === sessionId) as unknown as T;
    }

    case 'history_list': {
      const query = String(args?.query || '').trim().toLocaleLowerCase();
      const limit = Math.max(1, Math.min(Number(args?.limit) || 500, 10_000));
      return mockCommandHistories
        .filter((row) => !query || [row.command_line, row.shell_kind, row.source]
          .some(value => value.toLocaleLowerCase().includes(query)))
        .slice(0, limit) as unknown as T;
    }

    case 'history_record_typed': {
      const commandLine = String(args?.command_line || '').trimEnd();
      const patterns = mockSettings.historyBlockPatterns
        .split(/\r?\n/)
        .map(pattern => pattern.trim())
        .filter(Boolean);
      const blocked = commandLine.length === 0
        || commandLine.startsWith(' ')
        || patterns.some((pattern) => {
          try {
            return new RegExp(pattern, 'i').test(commandLine);
          } catch {
            return commandLine.toLocaleLowerCase().includes(pattern.toLocaleLowerCase());
          }
        });
      if (mockSettings.historyEnabled && !blocked) {
        const now = new Date().toISOString();
        const existingIndex = mockCommandHistories.findIndex(
          row => row.command_line === commandLine && row.shell_kind === 'mock',
        );
        if (existingIndex >= 0) {
          const existing = mockCommandHistories[existingIndex];
          mockCommandHistories.splice(existingIndex, 1);
          mockCommandHistories.unshift({
            ...existing,
            cwd: (args?.cwd as string | undefined) || existing.cwd,
            run_count: existing.run_count + 1,
            last_used_at: now,
          });
        } else {
          mockCommandHistories.unshift({
            id: `mock-history:${Date.now()}:${mockCommandHistories.length}`,
            command_line: commandLine,
            shell_kind: 'mock',
            cwd: args?.cwd as string | undefined,
            last_exit_code: null,
            run_count: 1,
            first_used_at: now,
            last_used_at: now,
            source: 'typed',
          });
        }
        mockCommandHistories = mockCommandHistories.slice(0, Math.max(1, mockSettings.historyMaxEntries));
        mockHistoryAddedListeners.forEach(listener => listener());
      }
      return true as unknown as T;
    }

    case 'history_delete':
      mockCommandHistories = mockCommandHistories.filter(row => row.id !== args?.id);
      return true as unknown as T;

    case 'history_clear':
      mockCommandHistories = [];
      return true as unknown as T;

    case 'history_os_sources':
      return [
        { shell_kinds: ['mock'], path: '~/.zsh_history (mock)', entries: mockOsHistory.length, error: null },
      ] as unknown as T;

    case 'history_import_os': {
      if (!mockSettings.historyEnabled) {
        throw new Error('Ghi lịch sử lệnh đang tắt; bật lại trong Cài đặt trước khi nhập.');
      }
      const now = new Date().toISOString();
      let imported = 0;
      for (const commandLine of mockOsHistory) {
        if (mockCommandHistories.some(row => row.command_line === commandLine && row.shell_kind === 'mock')) {
          imported += 1;
          continue;
        }
        mockCommandHistories.push({
          id: `mock-os-history:${commandLine}`,
          command_line: commandLine,
          shell_kind: 'mock',
          last_exit_code: null,
          run_count: 1,
          first_used_at: now,
          last_used_at: now,
          source: 'shell',
        });
        imported += 1;
      }
      mockHistoryAddedListeners.forEach(listener => listener());
      return [
        { shell_kind: 'mock', path: '~/.zsh_history (mock)', imported, skipped: 0 },
      ] as unknown as T;
    }

    // Backup & Restore
    case 'backup_export': {
      return {
        path: `/home/user/backups/cm_backup_${Date.now()}.sqlite`,
        integrityOk: true,
      } as unknown as T;
    }

    case 'backup_verify': {
      const result: BackupIntegrityResult = {
        valid: true,
        integrityOk: true,
        schemaVersion: '1.0.0',
        commandCount: mockCommands.length,
        groupCount: mockGroups.length,
        historyCount: mockSessions.length,
      };
      return result as unknown as T;
    }

    case 'backup_import': {
      return true as unknown as T;
    }

    // Settings
    case 'settings_get':
      return { ...mockSettings } as unknown as T;

    case 'settings_set': {
      const newSettings = args?.settings as SystemSettings;
      mockSettings = { ...newSettings };
      return true as unknown as T;
    }

    default:
      console.warn(`[Mock IPC] Unknown command: ${cmd}`);
      return null as unknown as T;
  }
}

// ----------------------------------------------------
// Public API Client Wrappers
// ----------------------------------------------------
export const ipcClient = {
  // Commands
  listCommands: async (): Promise<CommandDefinition[]> => {
    if (!usingNativeIpc()) return invokeTauri<CommandDefinition[]>('commands_list');
    const rows = await invokeTauri<BackendCommand[]>('commands_list');
    return rows.map((row) => ({
      id: rememberCommand(row),
      name: row.name,
      execution_string: row.execution_string,
      is_shell: row.is_shell,
      shell_kind: row.shell_kind ?? undefined,
    }));
  },

  saveCommand: async (command: Partial<CommandDefinition>) => {
    if (!usingNativeIpc()) return invokeTauri<boolean>('commands_save', { command });
    if (!command.name?.trim() || !command.execution_string?.trim()) {
      throw new Error('Tên và chuỗi thực thi không được để trống');
    }

    if (command.id !== undefined) {
      const id = backendIdFor(commandUiToBackend, command.id, 'Lệnh');
      await invokeTauri<void>('commands_update', {
        cmd: {
          id,
          name: command.name,
          execution_string: command.execution_string,
          is_shell: Boolean(command.is_shell),
          shell_kind: command.shell_kind ?? null,
        },
        confirmed: true,
      });
      commandNames.set(command.id, command.name);
    } else {
      const created = await invokeTauri<BackendCommand>('commands_create', {
        name: command.name,
        execution_string: command.execution_string,
        is_shell: Boolean(command.is_shell),
        shell_kind: command.shell_kind ?? null,
        confirmed: true,
      });
      rememberCommand(created);
    }
    return true;
  },

  runCommand: async (commandId: number): Promise<{ commandId: number; runEventId: string; pid?: number }> => {
    if (!usingNativeIpc()) {
      const result = await invokeTauri<BackendTerminal>('command_run', { commandId });
      runEventToCommand.set(result.run_event_id, commandId);
      return { commandId, runEventId: result.run_event_id, pid: result.pid };
    }

    // The library can be opened before the command list has finished loading.
    // Refresh the ID mapping once so a newly created command can run immediately.
    if (!commandUiToBackend.has(commandId)) await ipcClient.listCommands();
    const terminal = await invokeTauri<BackendTerminal>('command_run', {
      command_id: backendIdFor(commandUiToBackend, commandId, 'Lệnh'),
    });
    runEventToCommand.set(terminal.run_event_id, commandId);
    runEventToSession.set(
      terminal.run_event_id,
      uiIdFor(sessionUiToBackend, sessionBackendToUi, terminal.session_id),
    );
    return { commandId, runEventId: terminal.run_event_id, pid: terminal.pid };
  },

  deleteCommand: async (id: number) => {
    if (!usingNativeIpc()) return invokeTauri<boolean>('commands_delete', { id });
    await invokeTauri<void>('commands_delete', {
      id: backendIdFor(commandUiToBackend, id, 'Lệnh'),
      confirmed: true,
    });
    return true;
  },

  // Templates & presets
  listTemplates: async (): Promise<CommandTemplate[]> => {
    const rows = await invokeTauri<BackendTemplate[]>('templates_list');
    return rows.map(rememberTemplate);
  },

  previewTemplate: async (
    templateId: number,
    values: Record<string, string>,
  ): Promise<import('@/types/models').TemplatePreviewPayload> => {
    const preview = await invokeTauri<{
      rendered: string;
      masked_rendered: string;
      is_shell: boolean;
      validation_errors?: Record<string, string> | null;
      warnings?: string[];
    }>('template_preview', {
      template_id: backendIdFor(templateUiToBackend, templateId, 'Template'),
      values,
    });
    return {
      rendered: preview.rendered,
      masked_rendered: preview.masked_rendered,
      is_shell: preview.is_shell,
      validation_errors: preview.validation_errors ?? undefined,
      warnings: preview.warnings,
    };
  },

  saveTemplate: async (template: Partial<CommandTemplate>) => {
    if (!template.name?.trim() || !template.template_string?.trim()) {
      throw new Error('Tên và chuỗi template không được để trống');
    }
    const payload: Record<string, unknown> = {
      name: template.name,
      description: template.description || null,
      template_string: template.template_string,
      is_shell: Boolean(template.is_shell),
      params: (template.params || []).map((param) => ({
        name: param.name,
        label: param.label,
        kind: param.kind,
        default_value: param.default_value ?? null,
        required: Boolean(param.required),
        options: param.options ?? null,
        is_secret: Boolean(param.is_secret),
        param_order: param.param_order,
      })),
    };

    if (template.id !== undefined) {
      payload.id = backendIdFor(templateUiToBackend, template.id, 'Template');
      await invokeTauri<void>('templates_update', { template: payload, confirmed: true });
    } else {
      const created = await invokeTauri<BackendTemplate>('templates_create', {
        template: payload,
        confirmed: true,
      });
      rememberTemplate(created);
    }
    return true;
  },

  deleteTemplate: async (id: number) => {
    await invokeTauri<void>('templates_delete', {
      id: backendIdFor(templateUiToBackend, id, 'Template'),
      confirmed: true,
    });
    return true;
  },

  saveTemplatePreset: async (
    templateId: number,
    name: string,
    values: Record<string, string>,
  ): Promise<TemplatePreset> => {
    const preset = await invokeTauri<BackendTemplatePreset>('template_preset_save', {
      template_id: backendIdFor(templateUiToBackend, templateId, 'Template'),
      name,
      values,
      confirmed: true,
    });
    const mapped = rememberTemplate({
      id: preset.template_id,
      name: '',
      template_string: '',
      is_shell: false,
      params: [],
      presets: [preset],
    }).presets?.[0];
    if (!mapped) throw new Error('Backend không trả về preset vừa lưu');
    return mapped;
  },

  runTemplate: async (
    templateId: number,
    values: Record<string, string>,
  ): Promise<{ commandId: number; runEventId: string; pid?: number }> => {
    if (!usingNativeIpc()) {
      const commandId = nextTerminalUiId--;
      const runEventId = `mock-template-run:${templateId}:${Date.now()}`;
      terminalNames.set(commandId, `Template #${templateId}`);
      runEventToCommand.set(runEventId, commandId);
      return { commandId, runEventId };
    }

    const terminal = await invokeTauri<BackendTerminal>('template_run', {
      template_id: backendIdFor(templateUiToBackend, templateId, 'Template'),
      values,
      confirmed: true,
    });
    const commandId = terminalUiIdFor(terminal.command_id);
    terminalNames.set(commandId, `Template #${templateId}`);
    runEventToCommand.set(terminal.run_event_id, commandId);
    runEventToSession.set(
      terminal.run_event_id,
      uiIdFor(sessionUiToBackend, sessionBackendToUi, terminal.session_id),
    );
    return { commandId, runEventId: terminal.run_event_id, pid: terminal.pid };
  },

  deleteTemplatePreset: async (presetId: number) => {
    await invokeTauri<void>('template_preset_delete', {
      id: backendIdFor(templatePresetUiToBackend, presetId, 'Preset'),
      confirmed: true,
    });
    return true;
  },

  // Groups
  listGroups: async (): Promise<CommandGroupWithCommands[]> => {
    if (!usingNativeIpc()) return invokeTauri<CommandGroupWithCommands[]>('groups_list');

    const [groups, commands] = await Promise.all([
      invokeTauri<BackendGroup[]>('groups_list'),
      invokeTauri<BackendCommand[]>('commands_list'),
    ]);
    const commandByBackendId = new Map(commands.map((command) => [command.id, command]));
    commands.forEach(rememberCommand);

    return Promise.all(groups.map(async (group) => {
      const groupId = rememberGroup(group);
      const memberships = await invokeTauri<BackendMembership[]>('groups_memberships', {
        group_id: group.id,
      });
      const groupCommands = memberships
        .map((membership) => {
          const command = commandByBackendId.get(membership.command_id);
          if (!command) return null;
          return {
            id: rememberCommand(command),
            name: command.name,
            execution_string: command.execution_string,
            is_shell: command.is_shell,
            execution_order: membership.execution_order,
          };
        })
        .filter((command): command is NonNullable<typeof command> => command !== null)
        .sort((a, b) => a.execution_order - b.execution_order);

      return {
        id: groupId,
        group_name: group.group_name,
        autostart: group.autostart,
        execution_mode: group.execution_mode || 'startup',
        commands: groupCommands,
      };
    }));
  },

  saveGroup: async (group: Partial<CommandGroup> & { commandIds?: number[] }) => {
    if (!usingNativeIpc()) return invokeTauri<number>('groups_save', { group });
    if (!group.group_name?.trim()) throw new Error('Tên nhóm không được để trống');

    let backendGroup: BackendGroup;
    if (group.id !== undefined) {
      backendGroup = {
        id: backendIdFor(groupUiToBackend, group.id, 'Nhóm'),
        group_name: group.group_name,
        autostart: Boolean(group.autostart),
        execution_mode: group.execution_mode || 'startup',
      };
      await invokeTauri<void>('groups_update', { group: backendGroup, confirmed: true });
      groupNames.set(group.id, group.group_name);
    } else {
      backendGroup = await invokeTauri<BackendGroup>('groups_create', {
        group_name: group.group_name,
        execution_mode: group.execution_mode || 'startup',
        confirmed: true,
      });
      rememberGroup(backendGroup);
      if (group.autostart) {
        await invokeTauri<void>('groups_set_autostart', {
          group_id: backendGroup.id,
          autostart: true,
          confirmed: true,
        });
      }
    }

    if (group.commandIds) {
      const members: BackendMembership[] = group.commandIds.map((commandId, index) => ({
        group_id: backendGroup.id,
        command_id: backendIdFor(commandUiToBackend, commandId, 'Lệnh'),
        execution_order: index + 1,
      }));
      await invokeTauri<void>('groups_set_memberships', {
        group_id: backendGroup.id,
        members,
        confirmed: true,
      });
    }

    return group.id ?? groupBackendToUi.get(backendGroup.id)!;
  },

  deleteGroup: async (id: number) => {
    if (!usingNativeIpc()) return invokeTauri<boolean>('groups_delete', { id });
    await invokeTauri<void>('groups_delete', {
      id: backendIdFor(groupUiToBackend, id, 'Nhóm'),
      confirmed: true,
    });
    return true;
  },

  // Session & Execution
  startSession: async (groupId: number): Promise<RunSession> => {
    if (!usingNativeIpc()) return invokeTauri<RunSession>('session_start', { groupId });
    const backendSessionId = await invokeTauri<string>('session_start', {
      group_id: backendIdFor(groupUiToBackend, groupId, 'Nhóm'),
    });
    const id = rememberSession({
      id: backendSessionId,
      group_id: backendIdFor(groupUiToBackend, groupId, 'Nhóm'),
      started_at: new Date().toISOString(),
      status: 'running',
    });
    await ipcClient.listProcesses();
    return {
      id,
      group_id: groupId,
      group_name: groupNames.get(groupId),
      started_at: new Date().toISOString(),
      status: 'running',
    };
  },

  openTerminal: async (): Promise<{
    commandId: number;
    runEventId: string;
    pid?: number;
    shellKind: string;
    historyLevel: number;
  }> => {
    if (!usingNativeIpc()) {
      const commandId = nextTerminalUiId--;
      const runEventId = `mock-terminal:${Math.abs(commandId)}`;
      terminalNames.set(commandId, 'Terminal');
      runEventToCommand.set(runEventId, commandId);
      mockTerminalInput.set(commandId, '');
      mockLogBuffers.set(
        commandId,
        '\x1b]633;D;0\x07\x1b]633;P;Cwd=C:\\\\Users\\\\HOA\x07\x1b]633;A\x07\x1b]633;B\x07',
      );
      return { commandId, runEventId, shellKind: 'mock', historyLevel: 2 };
    }

    const terminal = await invokeTauri<BackendTerminal>('terminal_open');
    const commandId = terminalUiIdFor(terminal.command_id);
    runEventToCommand.set(terminal.run_event_id, commandId);
    runEventToSession.set(
      terminal.run_event_id,
      uiIdFor(sessionUiToBackend, sessionBackendToUi, terminal.session_id),
    );
    return {
      commandId,
      runEventId: terminal.run_event_id,
      pid: terminal.pid,
      shellKind: terminal.shell_kind,
      historyLevel: terminal.history_level,
    };
  },

  setTerminalName: (commandId: number, name: string) => {
    terminalNames.set(commandId, name);
    persistStoredTerminalNames(terminalNames);
  },

  getTerminalName: (commandId: number): string | undefined => {
    return terminalNames.get(commandId);
  },

  stopSession: async (sessionId: number) => {
    if (!usingNativeIpc()) return invokeTauri<boolean>('session_stop', { sessionId });
    await invokeTauri<void>('session_stop', {
      session_id: backendIdFor(sessionUiToBackend, sessionId, 'Phiên chạy'),
    });
    return true;
  },

  stopProcess: async (commandId: number, force = false) => {
    if (!usingNativeIpc()) return invokeTauri<boolean>('process_stop', { commandId, force });
    let runEventId = runEventIdForCommand(commandId);
    if (!runEventId) {
      await ipcClient.listProcesses();
      runEventId = runEventIdForCommand(commandId);
    }
    if (!runEventId) throw new Error('Không tìm thấy tiến trình đang chạy cho lệnh này');
    await invokeTauri<void>('process_stop', { run_event_id: runEventId, force });
    return true;
  },

  // PTY
  writePty: async (commandId: number, data: string, directRunEventId?: string) => {
    if (!usingNativeIpc()) return invokeTauri<boolean>('pty_write', { commandId, data });
    const runEventId = directRunEventId || runEventIdForCommand(commandId);
    if (!runEventId) return false;
    await invokeTauri<void>('pty_write', { run_event_id: runEventId, b64: encodeBase64(data) });
    return true;
  },

  recordTypedHistory: async (runEventId: string, commandLine: string, cwd?: string) => {
    if (!usingNativeIpc()) {
      await invokeTauri<void>('history_record_typed', {
        run_event_id: runEventId,
        command_line: commandLine,
        cwd,
      });
      return true;
    }
    await invokeTauri<void>('history_record_typed', {
      run_event_id: runEventId,
      command_line: commandLine,
      cwd,
    });
    return true;
  },

  reattachPty: async (commandId: number, directRunEventId?: string): Promise<string> => {
    if (!usingNativeIpc()) return invokeTauri<string>('pty_reattach', { commandId });
    const runEventId = directRunEventId || runEventIdForCommand(commandId);
    if (!runEventId) return '';
    const b64 = await invokeTauri<string>('pty_reattach', { run_event_id: runEventId });
    return decodeBase64(b64);
  },

  resizePty: async (commandId: number, cols: number, rows: number, directRunEventId?: string) => {
    if (!usingNativeIpc()) return invokeTauri<boolean>('pty_resize', { commandId, cols, rows });
    const runEventId = directRunEventId || runEventIdForCommand(commandId);
    if (!runEventId) return false;
    await invokeTauri<void>('pty_resize', { run_event_id: runEventId, cols, rows });
    return true;
  },

  // History
  listSessions: async (): Promise<RunSession[]> => {
    if (!usingNativeIpc()) return invokeTauri<RunSession[]>('history_sessions');
    const rows = await invokeTauri<BackendSession[]>('sessions_list');
    return rows.map((row) => {
      const groupId = row.group_id
        ? uiIdFor(groupUiToBackend, groupBackendToUi, row.group_id)
        : undefined;
      const templateId = row.template_id
        ? uiIdFor(templateUiToBackend, templateBackendToUi, row.template_id)
        : undefined;
      const id = rememberSession(row);
      return {
        id,
        group_id: groupId,
        group_name: groupId === undefined ? undefined : groupNames.get(groupId),
        template_id: templateId,
        template_name: row.template_name ?? (row.template_id ? templateNames.get(row.template_id) : undefined),
        started_at: row.started_at,
        status: toSessionStatus(row.status),
      };
    });
  },

  listCommandHistory: async (query = '', limit = 500): Promise<CommandHistory[]> => {
    return invokeTauri<BackendHistory[]>('history_list', { query, limit });
  },

  deleteCommandHistory: async (id: string): Promise<void> => {
    await invokeTauri<void>('history_delete', { id, confirmed: true });
  },

  clearCommandHistory: async (): Promise<void> => {
    await invokeTauri<void>('history_clear', { confirmed: true });
  },

  listOsHistorySources: async (): Promise<OsHistorySource[]> => {
    const rows = await invokeTauri<Array<{
      shell_kinds: string[];
      path: string;
      entries: number;
      error: string | null;
    }>>('history_os_sources');
    return rows.map(row => ({
      shellKinds: row.shell_kinds,
      path: row.path,
      entries: row.entries,
      error: row.error ?? undefined,
    }));
  },

  importOsHistory: async (): Promise<OsHistoryImportResult[]> => {
    const rows = await invokeTauri<Array<{
      shell_kind: string;
      path: string;
      imported: number;
      skipped: number;
    }>>('history_import_os', { confirmed: true });
    return rows.map(row => ({
      shellKind: row.shell_kind,
      path: row.path,
      imported: row.imported,
      skipped: row.skipped,
    }));
  },

  listEvents: async (sessionId?: number): Promise<RunEvent[]> => {
    if (!usingNativeIpc()) return invokeTauri<RunEvent[]>('history_events', { sessionId });
    if (sessionId === undefined) return [];
    const rows = await invokeTauri<BackendEvent[]>('session_events', {
      session_id: backendIdFor(sessionUiToBackend, sessionId, 'Phiên chạy'),
    });
    return rows.map((row) => {
      const commandId = commandUiIdForBackend(row.command_id);
      const id = rememberRunEvent(row.id);
      runEventToCommand.set(row.id, commandId);
      return {
        id,
        session_id: sessionId,
        command_id: commandId,
        command_name: row.command_name ?? commandNameForUi(commandId),
        started_at: row.started_at,
        ended_at: row.ended_at ?? undefined,
        status: toProcessStatus(row.status),
        exit_code: row.exit_code ?? null,
        pid: row.pid ?? null,
      };
    });
  },

  getEventLog: async (runEventId: number): Promise<string> => {
    if (!usingNativeIpc()) return '[Mock] Ring Buffer log của sự kiện chưa được mô phỏng.\n';
    const backendRunEventId = runEventUiToBackend.get(runEventId);
    if (!backendRunEventId) throw new Error('Sự kiện chưa được đồng bộ với backend');
    const b64 = await invokeTauri<string>('pty_reattach', { run_event_id: backendRunEventId });
    return decodeBase64(b64);
  },

  listProcesses: async (): Promise<ActiveProcessInfo[]> => {
    if (!usingNativeIpc()) return [];
    const rows = await invokeTauri<BackendProcess[]>('process_list');
    return rows.map((row) => {
      const commandId = commandUiIdForBackend(row.command_id);
      const sessionId = uiIdFor(sessionUiToBackend, sessionBackendToUi, row.session_id);
      const groupId = row.group_id === 'terminal' || row.group_id === 'command'
        ? undefined
        : uiIdFor(groupUiToBackend, groupBackendToUi, row.group_id);
      runEventToCommand.set(row.run_event_id, commandId);
      runEventToSession.set(row.run_event_id, sessionId);
      return {
        runEventId: row.run_event_id,
        commandId,
        commandName: commandNameForUi(commandId),
        pid: row.pid,
        status: 'running',
        sessionId,
        groupId,
        bufferBytes: row.buffer_bytes,
        shellKind: row.shell_kind,
      };
    });
  },

  // Backup & Restore
  exportBackup: async (destinationPath?: string) => {
    if (!usingNativeIpc()) return invokeTauri<{ path: string; integrityOk: boolean }>('backup_export', { destinationPath });
    const args = destinationPath ? { dest: destinationPath } : undefined;
    const path = await invokeTauri<string>('backup_export', args);
    return { path, integrityOk: true };
  },

  getBackupInfo: async (): Promise<BackupIntegrityResult> => {
    if (!usingNativeIpc()) return invokeTauri<BackupIntegrityResult>('backup_verify', { filePath: '' });
    const info = await invokeTauri<{
      schema_version: number;
      command_count: number;
      group_count: number;
      history_count: number;
      integrity_ok: boolean;
    }>('backup_info');
    return {
      valid: true,
      integrityOk: info.integrity_ok,
      schemaVersion: String(info.schema_version),
      commandCount: info.command_count,
      groupCount: info.group_count,
      historyCount: info.history_count,
    };
  },

  verifyBackupBytes: async (b64: string): Promise<BackupIntegrityResult> => {
    if (!usingNativeIpc()) return invokeTauri<BackupIntegrityResult>('backup_verify', { filePath: '' });
    const info = await invokeTauri<{
      schema_version: number;
      command_count: number;
      group_count: number;
      history_count: number;
      integrity_ok: boolean;
    }>('backup_verify_bytes', { b64 });
    return {
      valid: info.integrity_ok,
      integrityOk: info.integrity_ok,
      schemaVersion: String(info.schema_version),
      commandCount: info.command_count,
      groupCount: info.group_count,
      historyCount: info.history_count,
    };
  },

  verifyBackup: async (_filePath: string): Promise<BackupIntegrityResult> => {
    throw new Error('Backend chưa expose lệnh verify_backup độc lập; import sẽ tự kiểm tra integrity_check');
  },

  importBackup: async (filePath: string) => {
    if (!usingNativeIpc()) return invokeTauri<boolean>('backup_import', { filePath });
    await invokeTauri<void>('backup_import', { src: filePath, confirmed: true });
    return true;
  },

  importBackupBytes: async (b64: string) => {
    if (!usingNativeIpc()) return invokeTauri<boolean>('backup_import_bytes', { b64, confirmed: true });
    await invokeTauri<void>('backup_import_bytes', { b64, confirmed: true });
    return true;
  },

  // Settings
  getSettings: async (): Promise<SystemSettings> => {
    if (!usingNativeIpc()) return invokeTauri<SystemSettings>('settings_get');
    const rows = await invokeTauri<[string, string][]>('settings_get');
    let autostartApp = false;
    try {
      autostartApp = await invokeTauri<boolean>('autostart_os_is_enabled');
    } catch {
      autostartApp = false;
    }
    const values = new Map(rows);
    return {
      autostartApp,
      ringBufferSizeBytes: Number(values.get('ring_buffer_bytes') || 2097152),
      shutdownTimeoutSec: Number(values.get('shutdown_timeout_secs') || 8),
      fontSize: Number(values.get('font_size') || 13),
      fontFamily: values.get('font_family') || 'JetBrains Mono',
      historyEnabled: values.get('history_enabled') !== 'false',
      historyMaxEntries: Number(values.get('history_max_entries') || 5000),
      historyBlockPatterns: values.get('history_block_patterns') || '',
      terminalShell: values.get('terminal_shell') || '',
      ghostTextEnabled: values.get('ghost_text_enabled') !== 'false',
      terminalLoadProfile: values.get('terminal_load_profile') !== 'false',
    };
  },

  saveSettings: async (settings: SystemSettings) => {
    if (!usingNativeIpc()) return invokeTauri<boolean>('settings_set', { settings });
    const values: [string, string][] = [
      ['ring_buffer_bytes', String(settings.ringBufferSizeBytes)],
      ['shutdown_timeout_secs', String(settings.shutdownTimeoutSec)],
      ['font_size', String(settings.fontSize)],
      ['font_family', settings.fontFamily],
      ['history_enabled', String(settings.historyEnabled)],
      ['history_max_entries', String(settings.historyMaxEntries)],
      ['history_block_patterns', settings.historyBlockPatterns],
      ['terminal_shell', settings.terminalShell],
      ['ghost_text_enabled', String(settings.ghostTextEnabled)],
      ['terminal_load_profile', String(settings.terminalLoadProfile)],
    ];
    await Promise.all(values.map(([key, value]) => invokeTauri<void>('settings_set', {
      key,
      value,
      confirmed: true,
    })));
    await invokeTauri<void>('autostart_os_set', { enabled: settings.autostartApp, confirmed: true });
    return true;
  },

  onPtyData: async (callback: (commandId: number, data: string) => void) => {
    if (!usingNativeIpc()) {
      mockPtyDataListeners.add(callback);
      return () => mockPtyDataListeners.delete(callback);
    }
    const { listen } = await import('@tauri-apps/api/event');
    return listen<{ run_event_id: string; b64: string }>(IPC_EVENTS.PTY_DATA, (event) => {
      const commandId = runEventToCommand.get(event.payload.run_event_id);
      if (commandId !== undefined) callback(commandId, decodeBase64(event.payload.b64));
    });
  },

  onProcessStatus: async (callback: (process: ActiveProcessInfo) => void) => {
    if (!usingNativeIpc()) return () => undefined;
    const { listen } = await import('@tauri-apps/api/event');
    return listen<BackendProcessStatus>(IPC_EVENTS.PROCESS_STATUS, (event) => {
      const payload = event.payload;
      const commandId = payload.command_id
        ? commandUiIdForBackend(payload.command_id)
        : runEventToCommand.get(payload.run_event_id);
      if (commandId === undefined) return;
      runEventToCommand.set(payload.run_event_id, commandId);
      if (payload.session_id) {
        const sessionId = uiIdFor(sessionUiToBackend, sessionBackendToUi, payload.session_id);
        runEventToSession.set(payload.run_event_id, sessionId);
      }
      callback({
        runEventId: payload.run_event_id,
        commandId,
        commandName: commandNameForUi(commandId),
        pid: payload.pid ?? undefined,
        exitCode: payload.exit_code ?? null,
        status: toProcessStatus(payload.status),
        sessionId: runEventToSession.get(payload.run_event_id),
      });
    });
  },

  onHistoryAdded: async (callback: () => void) => {
    if (!usingNativeIpc()) {
      mockHistoryAddedListeners.add(callback);
      return () => mockHistoryAddedListeners.delete(callback);
    }
    const { listen } = await import('@tauri-apps/api/event');
    return listen(IPC_EVENTS.HISTORY_ADDED, () => callback());
  },

  openUrl: async (url: string): Promise<void> => {
    if (!url) return;
    try {
      if (usingNativeIpc()) {
        await invokeTauri('open_url', { url });
        return;
      }
    } catch (e) {
      console.warn('[IPC] open_url error, falling back to window.open:', e);
    }
    window.open(url, '_blank', 'noopener,noreferrer');
  },
};

