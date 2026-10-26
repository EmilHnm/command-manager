<template>
  <div v-if="visible" class="modal-backdrop">
    <div class="ai-push-modal" role="dialog" aria-modal="true">
      <div class="modal-head">
        <Sparkles :size="16" />
        <span class="modal-title">Push bằng AI</span>
        <button type="button" class="btn-close" title="Đóng" :disabled="busy" @click="emit('close')">
          <X :size="16" />
        </button>
      </div>
      <div class="modal-body">
        <p class="hint">
          Agent làm như khi bạn tự push, theo skill push của dự án nếu có: fetch, rebase lên nhánh đích, gỡ xung đột, chạy
          lệnh kiểm tra và sửa lỗi, rồi <code>git push --force-with-lease</code> lên nhánh PR (không bao giờ
          <code>--no-verify</code>). Agent làm trong worktree riêng, <strong>có quyền push</strong>. App chỉ coi là xong
          khi nhánh trên GitHub đúng bằng commit agent làm ra.
        </p>
        <div class="targets font-mono">
          <div v-for="pr in pullRequests" :key="pr.url">{{ pr.branch }} → {{ pr.base }}</div>
        </div>
        <div class="row">
          <label class="block">
            <span class="label">CLI</span>
            <select v-model="cli" class="field">
              <option v-for="item in AGENT_CLIS" :key="item" :value="item" :disabled="!installed(item)">
                {{ item }}{{ installed(item) ? '' : ' (chưa cài)' }}
              </option>
            </select>
          </label>
          <label class="block grow">
            <span class="label">Model</span>
            <input
              v-model="model"
              class="field font-mono"
              :list="`ai-push-models-${cli}`"
              :placeholder="profileModelHint(clis.find((info) => info.cli === cli)?.profile, 'lead')"
            />
            <datalist :id="`ai-push-models-${cli}`">
              <option v-for="item in modelsOf(cli)" :key="item.id" :value="item.id">{{ item.label }}</option>
            </datalist>
          </label>
          <label class="block">
            <span class="label">Effort</span>
            <select v-model="effort" class="field" :disabled="cli === 'cursor'">
              <option value="">mặc định</option>
              <option v-for="level in EFFORT_LEVELS" :key="level" :value="level">{{ level }}</option>
            </select>
          </label>
        </div>
        <label class="block">
          <span class="label">Ghi chú cho agent (tùy chọn)</span>
          <textarea
            v-model="note"
            rows="4"
            class="field font-mono"
            placeholder="Ví dụ: khi xung đột ở messages/ja/account.json thì giữ câu của develop"
          ></textarea>
        </label>
        <div v-if="error" class="error">{{ error }}</div>
      </div>
      <div class="modal-foot">
        <button type="button" class="btn btn-secondary btn-sm" :disabled="busy" @click="emit('close')">Hủy</button>
        <button type="button" class="btn btn-primary btn-sm" :disabled="busy" @click="submit">
          <Sparkles :size="13" /> {{ busy ? 'Đang bắt đầu…' : 'Giao cho AI push' }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue';
import { Sparkles, X } from 'lucide-vue-next';
import { ipcClient } from '@/ipc/client';
import type { AgentCli, AgentRun, CliInfo, PullRequest } from '@/types/agents';
import { AGENT_CLIS, EFFORT_LEVELS, profileModelHint } from '@/types/agents';
import { useAgentModels } from '@/composables/useAgentModels';

const props = defineProps<{
  visible: boolean;
  run: AgentRun | null;
  clis: CliInfo[];
}>();

const emit = defineEmits<{
  (e: 'close'): void;
  (e: 'started'): void;
}>();

const { modelsOf } = useAgentModels();
const cli = ref<AgentCli>('claude');
const model = ref('');
const effort = ref('');
const note = ref('');
const busy = ref(false);
const error = ref<string | null>(null);
const pullRequests = ref<PullRequest[]>([]);

const installed = (name: AgentCli) => props.clis.some((info) => info.cli === name && info.path);

watch(
  () => props.visible,
  (visible) => {
    if (!visible || !props.run) return;
    pullRequests.value = props.run.pull_requests;
    cli.value = installed(props.run.leader_cli) ? props.run.leader_cli : 'claude';
    model.value = '';
    effort.value = '';
    note.value = '';
    error.value = null;
  }
);

watch(cli, () => {
  model.value = '';
});

async function submit() {
  if (!props.run) return;
  busy.value = true;
  error.value = null;
  try {
    await ipcClient.agentsRunAiPush(props.run.id, {
      cli: cli.value,
      model: model.value.trim() || null,
      effort: effort.value || null,
      note: note.value.trim() || null,
    });
    emit('started');
    emit('close');
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    busy.value = false;
  }
}
</script>

<style scoped>
.modal-backdrop {
  position: fixed;
  inset: 0;
  z-index: 1100;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 16px;
  background-color: rgba(5, 7, 12, 0.7);
}

.ai-push-modal {
  width: min(640px, 100%);
  max-height: calc(100vh - 32px);
  display: flex;
  flex-direction: column;
  background-color: var(--surface-container, #1a1e2b);
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 8px;
  color: var(--text-primary, #f1f5f9);
}

.modal-head {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px 14px;
  border-bottom: 1px solid var(--border-subtle, #242a3e);
  color: #e4b5ff;
}

.modal-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary, #f1f5f9);
}

.btn-close {
  margin-left: auto;
  padding: 4px;
  color: var(--text-secondary, #94a3b8);
  background: none;
  border: none;
  cursor: pointer;
}

.modal-body {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 14px;
  overflow-y: auto;
}

.hint {
  margin: 0;
  font-size: 12px;
  line-height: 1.5;
  color: var(--text-secondary, #94a3b8);
}

.targets {
  font-size: 12px;
  color: #60a5fa;
}

.row {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.block {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 110px;
}

.block.grow {
  flex: 1;
}

.label {
  font-size: 11px;
  color: var(--text-muted, #64748b);
}

.field {
  min-width: 0;
  padding: 6px 8px;
  font-size: 12px;
  color: var(--text-primary, #f1f5f9);
  background-color: var(--bg-app-base, #0f1117);
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 4px;
  resize: vertical;
}

.error {
  font-size: 12px;
  color: var(--status-failed, #ef4444);
}

.modal-foot {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 12px 14px;
  border-top: 1px solid var(--border-subtle, #242a3e);
}
</style>
