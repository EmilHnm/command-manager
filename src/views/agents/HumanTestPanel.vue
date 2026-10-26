<template>
  <div class="human-panel" @paste="onPaste">
    <div class="human-head">
      <UserCheck :size="14" />
      <strong>Test thủ công</strong>
      <span class="human-status" :class="task.status">{{ TASK_STATUS_LABELS[task.status] }}</span>
      <span v-if="rounds.length" class="text-muted">vòng {{ rounds.length + (task.status === 'passed' ? 0 : 1) }}</span>
      <span class="spacer" />
      <button
        v-if="awaiting"
        type="button"
        class="btn btn-ghost btn-xs"
        title="AI chuẩn bị lại theo mô tả"
        :disabled="busy"
        @click="reprepare"
      >
        <RotateCcw :size="12" /> Tạo lại hướng dẫn
      </button>
    </div>

    <details class="human-desc">
      <summary>Mô tả cho AI</summary>
      <p>{{ task.description }}</p>
    </details>

    <div v-if="!guide" class="human-wait">
      {{
        task.status === 'triage'
          ? 'Leader đang xác nhận lỗi và lập task sửa…'
          : task.status === 'waiting'
            ? 'Chờ các task cần test (và task sửa lỗi) xong, rồi AI chuẩn bị hướng dẫn.'
            : 'AI đang chuẩn bị…'
      }}
    </div>

    <template v-else>
      <div v-if="human?.leader_note" class="leader-note">
        <strong>Leader:</strong> {{ human.leader_note }}
      </div>
      <p class="guide-summary">{{ guide.summary }}</p>
      <div v-if="guide.setup.length" class="guide-block">
        <span class="guide-label">Chuẩn bị</span>
        <ol>
          <li v-for="(step, index) in guide.setup" :key="index">{{ step }}</li>
        </ol>
      </div>
      <div class="cases">
        <div v-for="item in guide.cases" :key="item.id" class="case" :class="caseState(item.id)">
          <div class="case-head">
            <span class="case-id font-mono">{{ item.id }}</span>
            <span class="case-title">{{ item.title }}</span>
            <span class="spacer" />
            <div class="case-verdict">
              <button
                type="button"
                class="verdict pass"
                :class="{ on: draft[item.id]?.pass === true }"
                :disabled="!awaiting"
                @click="setPass(item.id, true)"
              >
                Đạt
              </button>
              <button
                type="button"
                class="verdict fail"
                :class="{ on: draft[item.id]?.pass === false }"
                :disabled="!awaiting"
                @click="setPass(item.id, false)"
              >
                Lỗi
              </button>
            </div>
          </div>
          <ol class="case-steps">
            <li v-for="(step, index) in item.steps" :key="index">{{ step }}</li>
          </ol>
          <div class="case-expected"><span class="guide-label">Mong đợi:</span> {{ item.expected }}</div>
          <input
            v-if="awaiting && draft[item.id]"
            v-model="draft[item.id].note"
            class="field case-note"
            :placeholder="draft[item.id].pass === false ? 'Thấy gì? (bắt buộc khi lỗi)' : 'Ghi chú (tùy chọn)'"
          />
        </div>
      </div>
      <p v-if="guide.notes" class="guide-notes">{{ guide.notes }}</p>

      <template v-if="awaiting">
        <label class="guide-label">Nhận xét chung</label>
        <textarea v-model="note" rows="3" class="field" placeholder="Nhận xét, môi trường đã test, lỗi khác ngoài các case…"></textarea>
        <div class="shots">
          <div v-for="name in attachments" :key="name" class="shot">
            <img v-if="previews[name]" :src="previews[name]" :alt="name" @click="zoom = previews[name]" />
            <button type="button" class="shot-remove" title="Bỏ ảnh" @click="removeShot(name)">
              <X :size="11" />
            </button>
          </div>
          <label class="shot-add" title="Chọn ảnh, hoặc dán (Ctrl+V) vào khung này">
            <ImagePlus :size="16" />
            <span>Ảnh</span>
            <input type="file" accept="image/*" multiple hidden @change="onPick" />
          </label>
        </div>
        <div class="submit-row">
          <span class="text-muted">{{ triedCount }}/{{ guide.cases.length }} case đã đánh giá</span>
          <span class="spacer" />
          <button type="button" class="btn btn-danger-ghost btn-sm" :disabled="busy" @click="submit(false)">
            <XCircle :size="13" /> Không đạt — gửi leader
          </button>
          <button type="button" class="btn btn-primary btn-sm" :disabled="busy" @click="submit(true)">
            <CheckCircle2 :size="13" /> Đạt
          </button>
        </div>
      </template>
    </template>
    <div v-if="error" class="error">{{ error }}</div>

    <!-- Chat with the leader -->
    <div class="chat">
      <div class="guide-label">Hỏi leader khi vướng</div>
      <div ref="chatBox" class="chat-log custom-scrollbar">
        <div v-if="!chat.length" class="text-muted">Chưa có câu hỏi nào.</div>
        <div v-for="(message, index) in chat" :key="index" class="chat-msg" :class="message.role">
          <span class="chat-who">{{ message.role === 'user' ? 'Bạn' : 'Leader' }}</span>
          <span class="chat-text">{{ message.text }}</span>
        </div>
        <div v-if="asking" class="chat-msg leader"><span class="chat-who">Leader</span> đang trả lời…</div>
      </div>
      <div class="chat-input">
        <textarea
          v-model="question"
          rows="2"
          class="field"
          placeholder="Ví dụ: Đăng nhập bằng tài khoản nào để thấy màn này?"
          @keydown.ctrl.enter="ask"
          @keydown.meta.enter="ask"
        ></textarea>
        <button type="button" class="btn btn-secondary btn-sm" :disabled="asking || !question.trim()" @click="ask">
          <Send :size="12" /> Hỏi
        </button>
      </div>
    </div>

    <!-- Earlier rounds -->
    <details v-if="rounds.length" class="rounds">
      <summary>Các vòng trước ({{ rounds.length }})</summary>
      <div v-for="(round, index) in rounds" :key="round.at" class="round">
        <strong>Vòng {{ index + 1 }}: {{ round.passed ? '✓ Đạt' : '✗ Không đạt' }}</strong>
        <span v-if="round.fix_tasks.length" class="text-muted"> — task sửa: {{ round.fix_tasks.join(', ') }}</span>
        <div v-for="result in round.results.filter((r) => r.pass === false)" :key="result.case_id" class="round-fail">
          {{ result.case_id }}: {{ result.note }}
        </div>
        <div v-if="round.note" class="round-note">{{ round.note }}</div>
        <div v-if="round.attachments.length" class="shots">
          <div v-for="name in round.attachments" :key="name" class="shot">
            <img v-if="previews[name]" :src="previews[name]" :alt="name" @click="zoom = previews[name]" />
          </div>
        </div>
      </div>
    </details>

    <div v-if="zoom" class="zoom" @click="zoom = null">
      <img :src="zoom" alt="" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, reactive, ref, watch } from 'vue';
