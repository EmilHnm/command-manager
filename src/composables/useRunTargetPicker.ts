import { ref } from 'vue';
import type { CommandDefinition } from '@/types/models';
import { allowedTargets } from '@/types/models';

interface PickRequest {
  command: Pick<CommandDefinition, 'id' | 'name' | 'execution_string' | 'targets'>;
  targets: string[];
  preselect?: string;
  resolve: (target: string | null) => void;
}

// One dialog for the whole app (RunTargetModal in AppShell).
const request = ref<PickRequest | null>(null);

export function useRunTargetPicker() {
  /**
   * Where to run `command`: answers at once when it has a single target,
   * otherwise opens RunTargetModal. `null` = the user cancelled.
   */
  const pick = (
    command: PickRequest['command'],
    preselect?: string
  ): Promise<string | null> => {
    const targets = allowedTargets(command);
    if (targets.length === 1) return Promise.resolve(targets[0]);
    // A newer request replaces an unanswered one.
    request.value?.resolve(null);
    return new Promise((resolve) => {
      request.value = { command, targets, preselect, resolve };
    });
  };

  const answer = (target: string | null) => {
    const current = request.value;
    request.value = null;
    current?.resolve(target);
  };

  return { request, pick, answer };
}
