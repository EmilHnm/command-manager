<template>
  <div v-if="visible" class="modal-backdrop">
    <div class="qa-modal" role="dialog" aria-modal="true">
      <div class="modal-head">
        <Search :size="16" />
        <span class="modal-title">{{ editable ? 'Cổng 0 — QA của leader' : 'Báo cáo tìm hiểu' }}</span>
        <span class="modal-subtitle">{{ run?.title }}</span>
        <button type="button" class="btn-close" title="Đóng" :disabled="busy" @click="emit('close')">
          <X :size="16" />
        </button>
      </div>

      <div v-if="report" class="modal-body">
        <section class="block">
          <span class="label">Tóm tắt</span>
          <p class="summary">{{ report.summary }}</p>
        </section>

        <section v-if="report.findings.length" class="block">
          <button type="button" class="toggle" @click="showFindings = !showFindings">
            <ChevronDown v-if="showFindings" :size="12" />
            <ChevronRight v-else :size="12" />
            Đã kiểm tra ({{ counts }})
          </button>
          <ul v-if="showFindings" class="findings">
            <li v-for="(finding, index) in report.findings" :key="index" class="finding">
              <span class="status" :class="'st-' + finding.status">{{ FINDING_LABELS[finding.status] ?? finding.status }}</span>
              <div class="finding-body">
                <span class="finding-title">{{ finding.title }}</span>
                <span v-if="finding.detail" class="muted">{{ finding.detail }}</span>
                <span v-if="finding.source" class="source font-mono">{{ finding.source }}</span>
              </div>
            </li>
          </ul>
        </section>

        <section class="block">
          <span class="label">QA ({{ report.qa.length }})</span>
          <div v-if="report.qa.length === 0" class="muted">Leader không có câu hỏi nào.</div>
          <article v-for="qa in report.qa" :key="qa.id" class="qa" :class="{ blocker: qa.severity === 'blocker' }">
            <header class="qa-head">
              <span class="qa-id font-mono">{{ qa.id }}</span>
              <span class="badge" :class="qa.severity === 'blocker' ? 'b-block' : 'b-ask'">
                {{ qa.severity === 'blocker' ? 'Chặn' : 'Nên hỏi' }}
              </span>
              <span class="qa-title">{{ qa.title }}</span>
            </header>
            <p v-if="qa.detail" class="qa-detail">{{ qa.detail }}</p>
            <p class="qa-question"><strong>Câu hỏi:</strong> {{ qa.question }}</p>
            <p v-if="qa.assumption" class="muted"><strong>Giả định nếu chưa có trả lời:</strong> {{ qa.assumption }}</p>
            <p v-if="qa.source" class="source font-mono">{{ qa.source }}</p>
            <template v-if="editable">
              <textarea
                v-model="answers[qa.id]"
                class="field answer"
                rows="3"
                :disabled="busy"
                :placeholder="qa.severity === 'blocker' ? 'Câu trả lời (bắt buộc)…' : 'Câu trả lời (để trống: leader làm theo giả định)…'"
              ></textarea>
              <div v-if="qa.assumption" class="qa-actions">
                <button type="button" class="link" :disabled="busy" @click="useAssumption(qa)">Dùng giả định của leader</button>
              </div>
            </template>
            <p v-else class="qa-answer">
              <strong>Trả lời:</strong> {{ qa.answer ?? `(chưa có — theo giả định: ${qa.assumption})` }}
            </p>
          </article>
        </section>

        <section v-if="editable" class="block">
          <button type="button" class="toggle" @click="showAgain = !showAgain">
            <ChevronDown v-if="showAgain" :size="12" />
            <ChevronRight v-else :size="12" />
            Nhờ leader tìm hiểu lại
          </button>
          <textarea
            v-if="showAgain"
            v-model="feedback"
            class="field"
            rows="3"
            :disabled="busy"
            placeholder="Ví dụ: API-361 đã merge ở PR #12, kiểm tra lại; xem thêm docs/04_api/…"
          ></textarea>
        </section>

        <div v-if="error" class="error">{{ error }}</div>
      </div>
      <div v-else class="modal-body muted">Leader chưa có báo cáo tìm hiểu.</div>

      <div class="modal-foot">
        <button type="button" class="btn btn-secondary btn-sm" :disabled="busy" @click="emit('close')">Đóng</button>
        <template v-if="editable">
          <button
            v-if="showAgain"
            type="button"
            class="btn btn-secondary btn-sm"
            :disabled="busy || !feedback.trim()"
            @click="again"
          >
            <RotateCcw :size="13" /> Tìm hiểu lại
          </button>
          <button type="button" class="btn btn-primary btn-sm" :disabled="busy || missing.length > 0" :title="missingTitle" @click="approve">
            <LoaderCircle v-if="busy" :size="13" class="spin" />
            <Check v-else :size="13" />
            Trả lời & thiết kế
          </button>
        </template>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue';
import { Check, ChevronDown, ChevronRight, LoaderCircle, RotateCcw, Search, X } from 'lucide-vue-next';
import { ipcClient } from '@/ipc/client';
import type { AgentRun, QaAnswer, QaItem } from '@/types/agents';

const props = defineProps<{
  visible: boolean;
  run: AgentRun | null;
}>();

const emit = defineEmits<{
  (e: 'close'): void;
  (e: 'done'): void;
}>();