import { CheckCircle2, ImagePlus, RotateCcw, Send, UserCheck, X, XCircle } from 'lucide-vue-next';
import { ipcClient } from '@/ipc/client';
import type { AgentTask, CaseResult } from '@/types/agents';
import { TASK_STATUS_LABELS } from '@/types/agents';

const props = defineProps<{
  task: AgentTask;
}>();

const emit = defineEmits<{
  (e: 'changed'): void;
}>();

const human = computed(() => props.task.human);
const guide = computed(() => human.value?.guide ?? null);
const rounds = computed(() => human.value?.rounds ?? []);
const chat = computed(() => human.value?.chat ?? []);
const awaiting = computed(() => props.task.status === 'awaiting_human');

// The person's draft for the round; kept while the view reloads.
const draft = reactive<Record<string, { pass: boolean | null; note: string }>>({});
const note = ref('');
const attachments = ref<string[]>([]);
const previews = reactive<Record<string, string>>({});
const busy = ref(false);
const error = ref<string | null>(null);
const question = ref('');
const asking = ref(false);
const zoom = ref<string | null>(null);
const chatBox = ref<HTMLElement | null>(null);

// A new task or a new guide (next round) starts a new draft.
watch(
  () => `${props.task.id}:${JSON.stringify(guide.value?.cases.map((c) => c.id) ?? [])}:${rounds.value.length}`,
  () => {
    for (const key of Object.keys(draft)) delete draft[key];
    for (const item of guide.value?.cases ?? []) draft[item.id] = { pass: null, note: '' };
    note.value = '';
    attachments.value = [];
    error.value = null;
  },
  { immediate: true }
);

