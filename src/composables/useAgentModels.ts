import { reactive } from 'vue';
import { ipcClient } from '@/ipc/client';
import type { AgentCli, ModelInfo } from '@/types/agents';
import { MODEL_SUGGESTIONS } from '@/types/agents';

/** Models each CLI listed, shared by every model field of the app. */
const lists = reactive<Partial<Record<AgentCli, ModelInfo[]>>>({});
const loading = reactive<Partial<Record<AgentCli, boolean>>>({});
const errors = reactive<Partial<Record<AgentCli, string>>>({});

const fallback = (cli: AgentCli): ModelInfo[] => MODEL_SUGGESTIONS[cli].map((id) => ({ id, label: id }));

export function useAgentModels() {
  /** Reads the CLI's list (the backend keeps it until `refresh`). */
  const load = async (cli: AgentCli, refresh = false) => {
    if (loading[cli]) return;
    loading[cli] = true;
    try {
      lists[cli] = await ipcClient.agentsModels(cli, refresh);
      delete errors[cli];
    } catch (e) {
      errors[cli] = e instanceof Error ? e.message : String(e);
    } finally {
      loading[cli] = false;
    }
  };

  /** Suggestions for `cli`; the first ask starts reading its list. */
  const modelsOf = (cli: AgentCli): ModelInfo[] => {
    if (!lists[cli] && !loading[cli] && !errors[cli]) {
      // Not while rendering: the read changes what is rendered.
      queueMicrotask(() => void load(cli));
    }
    return lists[cli]?.length ? lists[cli]! : fallback(cli);
  };

  return { modelsOf, load, loading, errors };
}
