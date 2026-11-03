<template>
  <div v-if="visible" class="modal-backdrop">
    <div class="squash-modal" role="dialog" aria-modal="true">
      <div class="modal-head">
        <GitCommitVertical :size="16" />
        <span class="modal-title">Gộp commit</span>
        <button type="button" class="btn-close" title="Đóng" :disabled="busy" @click="emit('close')">
          <X :size="16" />
        </button>
      </div>
      <div class="modal-body">
        <p class="hint">
          Gộp các commit của nhánh <code>{{ run?.integration_branch }}</code> thành một commit (code giữ nguyên).
          <template v-if="run?.pull_requests.length">
            PR đã push sẽ được ghi đè ở lần push tới (có lease, chỉ khi nhánh trên origin chưa ai đổi).
          </template>
        </p>
        <div v-if="loading" class="hint">Đang đọc commit…</div>
        <template v-else-if="commits">
          <div v-for="repo in commits.repos" :key="repo.repo" class="repo">
            <div class="repo-head">
              <span class="font-mono">{{ repo.repo === '.' ? 'repo' : repo.repo }}</span>
              <span class="count">{{ repo.subjects.length }} commit</span>
            </div>
            <ol class="subjects custom-scrollbar">
              <li v-for="(subject, index) in repo.subjects" :key="index" class="font-mono">{{ subject }}</li>
            </ol>
          </div>
          <p v-if="!commits.repos.length" class="hint">Lượt chạy chưa có commit nào so với nhánh gốc.</p>
          <label class="block">
            <span class="label">Commit message (theo quy ước của repo)</span>
            <textarea v-model="message" rows="7" class="field font-mono" :disabled="busy"></textarea>
          </label>
        </template>
        <div v-if="error" class="error">{{ error }}</div>
      </div>
      <div class="modal-foot">
        <button type="button" class="btn btn-secondary btn-sm" :disabled="busy" @click="emit('close')">Hủy</button>
        <button
          type="button"
          class="btn btn-primary btn-sm"
          :disabled="busy || loading || !canSquash || !message.trim()"
          @click="submit"
        >
          <GitCommitVertical :size="13" /> {{ busy ? 'Đang gộp…' : 'Gộp thành 1 commit' }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { GitCommitVertical, X } from 'lucide-vue-next';
import { ipcClient } from '@/ipc/client';
import type { AgentRun, RunCommits } from '@/types/agents';

const props = defineProps<{
  visible: boolean;
  run: AgentRun | null;
}>();

const emit = defineEmits<{
  (e: 'close'): void;
  (e: 'squashed', count: number): void;
}>();

const commits = ref<RunCommits | null>(null);
const message = ref('');
const loading = ref(false);
const busy = ref(false);
const error = ref<string | null>(null);

const canSquash = computed(() => commits.value?.repos.some((repo) => repo.subjects.length > 1) ?? false);

watch(
  () => props.visible,
  async (visible) => {
    if (!visible || !props.run) return;
    commits.value = null;
    message.value = '';
    error.value = null;
    loading.value = true;
    try {
      commits.value = await ipcClient.agentsRunCommits(props.run.id);
      message.value = commits.value.message;
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e);
    } finally {
      loading.value = false;
    }
  }
);

async function submit() {
  if (!props.run || busy.value || !message.value.trim()) return;
  busy.value = true;
  error.value = null;
  try {
    const count = await ipcClient.agentsRunSquash(props.run.id, message.value);
    emit('squashed', count);
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

.squash-modal {
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

.repo {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.repo-head {
  display: flex;
  justify-content: space-between;
  font-size: 12px;
}

.count {
  color: var(--text-muted, #64748b);
}

.subjects {
  max-height: 180px;
  margin: 0;
  padding: 6px 8px 6px 28px;
  overflow-y: auto;
  font-size: 11px;
  line-height: 1.6;
  background-color: var(--bg-app-base, #0f1117);
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 4px;
}

.block {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
}

.label {
  font-size: 11px;
  color: var(--text-muted, #64748b);
}

.field {
  min-width: 0;
  padding: 6px 8px;
  font-size: 12px;
  line-height: 1.5;
  resize: vertical;
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
