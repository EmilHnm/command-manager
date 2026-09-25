import type {
  CommandDefinition,
  CommandGroup,
  CommandGroupWithCommands,
  RunSession,
  RunEvent,
  SystemSettings,
  BackupIntegrityResult,
} from '@/types/models';

/** Flip when invoke names/payloads match Rust. Until then the shell shows mock data. */
const USE_MOCK_IPC = true;

async function invokeTauri<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const inTauri =
    typeof window !== 'undefined' && ('__TAURI_INTERNALS__' in window || '__TAURI__' in window);
  if (!USE_MOCK_IPC && inTauri) {
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
  { id: 1, group_name: 'Web Platform Development', autostart: true },
  { id: 2, group_name: 'Cloudflare Remote Tunnel', autostart: false },
  { id: 3, group_name: 'Database Migration & Seed', autostart: false },
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
};

// Mock buffer logs cho terminal
const mockLogBuffers = new Map<number, string>();
mockLogBuffers.set(1, '\x1b[35m[vite]\x1b[0m connecting...\n\x1b[32m  ➜  Local:   http://localhost:3000/\x1b[0m\n\x1b[32m  ➜  Network: http://192.168.1.15:3000/\x1b[0m\n\x1b[90m  ➜  press h + enter to show help\x1b[0m\n');
mockLogBuffers.set(2, '\x1b[32m[Nest]\x1b[0m 2845  - 09/25/2026, 2:20:10 PM     LOG [NestFactory] Starting Nest application...\n\x1b[32m[Nest]\x1b[0m 2845  - 09/25/2026, 2:20:11 PM     LOG [RoutesResolver] AppController {/api}: +4ms\n\x1b[32m[Nest]\x1b[0m 2845  - 09/25/2026, 2:20:11 PM     LOG [NestApplication] Nest application successfully started on port 4000\n');
mockLogBuffers.set(3, 'Starting postgresql container...\npostgres: Container postgresql running.\nExited with code 0\n');

async function mockInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  // Giả lập network delay nhẹ
  await new Promise(r => setTimeout(r, 60));

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
        mockGroups = mockGroups.map(g => g.id === groupId ? { id: g.id, group_name: payload.group_name, autostart: payload.autostart } : g);
      } else {
        groupId = Math.max(...mockGroups.map(g => g.id), 0) + 1;
        mockGroups.push({ id: groupId, group_name: payload.group_name, autostart: payload.autostart });
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

    case 'pty_write': {
      const commandId = args?.commandId as number;
      const data = args?.data as string;
      const current = mockLogBuffers.get(commandId) || '';
      mockLogBuffers.set(commandId, current + data);
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
  listCommands: () => invokeTauri<CommandDefinition[]>('commands_list'),
  saveCommand: (command: Partial<CommandDefinition>) => invokeTauri<boolean>('commands_save', { command }),
  deleteCommand: (id: number) => invokeTauri<boolean>('commands_delete', { id }),

  // Groups
  listGroups: () => invokeTauri<CommandGroupWithCommands[]>('groups_list'),
  saveGroup: (group: Partial<CommandGroup> & { commandIds?: number[] }) => invokeTauri<number>('groups_save', { group }),
  deleteGroup: (id: number) => invokeTauri<boolean>('groups_delete', { id }),

  // Session & Execution
  startSession: (groupId: number) => invokeTauri<RunSession>('session_start', { groupId }),
  stopSession: (sessionId: number) => invokeTauri<boolean>('session_stop', { sessionId }),
  stopProcess: (commandId: number, force = false) => invokeTauri<boolean>('process_stop', { commandId, force }),

  // PTY
  writePty: (commandId: number, data: string) => invokeTauri<boolean>('pty_write', { commandId, data }),
  reattachPty: (commandId: number) => invokeTauri<string>('pty_reattach', { commandId }),
  resizePty: (commandId: number, cols: number, rows: number) => invokeTauri<boolean>('pty_resize', { commandId, cols, rows }),

  // History
  listSessions: () => invokeTauri<RunSession[]>('history_sessions'),
  listEvents: (sessionId?: number) => invokeTauri<RunEvent[]>('history_events', { sessionId }),

  // Backup & Restore
  exportBackup: (destinationPath?: string) => invokeTauri<{ path: string; integrityOk: boolean }>('backup_export', { destinationPath }),
  verifyBackup: (filePath: string) => invokeTauri<BackupIntegrityResult>('backup_verify', { filePath }),
  importBackup: (filePath: string) => invokeTauri<boolean>('backup_import', { filePath }),

  // Settings
  getSettings: () => invokeTauri<SystemSettings>('settings_get'),
  saveSettings: (settings: SystemSettings) => invokeTauri<boolean>('settings_set', { settings }),
};
