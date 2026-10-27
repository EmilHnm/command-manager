<template>
  <div v-if="visible" class="modal-backdrop">
    <div class="pr-modal" role="dialog" aria-modal="true">
      <div class="modal-head">
        <GitPullRequest :size="16" />
        <span class="modal-title">Push &amp; tạo PR</span>
        <span class="modal-subtitle">{{ runTitle }}</span>
        <button type="button" class="btn-close" title="Đóng" :disabled="busy" @click="emit('close')">
          <X :size="16" />
        </button>
      </div>

      <div class="modal-body">
        <div v-if="loading" class="state">Đang chuẩn bị nội dung PR…</div>
        <template v-else-if="form">
          <p class="hint">
            App push nhánh tích hợp lên <code>origin</code> với tên nhánh bên dưới (hook pre-push của dự án vẫn chạy,
            có thể mất vài phút), rồi mở PR bằng <code>gh</code>. Nhánh đã có PR thì chỉ push để cập nhật PR đó.
          </p>

          <!-- Everything is locked while pushing or while the AI writes. -->
          <fieldset class="form-set" :disabled="busy || aiBusy">
          <div class="repo-head">
            <span />
            <span>Repo</span>
            <span>Nhánh push</span>
            <span>PR vào nhánh</span>
          </div>
          <label v-for="repo in form.repos" :key="repo.repo" class="repo-row">
            <input v-model="repo.use" type="checkbox" />
            <span class="repo-name font-mono">{{ repo.repo === '.' ? 'repo' : repo.repo }}</span>
            <input v-model="repo.branch" class="field font-mono" :disabled="!repo.use" />
            <input v-model="repo.base" class="field font-mono" :disabled="!repo.use" />
          </label>
          <div v-if="form.repos.length === 0" class="state">Lượt chạy không có thay đổi nào để push.</div>

          <div class="ai-box">
            <div class="ai-row">
              <input
                v-model="aiNote"
                class="field"
                placeholder="Gợi ý cho AI (tùy chọn): mã ticket, điểm cần nhấn mạnh…"
                :disabled="aiBusy || busy"
                @keydown.enter.prevent="writeWithAi"
              />
              <button
                type="button"
                class="btn btn-secondary btn-sm"
                :disabled="aiBusy || busy || picked.length === 0"
                @click="writeWithAi"
              >
                <LoaderCircle v-if="aiBusy" :size="13" class="spin" />
                <Sparkles v-else :size="13" />
                {{ aiBusy ? 'AI đang viết…' : 'Viết bằng AI' }}
              </button>
              <button v-if="previous && !aiBusy" type="button" class="btn btn-ghost btn-sm" @click="undoAi">Hoàn tác</button>
            </div>
            <span class="ai-hint">
              Leader đọc rules, skills và template PR của repo, cùng commit / diff của lượt chạy, rồi viết tiêu đề và mô
              tả theo đúng quy ước đó (có thể mất 1–2 phút). Bạn vẫn sửa được trước khi push.
            </span>
          </div>

          <label class="block">
            <span class="label">Tiêu đề</span>
            <input v-model="form.title" class="field" />
          </label>
          <label class="block">
            <span class="label">Mô tả (Markdown)</span>
            <textarea v-model="form.body" rows="12" class="field body font-mono"></textarea>
          </label>
          <label class="check">
            <input v-model="form.draft" type="checkbox" />
            <span>Tạo dạng draft</span>
          </label>
          </fieldset>
        </template>
        <div v-if="error" class="error">{{ error }}</div>
        <div v-if="pushFailed && !busy" class="fail-actions">
          <span>Leader có thể đọc output lỗi để push lại, tự sửa hoặc giao task sửa (xong tự push lại).</span>
          <button type="button" class="btn btn-primary btn-sm" :disabled="handing" @click="handOver">
            <Wrench :size="13" /> Leader xử lý lỗi push
          </button>
        </div>

        <section v-if="steps.length" ref="progressEl" class="progress">
          <div class="progress-head">
            <span class="label">{{ busy || aiBusy ? 'Đang làm' : 'Lần chạy vừa rồi' }}</span>
          </div>
          <ol class="steps">
            <li
              v-for="step in steps"
              :key="step.id"
              class="step"
              :class="{ selected: step.id === shownStep?.id }"
              @click="picked_step = step.id"
            >
              <LoaderCircle v-if="step.running" :size="13" class="spin tone-run" />
              <CircleCheck v-else-if="step.ok" :size="13" class="tone-ok" />
              <CircleX v-else :size="13" class="tone-fail" />
              <span class="step-title">{{ step.title }}</span>
              <span class="step-time font-mono">{{ elapsed(step) }}</span>
            </li>
          </ol>
          <div class="term">
            <XtermPane
              v-if="shownStep && terminals[shownStep.id]"
              :key="terminals[shownStep.id]!.runEventId"
              :command-id="terminals[shownStep.id]!.commandId"
              :command-name="shownStep.title"
              :run-event-id="terminals[shownStep.id]!.runEventId"
              :process-status="shownStep.running ? 'running' : shownStep.ok ? 'completed' : 'failed'"
              shell-kind="agent"
              :font-size="12"
              :hide-toolbar="true"
            />
            <div v-else class="state">{{ shownStep && termErrors[shownStep.id] ? termErrors[shownStep.id] : 'Đang mở log…' }}</div>
          </div>
        </section>
      </div>

      <div class="modal-foot">
        <button type="button" class="btn btn-secondary btn-sm" :disabled="busy" @click="emit('close')">Hủy</button>
        <button type="button" class="btn btn-primary btn-sm" :disabled="busy || aiBusy || !canSubmit" @click="submit">
          <LoaderCircle v-if="busy" :size="13" class="spin" />
          <Upload v-else :size="13" />
          {{ busy ? 'Đang push & tạo PR…' : `Push & tạo PR (${picked.length} repo)` }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onUnmounted, reactive, ref, watch } from 'vue';
