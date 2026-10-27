<template>
  <section class="push-failure">
    <header class="pf-head">
      <AlertTriangle :size="14" />
      <span class="pf-title">Push lỗi</span>
      <span class="pf-meta font-mono">{{ target }} · {{ when }}</span>
    </header>
    <div class="pf-error">{{ failure.error }}</div>

    <div v-if="failure.diagnosis" class="pf-diagnosis">
      <span class="pf-badge">{{ ACTIONS[failure.action ?? ''] ?? 'Leader' }}</span>
      <span>{{ failure.diagnosis }}</span>
    </div>
    <div v-if="failure.retry && failure.fix_tasks.length" class="pf-hint">
      Đang sửa qua task {{ failure.fix_tasks.join(', ') }} — merge xong và kiểm tra tích hợp qua thì app tự push lại.
    </div>

    <button type="button" class="pf-toggle" @click="showOutput = !showOutput">
      <ChevronDown v-if="showOutput" :size="12" />
      <ChevronRight v-else :size="12" />
      Output của lần push ({{ lines }} dòng cuối)
    </button>
    <pre v-if="showOutput" class="pf-output font-mono">{{ failure.output || '(không có output)' }}</pre>

    <div v-if="driving" class="pf-hint">
      <LoaderCircle :size="12" class="spin" /> Leader đang xử lý… xem terminal ở lưới agent bên dưới.
    </div>
    <div v-else-if="!failure.retry" class="pf-actions">
      <input
        v-model="note"
        class="pf-note"
        placeholder="Ghi chú cho leader (tùy chọn): test này hay flaky, đừng sửa file X…"
        :disabled="busy"
        @keydown.enter.prevent="handOver"
      />
      <button type="button" class="btn btn-primary btn-sm" :disabled="busy" @click="handOver">
        <Wrench :size="13" />
        <span>Leader xử lý lỗi push</span>
      </button>
      <button type="button" class="btn btn-ghost btn-sm" :disabled="busy" title="Ẩn lỗi push này" @click="dismiss">
        Bỏ qua
      </button>
    </div>
    <div class="pf-hint small">
      Leader đọc output và code rồi chọn: push lại (lỗi flaky / môi trường), tự sửa ngay trong worktree tích hợp rồi push,
      hoặc tạo task sửa cho đội (xong tự push lại). Không sửa được thì leader giải thích để bạn quyết định.
    </div>
    <div v-if="error" class="pf-err">{{ error }}</div>
  </section>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue';
import { AlertTriangle, ChevronDown, ChevronRight, LoaderCircle, Wrench } from 'lucide-vue-next';
import { ipcClient } from '@/ipc/client';
import type { PushFailure } from '@/types/agents';

const props = defineProps<{
  runId: string;
  failure: PushFailure;
  driving: boolean;
}>();

const ACTIONS: Record<string, string> = {
  retry: 'Push lại',
  self: 'Leader tự sửa',
  tasks: 'Giao task sửa',
  none: 'Cần bạn quyết định',
};

const note = ref('');
const busy = ref(false);
const error = ref<string | null>(null);
const showOutput = ref(false);

const target = computed(() =>
  props.failure.request.repos.map((pr) => `${pr.repo === '.' ? '' : pr.repo + ': '}${pr.branch} → ${pr.base}`).join(', ')
);
const when = computed(() => new Date(Number(props.failure.at) * 1000).toLocaleString());
const lines = computed(() => (props.failure.output ? props.failure.output.split('\n').length : 0));

async function run(action: () => Promise<unknown>) {
  busy.value = true;
  error.value = null;
  try {
    await action();
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    busy.value = false;
  }
}

const handOver = () =>
  run(async () => {
    await ipcClient.agentsRunFixPush(props.runId, note.value.trim());
    note.value = '';
  });

const dismiss = () => run(() => ipcClient.agentsPushFailureDismiss(props.runId));
</script>

<style scoped>
.push-failure {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-top: 10px;
  padding: 10px 12px;
  font-size: 12px;
  background-color: rgba(239, 68, 68, 0.06);
  border: 1px solid rgba(239, 68, 68, 0.35);
  border-radius: 6px;
}

.pf-head {
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--status-failed, #ef4444);
}

.pf-title {
  font-weight: 600;
}

.pf-meta {
  min-width: 0;
  overflow: hidden;
  font-size: 11px;
  white-space: nowrap;
  text-overflow: ellipsis;
  color: var(--text-muted, #64748b);
}

.pf-error {
  white-space: pre-wrap;
  color: var(--text-primary, #f1f5f9);
}

.pf-diagnosis {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  line-height: 1.5;
  white-space: pre-wrap;
  color: var(--text-secondary, #94a3b8);
}

.pf-badge {
  flex-shrink: 0;
  padding: 1px 6px;
  font-size: 11px;
  border-radius: 4px;
  color: #e4b5ff;
  background-color: rgba(228, 181, 255, 0.12);
}

.pf-toggle {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 0;
  font-size: 11px;
  color: var(--text-secondary, #94a3b8);
  background: none;
  border: none;
  cursor: pointer;
}

.pf-output {
  max-height: 280px;
  margin: 0;
  padding: 8px;
  overflow: auto;
  font-size: 11px;
  line-height: 1.45;
  white-space: pre-wrap;
  word-break: break-word;
  color: var(--text-secondary, #94a3b8);
  background-color: var(--bg-app-base, #0f1117);
  border-radius: 4px;
}

.pf-actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
}

.pf-note {
  flex: 1;
  min-width: 220px;
  padding: 6px 8px;
  font-size: 12px;
  color: var(--text-primary, #f1f5f9);
  background-color: var(--bg-app-base, #0f1117);
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 4px;
}

.pf-hint {
  display: flex;
  align-items: center;
  gap: 6px;
  line-height: 1.5;
  color: var(--text-secondary, #94a3b8);
}

.pf-hint.small {
  font-size: 11px;
  color: var(--text-muted, #64748b);
}

.pf-err {
  color: var(--status-failed, #ef4444);
}

.spin {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