// Thumbnails of the screenshots of earlier rounds.
watch(
  rounds,
  (list) => {
    for (const round of list) for (const name of round.attachments) void loadPreview(name);
  },
  { immediate: true }
);

watch(
  () => chat.value.length,
  async () => {
    await nextTick();
    chatBox.value?.scrollTo({ top: chatBox.value.scrollHeight });
  }
);

const triedCount = computed(() => Object.values(draft).filter((item) => item.pass !== null).length);

const caseState = (id: string) => (draft[id]?.pass === true ? 'ok' : draft[id]?.pass === false ? 'bad' : '');

const setPass = (id: string, pass: boolean) => {
  const item = draft[id] ?? (draft[id] = { pass: null, note: '' });
  item.pass = item.pass === pass ? null : pass;
};

async function loadPreview(name: string) {
  if (previews[name]) return;
  try {
    const data = await ipcClient.agentsHumanFile(props.task.id, name);
    const ext = name.split('.').pop()?.toLowerCase() ?? 'png';
    const mime = ext === 'jpg' || ext === 'jpeg' ? 'image/jpeg' : ext === 'gif' ? 'image/gif' : ext === 'webp' ? 'image/webp' : 'image/png';
    previews[name] = `data:${mime};base64,${data}`;
  } catch {
    // No preview; the file is still attached.
  }
}

const toBase64 = (file: Blob) =>
  new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result).replace(/^data:[^,]*,/, ''));
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });

async function addFiles(files: File[]) {
  if (!awaiting.value) return;
  error.value = null;
  for (const file of files.filter((item) => item.type.startsWith('image/'))) {
    try {
      const name = await ipcClient.agentsHumanAttach(
        props.task.id,
        file.name || `screenshot.${file.type.split('/')[1] ?? 'png'}`,
        await toBase64(file)
      );
      attachments.value.push(name);
      void loadPreview(name);
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e);
    }
  }
}

function onPaste(event: ClipboardEvent) {
  const files = [...(event.clipboardData?.files ?? [])];
  if (files.some((file) => file.type.startsWith('image/'))) {
    event.preventDefault();
    void addFiles(files);
  }
}

function onPick(event: Event) {
  const input = event.target as HTMLInputElement;
  void addFiles([...(input.files ?? [])]);
  input.value = '';
}

const removeShot = (name: string) => {
  attachments.value = attachments.value.filter((item) => item !== name);
};

async function submit(passed: boolean) {
  busy.value = true;
  error.value = null;
  const results: CaseResult[] = Object.entries(draft).map(([case_id, item]) => ({
    case_id,
    pass: item.pass,
    note: item.note.trim(),
  }));
  try {
    await ipcClient.agentsHumanSubmit(props.task.id, results, passed, note.value.trim(), attachments.value);
    emit('changed');
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    busy.value = false;
  }
}

async function reprepare() {
  busy.value = true;
  error.value = null;
  try {
    await ipcClient.agentsHumanReprepare(props.task.id);
    emit('changed');
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    busy.value = false;
  }
}

async function ask() {
  const text = question.value.trim();
  if (!text || asking.value) return;
  asking.value = true;
  error.value = null;
  try {
    await ipcClient.agentsHumanChat(props.task.id, text);
    question.value = '';
    emit('changed');
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    asking.value = false;
  }
}
</script>

<style scoped>
.human-panel {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 10px 12px;
  font-size: 12px;
  border: 1px solid rgba(251, 191, 36, 0.35);
  border-radius: 6px;
  background-color: rgba(251, 191, 36, 0.04);
}

.human-head {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  color: #fbbf24;
}