import { CircleCheck, CircleX, GitPullRequest, LoaderCircle, Sparkles, Upload, Wrench, X } from 'lucide-vue-next';
import { ipcClient } from '@/ipc/client';
import XtermPane from '@/components/terminal/XtermPane.vue';
import type { AgentInvocation, PrRequest, SideLog } from '@/types/agents';

const props = defineProps<{
  visible: boolean;
  runId: string | null;
  runTitle: string;
  /** The run's logs, kept live by the studio: what pushing shows. */
  sideLogs: SideLog[];
  invocations: AgentInvocation[];
}>();

const emit = defineEmits<{
  (e: 'close'): void;
  (e: 'opened', urls: string[]): void;
}>();

interface Form {
  repos: { repo: string; branch: string; base: string; use: boolean }[];
  title: string;
  body: string;
  draft: boolean;
}

const form = ref<Form | null>(null);
const loading = ref(false);
const busy = ref(false);
const error = ref<string | null>(null);
const aiNote = ref('');
const aiBusy = ref(false);
const previous = ref<{ title: string; body: string } | null>(null);

// ---- progress: the logs that started since the last push / AI writing --------

interface Step {
  id: string;
  title: string;
  started: number;
  finished: number | null;
  running: boolean;
  ok: boolean;
  /** An agent's log (reopened from its invocation) rather than a side log. */
  invocation: boolean;
}

const since = ref<number | null>(null);
const picked_step = ref<string | null>(null);
const now = ref(Math.floor(Date.now() / 1000));
const clock = window.setInterval(() => (now.value = Math.floor(Date.now() / 1000)), 1000);
onUnmounted(() => window.clearInterval(clock));

const steps = computed<Step[]>(() => {
  const from = since.value;
  if (from === null) return [];
  const side: Step[] = props.sideLogs
    .filter((log) => !log.task_id && (log.kind === 'pr' || log.kind === 'setup') && Number(log.started_at) >= from)
    .map((log) => ({
      id: log.id,
      title: log.title,
      started: Number(log.started_at),
      finished: log.finished_at ? Number(log.finished_at) : null,
      running: !log.finished_at,
      ok: !!log.ok,
      invocation: false,
    }));
  const agents: Step[] = props.invocations
    .filter((inv) => inv.step === 'describe' && Number(inv.started_at) >= from)
    .map((inv) => ({
      id: inv.id,
      title: `AI viết nội dung PR (${inv.cli})`,
      started: Number(inv.started_at),
      finished: inv.finished_at ? Number(inv.finished_at) : null,
      running: !inv.finished_at,
      ok: !!inv.ok,
      invocation: true,
    }));
  return [...side, ...agents].sort((a, b) => a.started - b.started);
});

/** The step picked, else the one running, else the last. */
const shownStep = computed(
  () =>
    steps.value.find((step) => step.id === picked_step.value) ??
    [...steps.value].reverse().find((step) => step.running) ??
    steps.value[steps.value.length - 1] ??
    null
);

const elapsed = (step: Step) => {
  const seconds = Math.max(0, (step.finished ?? now.value) - step.started);
  return seconds >= 60 ? `${Math.floor(seconds / 60)}m${String(seconds % 60).padStart(2, '0')}s` : `${seconds}s`;
};

// Brought into view when the first step shows up.
const progressEl = ref<HTMLElement | null>(null);
watch(
  () => steps.value.length > 0,
  async (shown) => {
    if (!shown) return;
    await nextTick();
    progressEl.value?.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
  }
);

const terminals = reactive<Record<string, { commandId: number; runEventId: string }>>({});
const termErrors = reactive<Record<string, string>>({});
const opening = new Set<string>();

watch(shownStep, async (step) => {
  if (!step || terminals[step.id] || opening.has(step.id)) return;
  opening.add(step.id);
  try {
    terminals[step.id] = step.invocation
      ? await ipcClient.agentsOpenLog(step.id, step.title)
      : ipcClient.agentsAttachLog(step.id, step.title);
    delete termErrors[step.id];
  } catch (e) {
    termErrors[step.id] = e instanceof Error ? e.message : String(e);
  } finally {
    opening.delete(step.id);
  }
});

