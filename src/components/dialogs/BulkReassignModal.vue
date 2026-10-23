<template>
  <div v-if="visible" class="modal-backdrop">
    <div class="bulk-modal" role="dialog" aria-modal="true">
      <div class="modal-head">
        <Users :size="16" />
        <span class="modal-title">Giao lại các task lỗi</span>
        <button type="button" class="btn-close" title="Đóng" @click="emit('close')">
          <X :size="16" />
        </button>
      </div>
      <div class="modal-body">
        <p class="hint">
          Mỗi task mở phiên mới với agent được chọn, trên cùng worktree (code dở vẫn còn), rồi lượt chạy làm lần lượt
          từng task. Để trống model là dùng mặc định trong Đội &amp; CLI.
        </p>

        <div class="task-list">
          <label v-for="task in tasks" :key="task.id" class="task-item">
            <input v-model="picked" type="checkbox" :value="task.id" />
            <span class="task-key font-mono">{{ task.key }}</span>
            <span class="task-title">{{ task.title }}</span>
            <span class="task-agent font-mono">{{ task.cli }}{{ task.model ? ` · ${task.model}` : '' }}</span>
            <span class="task-note" :title="task.status_note || ''">{{ task.status_note }}</span>
          </label>
        </div>

        <div class="agent-row">
          <label class="field-wrap">
            <span class="label">CLI</span>
            <select v-model="cli" class="field">
              <option v-for="item in AGENT_CLIS" :key="item" :value="item" :disabled="!installed(item)">
                {{ item }}{{ installed(item) ? '' : ' (chưa cài)' }}
              </option>
            </select>
          </label>
          <label class="field-wrap grow">
            <span class="label">Model</span>
            <input
              v-model="model"
              class="field font-mono"
              :list="`bulk-models-${cli}`"
              :placeholder="profileModelHint(clis.find((info) => info.cli === cli)?.profile, 'work')"
            />
            <datalist :id="`bulk-models-${cli}`">
              <option v-for="item in modelsOf(cli)" :key="item.id" :value="item.id">{{ item.label }}</option>
            </datalist>
          </label>
          <label class="field-wrap timeout" title="Để trống: giữ timeout hiện tại của từng task">
            <span class="label">Timeout (phút)</span>
            <input v-model.number="timeout" type="number" min="1" :max="MAX_TIMEOUT_MINUTES" step="5" class="field font-mono" placeholder="giữ nguyên" />
          </label>
          <label class="field-wrap">
            <span class="label">Effort</span>
            <select v-model="effort" class="field" :disabled="cli === 'cursor'">
              <option value="">mặc định</option>
              <option v-for="level in EFFORT_LEVELS" :key="level" :value="level">{{ level }}</option>
            </select>
          </label>
        </div>
        <div v-if="error" class="error">{{ error }}</div>
      </div>
      <div class="modal-foot">
        <button type="button" class="btn btn-secondary btn-sm" :disabled="busy" @click="emit('close')">Hủy</button>
        <button type="button" class="btn btn-primary btn-sm" :disabled="busy || picked.length === 0" @click="submit">
          <Play :size="13" /> {{ busy ? 'Đang giao…' : `Giao lại ${picked.length} task và chạy tiếp` }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue';
import { Play, Users, X } from 'lucide-vue-next';
import type { AgentCli, AgentTask, CliInfo } from '@/types/agents';
import { AGENT_CLIS, EFFORT_LEVELS, MAX_TIMEOUT_MINUTES, profileModelHint } from '@/types/agents';
import { useAgentModels } from '@/composables/useAgentModels';

const { modelsOf } = useAgentModels();

const props = defineProps<{
  visible: boolean;
  tasks: AgentTask[];
  clis: CliInfo[];
  submit: (
    taskIds: string[],
    cli: AgentCli,
    model: string | null,
    effort: string | null,
    timeoutMinutes: number | null
  ) => Promise<void>;
}>();

const emit = defineEmits<{
  (e: 'close'): void;
}>();

const picked = ref<string[]>([]);
const cli = ref<AgentCli>('claude');
const model = ref('');
const effort = ref('');
const timeout = ref<number | ''>('');
const busy = ref(false);
const error = ref<string | null>(null);

const installed = (name: AgentCli) => props.clis.some((info) => info.cli === name && info.path);

watch(
  () => props.visible,
  (visible) => {
    if (!visible) return;
    picked.value = props.tasks.map((task) => task.id);
    // Start from the agent the tasks had, when they share one.
    const shared = props.tasks[0]?.cli;
    cli.value = shared && props.tasks.every((task) => task.cli === shared) ? shared : 'claude';
    model.value = '';
    effort.value = '';
    timeout.value = '';
    error.value = null;
  },
  { immediate: true }
);

// Model names belong to one CLI.
watch(cli, () => {
  model.value = '';
});

async function submit() {
  busy.value = true;
  error.value = null;
  try {
    await props.submit(
      picked.value,
      cli.value,
      model.value.trim() || null,
      effort.value || null,
      typeof timeout.value === 'number' && timeout.value > 0 ? timeout.value : null
    );
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
  z-index: 1000;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 16px;
  background-color: rgba(5, 7, 12, 0.7);
}

.bulk-modal {
  width: min(720px, 100%);
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
  gap: 12px;
  padding: 14px;
  overflow-y: auto;
}

.hint {
  margin: 0;
  font-size: 12px;
  line-height: 1.5;
  color: var(--text-secondary, #94a3b8);
}

.task-list {
  display: flex;
  flex-direction: column;
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 6px;
}

.task-item {
  display: grid;
  grid-template-columns: auto auto 1fr auto;
  gap: 4px 10px;
  align-items: center;
  padding: 8px 10px;
  font-size: 12px;
  cursor: pointer;
}

.task-item + .task-item {
  border-top: 1px solid var(--border-subtle, #242a3e);
}

.task-key {
  font-weight: 600;
}

.task-title {
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
  color: var(--text-secondary, #94a3b8);
}

.task-agent {
  font-size: 11px;
  color: #e4b5ff;
}

.task-note {
  grid-column: 2 / -1;
  min-width: 0;
  overflow: hidden;
  font-size: 11px;
  white-space: nowrap;
  text-overflow: ellipsis;
  color: var(--status-failed, #ef4444);
}

.agent-row {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.field-wrap {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 120px;
}

.field-wrap.timeout {
  min-width: 110px;
  max-width: 130px;
}

.field-wrap.grow {
  flex: 1;
}

.label {
  font-size: 11px;
  color: var(--text-muted, #64748b);
}

.field {
  height: 30px;
  padding: 0 8px;
  font-size: 12px;
  color: var(--text-primary, #f1f5f9);
  background-color: var(--bg-app-base, #0f1117);
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 4px;
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
