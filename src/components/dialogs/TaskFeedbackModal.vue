<template>
  <div v-if="visible && task" class="modal-backdrop" @keydown.esc="emit('close')">
    <div class="feedback-modal" role="dialog" aria-modal="true" aria-labelledby="feedback-title">
      <!-- Top Accent Bar -->
      <div class="modal-accent-bar"></div>

      <!-- Header -->
      <div class="modal-header">
        <div class="header-left">
          <div class="icon-badge">
            <Zap :size="16" />
          </div>
          <div class="header-titles">
            <h3 id="feedback-title" class="modal-title">
              Yêu Cầu Agent Sửa Lại: <span class="font-mono text-primary">{{ task.key }}</span>
            </h3>
            <span class="modal-subtitle">
              Gửi góp ý trực tiếp vào phiên của worker {{ task.cli }} để tạo vòng commit sửa lỗi mới
            </span>
          </div>
        </div>
        <button class="btn-close" title="Đóng (Esc)" @click="emit('close')">
          <X :size="16" />
        </button>
      </div>

      <!-- Body -->
      <div class="modal-body custom-scrollbar">
        <!-- Review context hint if available -->
        <div v-if="task.last_review && task.last_review.comments?.length" class="review-context-box">
          <div class="context-head">
            <AlertTriangle :size="13" class="text-status-warning" />
            <span class="font-bold">Góp ý từ review gần nhất (vòng {{ task.last_review.round }}):</span>
          </div>
          <div class="context-list font-mono">
            <button
              v-for="(cmt, idx) in task.last_review.comments"
              :key="idx"
              type="button"
              class="comment-suggestion"
              title="Bấm để chèn nội dung này vào ô góp ý"
              @click="insertSuggestion(cmt.issue)"
            >
              <span class="sev-tag" :class="cmt.severity">[{{ cmt.severity }}]</span>
              <span class="file-path">{{ cmt.file }}{{ cmt.line ? `:${cmt.line}` : '' }}</span>:
              <span class="issue-text">{{ cmt.issue }}</span>
            </button>
          </div>
        </div>

        <!-- Scope Warning if available -->
        <div v-if="task.out_of_scope?.length" class="scope-warning-box font-mono">
          <AlertCircle :size="13" class="text-amber-400" />
          <span>Có thay đổi ngoài scope: {{ task.out_of_scope.join(', ') }}</span>
          <button type="button" class="btn-text-xs" @click="insertSuggestion(`Đưa các thay đổi ngoài phạm vi (${task.out_of_scope.join(', ')}) về trạng thái ban đầu.`)">
            Chèn yêu cầu revert
          </button>
        </div>

        <!-- Feedback Textarea -->
        <div class="form-group">
          <label class="form-label required">
            Chi tiết nội dung cần sửa hoặc hướng dẫn cho {{ task.cli }}
          </label>
          <textarea
            v-model="feedbackText"
            rows="5"
            class="form-textarea font-mono"
            placeholder="Ví dụ: Thiếu kiểm tra coupon hết hạn ở apply_coupon; cần bổ sung test case khi coupon hết hạn trả về Error::ExpiredCoupon..."
            autofocus
            @keydown.ctrl.enter="handleSubmit"
            @keydown.meta.enter="handleSubmit"
          ></textarea>
          <span class="form-hint">Nhấn Ctrl+Enter hoặc ⌘+Enter để gửi nhanh</span>
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
          :disabled="busy || !feedbackText.trim()"
          @click="handleSubmit"
        >
          <Zap :size="14" />
          <span>{{ busy ? 'Đang gửi…' : 'Gửi cho agent sửa ngay' }}</span>
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue';
import { AlertCircle, AlertTriangle, X, Zap } from 'lucide-vue-next';
import type { AgentTask } from '@/types/agents';

const props = defineProps<{
  visible: boolean;
  task: AgentTask | null;
  submit: (feedback: string) => Promise<void> | void;
}>();

const emit = defineEmits<{
  (e: 'close'): void;
}>();

const feedbackText = ref('');
const busy = ref(false);
const error = ref<string | null>(null);

watch(
  () => props.visible,
  (val) => {
    if (val) {
      error.value = null;
      busy.value = false;
      // Pre-fill with issue if single must issue
      if (props.task?.last_review?.comments?.length === 1) {
        feedbackText.value = props.task.last_review.comments[0].issue;
      } else {
        feedbackText.value = '';
      }
    }
  }
);

const insertSuggestion = (text: string) => {
  if (!feedbackText.value.trim()) {
    feedbackText.value = text;
  } else {
    feedbackText.value = `${feedbackText.value.trim()}\n- ${text}`;
  }
};

const handleSubmit = async () => {
  const trimmed = feedbackText.value.trim();
  if (!trimmed) return;
  busy.value = true;
  error.value = null;
  try {
    await props.submit(trimmed);
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

.feedback-modal {
  background-color: var(--color-bg-surface, #1a1e2b);
  border: 1px solid var(--color-border-subtle, #242a3e);
  border-radius: 12px;
  width: 100%;
  max-width: 600px;
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
  background: linear-gradient(90deg, #744791 0%, #10b981 100%);
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
  color: #bd85e8;
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

.review-context-box {
  background-color: rgba(245, 158, 11, 0.08);
  border: 1px solid rgba(245, 158, 11, 0.25);
  border-radius: 6px;
  padding: 10px 12px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.context-head {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: #f59e0b;
}

.context-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.comment-suggestion {
  background: transparent;
  border: 1px dashed rgba(245, 158, 11, 0.2);
  border-radius: 4px;
  padding: 4px 8px;
  text-align: left;
  font-size: 11px;
  color: var(--color-text-secondary, #94a3b8);
  cursor: pointer;
  transition: all 0.15s ease;
}

.comment-suggestion:hover {
  background-color: rgba(245, 158, 11, 0.15);
  color: var(--color-text-primary, #e2e2eb);
}

.sev-tag {
  font-weight: 600;
  margin-right: 4px;
}

.sev-tag.must {
  color: #ef4444;
}

.sev-tag.should {
  color: #f59e0b;
}

.file-path {
  color: var(--color-text-muted, #64748b);
}

.issue-text {
  color: var(--color-text-primary, #e2e2eb);
}

.scope-warning-box {
  display: flex;
  align-items: center;
  justify-content: space-between;
  background-color: rgba(245, 158, 11, 0.08);
  border: 1px solid rgba(245, 158, 11, 0.25);
  border-radius: 6px;
  padding: 8px 12px;
  font-size: 11px;
  color: #fbbf24;
}

.btn-text-xs {
  background: transparent;
  border: none;
  color: #38bdf8;
  font-size: 11px;
  text-decoration: underline;
  cursor: pointer;
  padding: 0;
}

.form-group {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.form-label {
  font-size: 12px;
  font-weight: 500;
  color: var(--color-text-primary, #e2e2eb);
}

.form-label.required::after {
  content: ' *';
  color: #ef4444;
}

.form-textarea {
  width: 100%;
  padding: 10px 12px;
  background-color: var(--color-bg-app-base, #0f1117);
  border: 1px solid var(--color-border-subtle, #242a3e);
  border-radius: 6px;
  color: var(--color-text-primary, #e2e2eb);
  font-size: 12px;
  line-height: 1.5;
  outline: none;
  resize: vertical;
  min-height: 100px;
  transition: border-color 0.15s ease;
}

.form-textarea:focus {
  border-color: var(--color-border-focus, #744791);
}

.form-hint {
  font-size: 11px;
  color: var(--color-text-muted, #64748b);
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