// The push failed: the leader can take it from here.
const pushFailed = ref(false);
const handing = ref(false);

async function handOver() {
  if (!props.runId) return;
  handing.value = true;
  try {
    await ipcClient.agentsRunFixPush(props.runId, '');
    emit('close');
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    handing.value = false;
  }
}

const start = () => {
  pushFailed.value = false;
  since.value = Math.floor(Date.now() / 1000) - 1;
  picked_step.value = null;
};

const picked = computed(() => form.value?.repos.filter((repo) => repo.use) ?? []);
const canSubmit = computed(
  () =>
    !!form.value?.title.trim() &&
    picked.value.length > 0 &&
    picked.value.every((repo) => repo.branch.trim() && repo.base.trim())
);

watch(
  () => props.visible,
  async (visible) => {
    if (!visible || !props.runId) return;
    error.value = null;
    form.value = null;
    previous.value = null;
    aiNote.value = '';
    since.value = null;
    pushFailed.value = false;
    loading.value = true;
    try {
      const draft = await ipcClient.agentsPrDraft(props.runId);
      form.value = {
        repos: draft.repos.map((repo) => ({ ...repo, use: true })),
        title: draft.title,
        body: draft.body,
        draft: draft.draft,
      };
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e);
    } finally {
      loading.value = false;
    }
  },
  { immediate: true }
);

async function writeWithAi() {
  if (!form.value || !props.runId || aiBusy.value || picked.value.length === 0) return;
  aiBusy.value = true;
  error.value = null;
  start();
  const repos = picked.value.map(({ repo, branch, base }) => ({ repo, branch: branch.trim(), base: base.trim() }));
  try {
    const target = form.value;
    const text = await ipcClient.agentsPrAiDraft(props.runId, repos, aiNote.value.trim());
    // The modal was closed or reopened meanwhile.
    if (form.value !== target) return;
    previous.value = { title: form.value.title, body: form.value.body };
    form.value.title = text.title;
    form.value.body = text.body;
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    aiBusy.value = false;
  }
}

function undoAi() {
  if (!form.value || !previous.value) return;
  form.value.title = previous.value.title;
  form.value.body = previous.value.body;
  previous.value = null;
}

async function submit() {
  if (!form.value || !props.runId) return;
  busy.value = true;
  error.value = null;
  start();
  const request: PrRequest = {
    repos: picked.value.map(({ repo, branch, base }) => ({ repo, branch: branch.trim(), base: base.trim() })),
    title: form.value.title.trim(),
    body: form.value.body,
    draft: form.value.draft,
  };
  try {
    const run = await ipcClient.agentsRunOpenPrs(props.runId, request);
    emit(
      'opened',
      run.pull_requests.map((pr) => pr.url).filter(Boolean)
    );
    emit('close');
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
    pushFailed.value = true;
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

.pr-modal {
  width: min(760px, 100%);
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

.state {
  font-size: 12px;
  color: var(--text-muted, #64748b);
}

.repo-head,
.repo-row {
  display: grid;
  grid-template-columns: 20px minmax(120px, 1fr) 1.4fr 1fr;
  gap: 8px;
  align-items: center;
}

.repo-head {
  font-size: 11px;
  color: var(--text-muted, #64748b);
}

.repo-name {
  font-size: 12px;
  font-weight: 600;
}

.block {
  display: flex;
  flex-direction: column;
  gap: 4px;
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
}

.field.body {
  resize: vertical;
  line-height: 1.5;
}

.fail-actions {
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: 12px;
  color: var(--text-secondary, #94a3b8);
}

.form-set {
  display: flex;
  flex-direction: column;
  gap: 10px;
  min-width: 0;
  margin: 0;
  padding: 0;
  border: none;
}

.form-set:disabled {
  opacity: 0.6;
}

.progress {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.steps {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.step {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 8px;
  font-size: 12px;
  border-radius: 4px;
  cursor: pointer;
}

.step:hover,
.step.selected {
  background-color: var(--bg-app-base, #0f1117);
}

.step-title {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.step-time {
  font-size: 11px;
  color: var(--text-muted, #64748b);
}

.tone-run {
  color: #e4b5ff;
}

.tone-ok {
  color: var(--status-success, #22c55e);
}

.tone-fail {
  color: var(--status-failed, #ef4444);
}

.term {
  height: 260px;
  overflow: hidden;
  background-color: var(--bg-app-base, #0f1117);
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 6px;
}

.ai-box {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 10px;
  background-color: rgba(228, 181, 255, 0.05);
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 6px;
}

.ai-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.ai-row .field {
  flex: 1;
}

.ai-hint {
  font-size: 11px;
  line-height: 1.5;
  color: var(--text-muted, #64748b);
}

.check {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
}

.error {
  font-size: 12px;
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

@media (max-width: 600px) {
  .repo-head {
    display: none;
  }

  .repo-row {
    grid-template-columns: 20px 1fr;
  }
}
</style>
