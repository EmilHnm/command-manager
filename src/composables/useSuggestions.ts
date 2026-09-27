import { onBeforeUnmount, ref } from 'vue';
import { ipcClient, isTauriRuntime } from '@/ipc/client';
import { mockTemplates } from '@/composables/useTemplates';
import type { CommandDefinition, CommandHistory, CommandTemplate } from '@/types/models';

export function useSuggestions(onChanged?: () => void) {
  const history = ref<CommandHistory[]>([]);
  let loaded = false;
  let loadGeneration = 0;
  let disposed = false;
  let stopHistoryListener: (() => void) | undefined;

  const loadHistory = async () => {
    const generation = ++loadGeneration;
    const [rows, commands, templates] = await Promise.all([
      ipcClient.listCommandHistory('', 500).catch(() => [] as CommandHistory[]),
      ipcClient.listCommands().catch(() => [] as CommandDefinition[]),
      isTauriRuntime()
        ? ipcClient.listTemplates().catch(() => [] as CommandTemplate[])
        : Promise.resolve(mockTemplates),
    ]);
    const staticRows: CommandHistory[] = [
      ...commands.map((command) => ({
        id: `definition:${command.id}`,
        command_line: command.execution_string,
        shell_kind: command.is_shell ? 'shell' : 'argv',
        cwd: command.cwd,
        last_exit_code: null,
        run_count: 0,
        first_used_at: '',
        last_used_at: '',
        source: 'command',
      })),
      ...templates.map((template) => ({
        id: `template:${template.id}`,
        command_line: template.template_string,
        shell_kind: template.is_shell ? 'shell' : 'argv',
        cwd: undefined,
        last_exit_code: null,
        run_count: 0,
        first_used_at: '',
        last_used_at: '',
        source: 'template',
      })),
    ];
    if (disposed || generation !== loadGeneration) return rows;
    const merged = [...rows, ...staticRows];
    const unique = new Map<string, CommandHistory>();
    for (const row of merged) unique.set(`${row.command_line}\n${row.shell_kind}`, row);
    history.value = [...unique.values()];
    loaded = true;
    onChanged?.();
    return rows;
  };

  const findSuggestions = (input: string, cwd?: string, shellKind?: string) => {
    if (!loaded || !input.trim()) return [] as CommandHistory[];
    return history.value
      .filter((row) => row.command_line.startsWith(input))
      .sort((a, b) => compareSuggestionRank(a, b, cwd, shellKind));
  };

  const findFuzzySuggestions = (input: string, cwd?: string, shellKind?: string) => {
    if (!loaded) return [] as CommandHistory[];
    const query = input.trim().toLocaleLowerCase();
    return history.value
      .map((row) => ({ row, score: fuzzyScore(row.command_line, query) }))
      .filter(({ score }) => score > 0)
      .sort((a, b) => b.score - a.score || compareSuggestionRank(a.row, b.row, cwd, shellKind))
      .map(({ row }) => row);
  };

  const findHistoryEntries = (input: string, shellKind?: string) => {
    if (!loaded) return [] as CommandHistory[];
    const seen = new Set<string>();
    return history.value
      .filter((row) => row.shell_kind === shellKind && (row.source === 'shell' || row.source === 'typed'))
      .filter((row) => !input || row.command_line.startsWith(input))
      .filter((row) => {
        if (seen.has(row.command_line)) return false;
        seen.add(row.command_line);
        return true;
      });
  };

  void ipcClient.onHistoryAdded(() => {
    void loadHistory();
  }).then((unlisten) => {
    if (disposed) {
      if (typeof unlisten === 'function') unlisten();
      return;
    }
    stopHistoryListener = typeof unlisten === 'function' ? unlisten : undefined;
  });
  onBeforeUnmount(() => {
    disposed = true;
    stopHistoryListener?.();
  });

  return { history, loadHistory, findSuggestions, findFuzzySuggestions, findHistoryEntries };
}

function compareSuggestionRank(a: CommandHistory, b: CommandHistory, cwd?: string, shellKind?: string) {
  const shellScore = Number(Boolean(shellKind && b.shell_kind === shellKind))
    - Number(Boolean(shellKind && a.shell_kind === shellKind));
  if (shellScore) return shellScore;
  const cwdScore = Number(Boolean(b.cwd && cwd && b.cwd === cwd)) - Number(Boolean(a.cwd && cwd && a.cwd === cwd));
  if (cwdScore) return cwdScore;
  const exitScore = Number(b.last_exit_code === 0) - Number(a.last_exit_code === 0);
  if (exitScore) return exitScore;
  return b.run_count - a.run_count || b.last_used_at.localeCompare(a.last_used_at);
}

function fuzzyScore(value: string, query: string) {
  if (!query) return 1;
  const candidate = value.toLocaleLowerCase();
  if (candidate === query) return 10_000;
  if (candidate.startsWith(query)) return 8_000 - candidate.length;
  const substring = candidate.indexOf(query);
  if (substring >= 0) return 5_000 - substring;

  let queryIndex = 0;
  let score = 0;
  for (let index = 0; index < candidate.length && queryIndex < query.length; index += 1) {
    if (candidate[index] !== query[queryIndex]) continue;
    score += index === 0 || /[^a-z0-9]/i.test(candidate[index - 1]) ? 12 : 4;
    queryIndex += 1;
  }
  return queryIndex === query.length ? score : 0;
}
