<template>
  <div v-if="visible" class="modal-backdrop">
    <div class="followup-modal" role="dialog" aria-modal="true">
      <div class="modal-head">
        <MessageSquarePlus :size="16" />
        <span class="modal-title">Làm tiếp lượt chạy</span>
        <span class="modal-subtitle">{{ subtitle }}</span>
        <button type="button" class="btn-close" title="Đóng" @click="emit('close')">
          <X :size="16" />
        </button>
      </div>
      <div class="modal-body">
        <p class="hint">
          Leader lập kế hoạch tiếp trong phiên cũ (vẫn nhớ thiết kế, contract và các task đã làm), rồi đội làm qua
          Cổng 1 → triển khai → Cổng 2 như trước.
          <template v-if="merged">Nhánh tích hợp bắt đầu lại từ nhánh gốc hiện tại.</template>
          <template v-else>Làm tiếp trên nhánh đã giữ.</template>
          Dán link PR để app tải lại comment review mới nhất.
        </p>
        <textarea
          ref="input"
          v-model="text"
          class="request-input font-mono"
          rows="10"
          placeholder="Ví dụ: https://github.com/org/repo/pull/123&#10;Reviewer: tách validate ra hook riêng, thêm test cho mã lỗi 429…"
          @keydown.ctrl.enter="submit"
          @keydown.meta.enter="submit"
        ></textarea>
        <label v-if="pullRequests.length" class="push-option">
          <input v-model="pushPr" type="checkbox" />
          <span>
            Xong thì tự push lên PR hiện tại
            <span class="font-mono">({{ pullRequests.map((pr) => pr.url.replace(/^https:\/\/github\.com\//, '')).join(', ') }})</span>
            khi mọi task đã merge và kiểm tra tích hợp qua; không dừng ở Cổng 2. Bỏ chọn để duyệt ở Cổng 2 trước khi push.
          </span>
        </label>
        <div v-if="error" class="error">{{ error }}</div>
      </div>
      <div class="modal-foot">
        <button type="button" class="btn btn-secondary btn-sm" :disabled="busy" @click="emit('close')">Hủy</button>
        <button type="button" class="btn btn-primary btn-sm" :disabled="busy || !text.trim()" @click="submit">
          <Play :size="13" /> {{ busy ? 'Đang gửi…' : 'Làm tiếp (Ctrl+Enter)' }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { nextTick, ref, watch } from 'vue';
import { MessageSquarePlus, Play, X } from 'lucide-vue-next';
import type { PullRequest } from '@/types/agents';

const props = defineProps<{
  visible: boolean;
  subtitle: string;
  merged: boolean;
  pullRequests: PullRequest[];
  submit: (request: string, pushPr: boolean) => Promise<void>;
}>();

const emit = defineEmits<{
  (e: 'close'): void;
}>();

const text = ref('');
const busy = ref(false);
const error = ref<string | null>(null);
const pushPr = ref(true);
const input = ref<HTMLTextAreaElement | null>(null);

watch(
  () => props.visible,
  async (visible) => {
    if (!visible) return;
    text.value = '';
    error.value = null;
    pushPr.value = props.pullRequests.length > 0;
    await nextTick();
    input.value?.focus();
  }
);

async function submit() {
  if (!text.value.trim() || busy.value) return;
  busy.value = true;
  error.value = null;
  try {
    await props.submit(text.value.trim(), pushPr.value && props.pullRequests.length > 0);
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

.followup-modal {
  width: min(680px, 100%);
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
}

.hint {
  margin: 0;
  font-size: 12px;
  line-height: 1.5;
  color: var(--text-secondary, #94a3b8);
}

.request-input {
  width: 100%;
  min-height: 160px;
  padding: 10px;
  font-size: 12px;
  line-height: 1.5;
  resize: vertical;
  color: var(--text-primary, #f1f5f9);
  background-color: var(--bg-app-base, #0f1117);
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 6px;
}

.push-option {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  font-size: 12px;
  line-height: 1.5;
  color: var(--text-secondary, #94a3b8);
  cursor: pointer;
}

.push-option input {
  margin-top: 3px;
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
