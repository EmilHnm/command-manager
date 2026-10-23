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
        </template>
        <div v-if="error" class="error">{{ error }}</div>
      </div>

      <div class="modal-foot">
        <button type="button" class="btn btn-secondary btn-sm" :disabled="busy" @click="emit('close')">Hủy</button>
        <button type="button" class="btn btn-primary btn-sm" :disabled="busy || !canSubmit" @click="submit">
          <LoaderCircle v-if="busy" :size="13" class="spin" />
          <Upload v-else :size="13" />
          {{ busy ? 'Đang push & tạo PR…' : `Push & tạo PR (${picked.length} repo)` }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { GitPullRequest, LoaderCircle, Upload, X } from 'lucide-vue-next';
import { ipcClient } from '@/ipc/client';
import type { PrRequest } from '@/types/agents';

const props = defineProps<{
  visible: boolean;
  runId: string | null;
  runTitle: string;
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

async function submit() {
  if (!form.value || !props.runId) return;
  busy.value = true;
  error.value = null;
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