const FINDING_LABELS: Record<string, string> = {
  ready: 'Sẵn sàng',
  missing: 'Thiếu',
  unclear: 'Chưa rõ',
};

const report = computed(() => props.run?.research?.report ?? null);
const editable = computed(() => props.run?.stage === 'qa_gate');

const answers = reactive<Record<string, string>>({});
const feedback = ref('');
const showAgain = ref(false);
const showFindings = ref(false);
const busy = ref(false);
const error = ref<string | null>(null);

watch(
  () => props.visible,
  (visible) => {
    if (!visible) return;
    error.value = null;
    feedback.value = '';
    showAgain.value = false;
    for (const key of Object.keys(answers)) delete answers[key];
    for (const qa of report.value?.qa ?? []) answers[qa.id] = qa.answer ?? '';
    // Nothing missing or unclear: the list is noise.
    showFindings.value = (report.value?.findings ?? []).some((f) => f.status !== 'ready');
  },
  { immediate: true }
);

const counts = computed(() => {
  const findings = report.value?.findings ?? [];
  const n = (status: string) => findings.filter((f) => f.status === status).length;
  return `${n('ready')} sẵn sàng · ${n('missing')} thiếu · ${n('unclear')} chưa rõ`;
});

const missing = computed(() =>
  (report.value?.qa ?? []).filter((qa) => qa.severity === 'blocker' && !answers[qa.id]?.trim()).map((qa) => qa.id)
);
const missingTitle = computed(() =>
  missing.value.length ? `Còn câu chặn chưa trả lời: ${missing.value.join(', ')}` : undefined
);

const collected = (): QaAnswer[] =>
  (report.value?.qa ?? []).map((qa) => ({ id: qa.id, answer: answers[qa.id]?.trim() ?? '' }));

const useAssumption = (qa: QaItem) => {
  answers[qa.id] = `Đồng ý giả định: ${qa.assumption}`;
};

async function act(action: () => Promise<unknown>) {
  if (!props.run) return;
  busy.value = true;
  error.value = null;
  try {
    await action();
    emit('done');
    emit('close');
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    busy.value = false;
  }
}

const approve = () => act(() => ipcClient.agentsResearchAnswer(props.run!.id, collected()));
const again = () => act(() => ipcClient.agentsResearchAgain(props.run!.id, feedback.value.trim(), collected()));
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

.qa-modal {
  width: min(860px, 100%);
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

.modal-subtitle {
  min-width: 0;
  overflow: hidden;
  font-size: 12px;
  white-space: nowrap;
  text-overflow: ellipsis;
  color: var(--text-muted, #64748b);
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
  gap: 14px;
  padding: 14px;
  overflow-y: auto;
  font-size: 12px;
}

.block {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.label {
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: var(--text-muted, #64748b);
}

.summary {
  margin: 0;
  line-height: 1.6;
  white-space: pre-wrap;
}

.muted {
  margin: 0;
  line-height: 1.5;
  color: var(--text-secondary, #94a3b8);
}

.toggle {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 0;
  font-size: 12px;
  color: var(--text-secondary, #94a3b8);
  background: none;
  border: none;
  cursor: pointer;
}

.findings {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.finding {
  display: flex;
  align-items: flex-start;
  gap: 8px;
}

.finding-body {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.finding-title {
  font-weight: 500;
}

.status {
  flex-shrink: 0;
  min-width: 64px;
  padding: 1px 6px;
  font-size: 11px;
  text-align: center;
  border-radius: 4px;
}

.st-ready {
  color: #22c55e;
  background-color: rgba(34, 197, 94, 0.12);
}

.st-missing {
  color: #ef4444;
  background-color: rgba(239, 68, 68, 0.12);
}

.st-unclear {
  color: #f59e0b;
  background-color: rgba(245, 158, 11, 0.12);
}

.source {
  margin: 0;
  overflow-wrap: anywhere;
  font-size: 11px;
  color: var(--text-muted, #64748b);
}

.qa {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 10px 12px;
  background-color: var(--bg-app-base, #0f1117);
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 6px;
}

.qa.blocker {
  border-color: rgba(239, 68, 68, 0.45);
}

.qa-head {
  display: flex;
  align-items: center;
  gap: 8px;
}

.qa-id {
  font-size: 11px;
  color: var(--text-muted, #64748b);
}

.qa-title {
  font-weight: 600;
}

.badge {
  padding: 1px 6px;
  font-size: 11px;
  border-radius: 4px;
}

.b-block {
  color: #ef4444;
  background-color: rgba(239, 68, 68, 0.12);
}

.b-ask {
  color: #e4b5ff;
  background-color: rgba(228, 181, 255, 0.12);
}

.qa-detail,
.qa-question,
.qa-answer {
  margin: 0;
  line-height: 1.5;
  white-space: pre-wrap;
}

.field {
  width: 100%;
  padding: 8px;
  font-size: 12px;
  line-height: 1.5;
  resize: vertical;
  color: var(--text-primary, #f1f5f9);
  background-color: var(--surface-container, #1a1e2b);
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 4px;
}

.qa-actions {
  display: flex;
  justify-content: flex-end;
}

.link {
  padding: 0;
  font-size: 11px;
  color: #e4b5ff;
  background: none;
  border: none;
  cursor: pointer;
}

.error {
  color: var(--status-failed, #ef4444);
  white-space: pre-wrap;
}

.modal-foot {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 12px 14px;
  border-top: 1px solid var(--border-subtle, #242a3e);
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