.human-status {
  padding: 1px 6px;
  font-size: 11px;
  border-radius: 4px;
  color: var(--text-secondary, #94a3b8);
  background-color: rgba(255, 255, 255, 0.06);
}

.human-status.awaiting_human {
  color: #fbbf24;
}

.human-status.passed {
  color: #4edea3;
}

.spacer {
  flex: 1;
}

.human-desc summary,
.rounds summary {
  cursor: pointer;
  color: var(--text-secondary, #94a3b8);
}

.human-desc p {
  margin: 6px 0 0;
  white-space: pre-wrap;
}

.human-wait,
.text-muted {
  color: var(--text-muted, #64748b);
}

.leader-note {
  padding: 6px 8px;
  border-radius: 4px;
  background-color: rgba(96, 165, 250, 0.1);
  white-space: pre-wrap;
}

.guide-summary {
  margin: 0;
  font-weight: 600;
}

.guide-label {
  font-size: 11px;
  color: var(--text-muted, #64748b);
}

.guide-block ol,
.case-steps {
  margin: 4px 0 0;
  padding-left: 18px;
}

.cases {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.case {
  padding: 6px 8px;
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 4px;
}

.case.ok {
  border-color: rgba(78, 222, 163, 0.5);
}

.case.bad {
  border-color: rgba(248, 113, 113, 0.6);
}

.case-head {
  display: flex;
  align-items: center;
  gap: 8px;
}

.case-id {
  font-weight: 600;
}

.case-verdict {
  display: flex;
  gap: 4px;
}

.verdict {
  padding: 1px 8px;
  font-size: 11px;
  color: var(--text-secondary, #94a3b8);
  background: none;
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 4px;
  cursor: pointer;
}

.verdict.pass.on {
  color: #0f1117;
  background-color: #4edea3;
}

.verdict.fail.on {
  color: #0f1117;
  background-color: #f87171;
}

.case-expected {
  margin-top: 4px;
}

.field {
  width: 100%;
  padding: 5px 8px;
  font-size: 12px;
  color: var(--text-primary, #f1f5f9);
  background-color: var(--bg-app-base, #0f1117);
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 4px;
  resize: vertical;
}

.case-note {
  margin-top: 6px;
}

.guide-notes {
  margin: 0;
  white-space: pre-wrap;
  color: var(--text-secondary, #94a3b8);
}

.shots {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.shot {
  position: relative;
  width: 72px;
  height: 54px;
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 4px;
  overflow: hidden;
}

.shot img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  cursor: zoom-in;
}

.shot-remove {
  position: absolute;
  top: 2px;
  right: 2px;
  display: flex;
  padding: 1px;
  color: #fff;
  background-color: rgba(0, 0, 0, 0.6);
  border: none;
  border-radius: 3px;
  cursor: pointer;
}

.shot-add {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  width: 72px;
  height: 54px;
  font-size: 11px;
  color: var(--text-secondary, #94a3b8);
  border: 1px dashed var(--border-subtle, #242a3e);
  border-radius: 4px;
  cursor: pointer;
}

.submit-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
}

.error {
  color: var(--status-failed, #ef4444);
}

.chat {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding-top: 8px;
  border-top: 1px solid var(--border-subtle, #242a3e);
}

.chat-log {
  display: flex;
  flex-direction: column;
  gap: 6px;
  max-height: 220px;
  overflow-y: auto;
}

.chat-msg {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 6px 8px;
  border-radius: 6px;
  white-space: pre-wrap;
}

.chat-msg.user {
  align-self: flex-end;
  max-width: 85%;
  background-color: rgba(116, 71, 145, 0.25);
}

.chat-msg.leader {
  align-self: flex-start;
  max-width: 95%;
  background-color: rgba(96, 165, 250, 0.1);
}

.chat-who {
  font-size: 10px;
  color: var(--text-muted, #64748b);
}

.chat-input {
  display: flex;
  gap: 6px;
  align-items: flex-end;
}

.round {
  margin-top: 6px;
}

.round-fail,
.round-note {
  color: var(--text-secondary, #94a3b8);
}

.zoom {
  position: fixed;
  inset: 0;
  z-index: 1200;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
  background-color: rgba(0, 0, 0, 0.8);
  cursor: zoom-out;
}

.zoom img {
  max-width: 100%;
  max-height: 100%;
}
</style>
