<template>
  <div v-if="visible && task" class="modal-backdrop" @keydown.esc="emit('close')">
    <div class="reassign-modal" role="dialog" aria-modal="true" aria-labelledby="reassign-title">
      <!-- Top Accent Bar -->
      <div class="modal-accent-bar"></div>

      <!-- Header -->
      <div class="modal-header">
        <div class="header-left">
          <div class="icon-badge">
            <UserCheck :size="16" />
          </div>
          <div class="header-titles">
            <h3 id="reassign-title" class="modal-title">
              Giao Lại Task: <span class="font-mono text-primary">{{ task.key }}</span>
            </h3>
            <span class="modal-subtitle">
              Mở phiên làm việc mới trên cùng git worktree của task này
            </span>
          </div>
        </div>
        <button type="button" class="btn-close" title="Đóng (Esc)" @click="emit('close')">
          <X :size="16" />
        </button>
      </div>

      <!-- Body -->
      <div class="modal-body custom-scrollbar">
        <!-- Task current info pill -->
        <div class="current-task-pill font-mono">
          <span class="pill-label">Hiện tại:</span>
          <span class="pill-val">{{ task.cli }}</span>
          <span v-if="task.model" class="pill-model">({{ task.model }})</span>
          <span class="pill-divider">•</span>
          <span class="pill-role">Vai: {{ task.role }}</span>
          <span class="pill-divider">•</span>
          <span class="pill-worktree" :title="task.worktree || ''">Worktree: {{ task.worktree ? task.worktree.split('/').slice(-2).join('/') : 'chưa tạo' }}</span>
        </div>

        <p class="reassign-hint">
          Task sẽ tiếp tục làm việc trên worktree hiện tại (các tệp đã sửa vẫn giữ nguyên). Lần chạy tới sẽ dùng agent và thiết lập mới này.
        </p>

        <div class="form-grid">
          <div class="form-group">
            <label class="form-label required">Agent CLI</label>
            <div class="select-wrapper">
              <select v-model="cli" class="form-select font-mono" @change="onCliChange">
                <option v-for="item in AGENT_CLIS" :key="item" :value="item" :disabled="!installed(item)">
                  {{ item }}{{ installed(item) ? '' : ' (chưa cài đặt)' }}
                </option>
              </select>
            </div>
          </div>

          <div class="form-group">
            <label class="form-label">Model (Mô hình)</label>
            <div class="input-wrapper">
              <input
                v-model="model"
                class="form-input font-mono"
                :list="`task-models-${cli}`"
                :placeholder="profileModelHint(clis.find((info) => info.cli === cli)?.profile, 'work')"
              />
              <datalist :id="`task-models-${cli}`">
                <option v-for="item in modelsOf(cli)" :key="item.id" :value="item.id">{{ item.label }}</option>
              </datalist>
            </div>
          </div>

          <div class="form-group">
            <label class="form-label">Effort</label>
            <div class="select-wrapper">
              <select v-model="effort" class="form-select font-mono" :disabled="cli === 'cursor'">
                <option value="">mặc định</option>
                <option v-for="level in EFFORT_LEVELS" :key="level" :value="level">{{ level }}</option>
              </select>
            </div>
          </div>

          <div class="form-group">
            <label class="form-label">Timeout (phút)</label>
            <div class="input-wrapper">
              <input
                v-model.number="timeout"
                type="number"
                min="1"
                :max="MAX_TIMEOUT_MINUTES"
                step="5"
                class="form-input font-mono"
                :placeholder="defaultTimeoutPlaceholder"
              />
            </div>
          </div>
        </div>

        <div v-if="error" class="error-banner">
          <AlertCircle :size="14" />
          <span>{{ error }}</span>
        </div>
      </div>

      <!-- Footer -->
      <div class="modal-footer">
        <button type="button" class="btn btn-secondary btn-sm" :disabled="busy" @click="emit('close')">
          Hủy
        </button>
        <button
          type="button"
          class="btn btn-primary btn-sm btn-confirm"
          :disabled="busy"
          @click="handleSubmit"
        >
          <UserCheck :size="14" />
          <span>{{ busy ? 'Đang giao lại…' : `Giao lại cho ${cli}` }}</span>
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { AlertCircle, UserCheck, X } from 'lucide-vue-next';
import type { AgentCli, AgentTask, CliInfo } from '@/types/agents';
import { AGENT_CLIS, EFFORT_LEVELS, MAX_TIMEOUT_MINUTES, profileModelHint } from '@/types/agents';
import { useAgentModels } from '@/composables/useAgentModels';

const props = defineProps<{
  visible: boolean;
  task: AgentTask | null;
  clis: CliInfo[];
  defaultTimeout?: number;
  submit: (
    cli: AgentCli,
    model: string | null,
    effort: string | null,
    timeoutMinutes: number | null
  ) => Promise<void>;
}>();

const emit = defineEmits<{
  (e: 'close'): void;
}>();

const { modelsOf } = useAgentModels();

const cli = ref<AgentCli>('claude');
const model = ref('');
const effort = ref('');
const timeout = ref<number | ''>('');
const busy = ref(false);
const error = ref<string | null>(null);

const installed = (agentCli: AgentCli) => props.clis.some((info) => info.cli === agentCli && !!info.path);

const defaultTimeoutPlaceholder = computed(() =>
  props.defaultTimeout ? `${props.defaultTimeout} (của lượt chạy)` : 'của lượt chạy'
);

watch(
  () => [props.visible, props.task] as const,
  ([visible, task]) => {
    if (visible && task) {
      cli.value = task.cli;
      model.value = task.model || '';
      effort.value = '';
      timeout.value = task.timeout_minutes ?? '';
      error.value = null;
      busy.value = false;
    }
  },
  { immediate: true }
);

