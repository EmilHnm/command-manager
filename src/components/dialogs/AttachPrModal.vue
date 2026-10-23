<template>
  <div v-if="visible" class="modal-backdrop">
    <div class="attach-modal" role="dialog" aria-modal="true">
      <div class="modal-head">
        <Link2 :size="16" />
        <span class="modal-title">Gắn PR đã tạo</span>
        <button type="button" class="btn-close" title="Đóng" :disabled="busy" @click="emit('close')">
          <X :size="16" />
        </button>
      </div>
      <div class="modal-body">
        <p class="hint">
          Lượt chạy ghi nhận PR này: lần "Làm tiếp" sau sẽ lấy bản mới nhất của nhánh PR từ <code>origin</code>, tải comment
          review của nó, và "Push &amp; tạo PR" ở Cổng 2 cập nhật đúng PR này.
          <template v-if="atGate">Lượt chạy kết thúc ở trạng thái "Giữ nhánh".</template>
        </p>
        <label v-if="repos.length > 1" class="block">
          <span class="label">Repo</span>
          <select v-model="repo" class="field">
            <option v-for="item in repos" :key="item" :value="item">{{ item }}</option>
          </select>
        </label>
        <label class="block">
          <span class="label">Link PR</span>
          <input
            ref="urlInput"
            v-model="url"
            class="field font-mono"
            placeholder="https://github.com/org/repo/pull/123"
            @keydown.enter="submit"
          />
        </label>
        <details class="advanced">
          <summary>Nhánh (để trống: đọc từ GitHub bằng gh)</summary>
          <div class="pair">
            <label class="block">
              <span class="label">Nhánh của PR</span>
              <input v-model="branch" class="field font-mono" placeholder="feat/..." />
            </label>
            <label class="block">
              <span class="label">PR vào nhánh</span>
              <input v-model="base" class="field font-mono" placeholder="develop" />
            </label>
          </div>
        </details>
        <div v-if="error" class="error">{{ error }}</div>
      </div>
      <div class="modal-foot">
        <button type="button" class="btn btn-secondary btn-sm" :disabled="busy" @click="emit('close')">Hủy</button>
        <button type="button" class="btn btn-primary btn-sm" :disabled="busy || !url.trim()" @click="submit">
          <Link2 :size="13" /> {{ busy ? 'Đang đọc PR…' : 'Gắn PR' }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { nextTick, ref, watch } from 'vue';
import { Link2, X } from 'lucide-vue-next';
import { ipcClient } from '@/ipc/client';
import type { AgentRun } from '@/types/agents';

const props = defineProps<{
  visible: boolean;
  run: AgentRun | null;
}>();

const emit = defineEmits<{
  (e: 'close'): void;
  (e: 'attached', run: AgentRun): void;
}>();

const repos = ref<string[]>([]);
const repo = ref('.');
const url = ref('');
const branch = ref('');
const base = ref('');
const busy = ref(false);
const error = ref<string | null>(null);
const atGate = ref(false);
const urlInput = ref<HTMLInputElement | null>(null);

watch(
  () => props.visible,
  async (visible) => {
    if (!visible || !props.run) return;
    const run = props.run;
    repos.value = run.repos.map((item) => item.path);
    // The repo whose PR is likely: one already linked, else the first.
    repo.value = run.pull_requests[0]?.repo ?? repos.value[0] ?? '.';
    url.value = '';
    branch.value = '';
    base.value = '';
    error.value = null;
    atGate.value = run.stage === 'merge_gate';
    await nextTick();
    urlInput.value?.focus();
  }
);

async function submit() {
  if (!props.run || !url.value.trim() || busy.value) return;
  busy.value = true;
  error.value = null;
  try {
    const run = await ipcClient.agentsRunAttachPr(props.run.id, {
      repo: repo.value,
      url: url.value.trim(),
      branch: branch.value.trim() || null,
      base: base.value.trim() || null,
    });
    emit('attached', run);
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

.attach-modal {
  width: min(560px, 100%);
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
}

.hint {
  margin: 0;
  font-size: 12px;
  line-height: 1.5;
  color: var(--text-secondary, #94a3b8);
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
  color: var(--text-primary, #f1f5f9);
  background-color: var(--bg-app-base, #0f1117);
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 4px;
}

.advanced summary {
  font-size: 12px;
  color: var(--text-secondary, #94a3b8);
  cursor: pointer;
}

.pair {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
  margin-top: 8px;
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

@media (max-width: 480px) {
  .pair {
    grid-template-columns: 1fr;
  }
}
</style>