const onCliChange = () => {
  model.value = '';
  effort.value = '';
};

const handleSubmit = async () => {
  if (!props.task) return;
  busy.value = true;
  error.value = null;
  try {
    const finalModel = model.value.trim() || null;
    const finalEffort = effort.value.trim() || null;
    const finalTimeout = typeof timeout.value === 'number' && timeout.value > 0 ? timeout.value : null;
    await props.submit(cli.value, finalModel, finalEffort, finalTimeout);
    emit('close');
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err);
  } finally {
    busy.value = false;
  }
};
</script>

<style scoped>
.modal-backdrop {
  position: fixed;
  inset: 0;
  background-color: rgba(11, 13, 19, 0.78);
  backdrop-filter: blur(8px);
  z-index: 1050;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 16px;
}

.reassign-modal {
  background-color: var(--color-bg-surface, #1a1e2b);
  border: 1px solid var(--color-border-subtle, #242a3e);
  border-radius: 12px;
  width: 100%;
  max-width: 540px;
  box-shadow: 0 20px 45px rgba(0, 0, 0, 0.5);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  animation: modal-enter 0.15s ease-out;
}

@keyframes modal-enter {
  from {
    opacity: 0;
    transform: scale(0.96);
  }
  to {
    opacity: 1;
    transform: scale(1);
  }
}

.modal-accent-bar {
  height: 3px;
  background: linear-gradient(90deg, #744791 0%, #4cd7f6 100%);
}

.modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 20px;
  background-color: var(--color-bg-sidebar, #111319);
  border-bottom: 1px solid var(--color-border-subtle, #242a3e);
}

.header-left {
  display: flex;
  align-items: center;
  gap: 12px;
}

.icon-badge {
  width: 32px;
  height: 32px;
  border-radius: 8px;
  background-color: rgba(116, 71, 145, 0.2);
  color: var(--color-primary, #bd85e8);
  display: flex;
  align-items: center;
  justify-content: center;
}

.header-titles {
  display: flex;
  flex-direction: column;
}

.modal-title {
  font-size: 15px;
  font-weight: 600;
  color: var(--color-text-primary, #e2e2eb);
  margin: 0;
  line-height: 1.3;
}

.modal-subtitle {
  font-size: 11px;
  color: var(--color-text-secondary, #94a3b8);
  margin-top: 2px;
}

.btn-close {
  background: transparent;
  border: none;
  color: var(--color-text-secondary, #94a3b8);
  cursor: pointer;
  padding: 4px;
  border-radius: 4px;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.15s ease;
}

.btn-close:hover {
  background-color: var(--color-bg-surface-hover, #232738);
  color: var(--color-text-primary, #e2e2eb);
}

.modal-body {
  padding: 18px 20px;
  display: flex;
  flex-direction: column;
  gap: 14px;
  max-height: 70vh;
  overflow-y: auto;
}

.current-task-pill {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
  padding: 8px 12px;
  background-color: var(--color-bg-app-base, #0f1117);
  border: 1px solid var(--color-border-subtle, #242a3e);
  border-radius: 6px;
  font-size: 12px;
}

.pill-label {
  color: var(--color-text-muted, #64748b);
}

.pill-val {
  color: var(--color-text-primary, #e2e2eb);
  font-weight: 600;
}

.pill-model {
  color: var(--color-text-secondary, #94a3b8);
}

.pill-divider {
  color: var(--color-border-subtle, #242a3e);
}

.pill-role {
  color: #4cd7f6;
}

.pill-worktree {
  color: var(--color-text-muted, #64748b);
  font-size: 11px;
}

.reassign-hint {
  font-size: 12px;
  color: var(--color-text-secondary, #94a3b8);
  line-height: 1.5;
  margin: 0;
}

.form-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
}

.form-group {
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.form-label {
  font-size: 11px;
  font-weight: 500;
  color: var(--color-text-secondary, #94a3b8);
}

.form-label.required::after {
  content: ' *';
  color: #ef4444;
}

.select-wrapper,
.input-wrapper {
  position: relative;
  width: 100%;
}

.form-select,
.form-input {
  width: 100%;
  padding: 7px 10px;
  background-color: var(--color-bg-app-base, #0f1117);
  border: 1px solid var(--color-border-subtle, #242a3e);
  border-radius: 6px;
  color: var(--color-text-primary, #e2e2eb);
  font-size: 12px;
  outline: none;
  transition: border-color 0.15s ease;
}

.form-select:focus,
.form-input:focus {
  border-color: var(--color-border-focus, #744791);
}

.error-banner {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  background-color: rgba(239, 68, 68, 0.12);
  border: 1px solid rgba(239, 68, 68, 0.3);
  border-radius: 6px;
  color: #fca5a5;
  font-size: 12px;
}

.modal-footer {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 10px;
  padding: 12px 20px;
  background-color: var(--color-bg-sidebar, #111319);
  border-top: 1px solid var(--color-border-subtle, #242a3e);
}

.btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  font-weight: 500;
  padding: 6px 14px;
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.15s ease;
  border: 1px solid transparent;
}

.btn-secondary {
  background-color: var(--color-bg-surface, #1a1e2b);
  border-color: var(--color-border-subtle, #242a3e);
  color: var(--color-text-primary, #e2e2eb);
}

.btn-secondary:hover:not(:disabled) {
  background-color: var(--color-bg-surface-hover, #232738);
}

.btn-primary {
  background-color: #744791;
  color: #f1f5f9;
}

.btn-primary:hover:not(:disabled) {
  background-color: #8b5aa9;
}

.btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
</style>
