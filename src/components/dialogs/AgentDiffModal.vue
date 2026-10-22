<template>
  <div v-if="visible" class="modal-backdrop" @keydown.esc="emit('close')">
    <div class="diff-modal" role="dialog" aria-modal="true">
      <div class="diff-head">
        <FileDiff :size="15" />
        <span class="diff-title">{{ title }}</span>
        <span class="diff-stat font-mono">
          <span class="add">+{{ added }}</span> <span class="del">-{{ removed }}</span>
        </span>
        <button class="btn-close" title="Đóng (Esc)" @click="emit('close')">
          <X :size="16" />
        </button>
      </div>
      <div class="diff-body custom-scrollbar">
        <div v-if="loading" class="diff-state">Đang tải diff…</div>
        <div v-else-if="error" class="diff-state diff-error">{{ error }}</div>
        <div v-else-if="!text" class="diff-state">Không có thay đổi.</div>
        <pre v-else class="diff-text font-mono"><span
          v-for="(line, index) in lines"
          :key="index"
          class="diff-line"
          :class="line.kind"
        >{{ line.text }}
</span></pre>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { FileDiff, X } from 'lucide-vue-next';

const props = defineProps<{
  visible: boolean;
  title: string;
  text: string;
  loading: boolean;
  error?: string | null;
}>();

const emit = defineEmits<{
  (e: 'close'): void;
}>();

/** Long diffs are cut for the DOM's sake. */
const MAX_LINES = 20000;

const lines = computed(() =>
  props.text
    .split('\n')
    .slice(0, MAX_LINES)
    .map((text) => ({
      text,
      kind: text.startsWith('diff --git')
        ? 'file'
        : text.startsWith('@@')
          ? 'hunk'
          : text.startsWith('+') && !text.startsWith('+++')
            ? 'add'
            : text.startsWith('-') && !text.startsWith('---')
              ? 'del'
              : '',
    }))
);

const added = computed(() => lines.value.filter((line) => line.kind === 'add').length);
const removed = computed(() => lines.value.filter((line) => line.kind === 'del').length);
</script>

<style scoped>
.modal-backdrop {
  position: fixed;
  inset: 0;
  z-index: 1100;
  display: flex;
  align-items: center;
  justify-content: center;
  background-color: rgba(5, 7, 12, 0.7);
}

.diff-modal {
  width: min(1100px, calc(100vw - 32px));
  height: calc(100vh - 64px);
  display: flex;
  flex-direction: column;
  background-color: var(--surface-container, #1a1e2b);
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 8px;
  overflow: hidden;
  color: var(--text-primary, #f1f5f9);
}

.diff-head {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 14px;
  border-bottom: 1px solid var(--border-subtle, #242a3e);
  color: #e4b5ff;
}

.diff-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-primary, #f1f5f9);
}

.diff-stat {
  font-size: 12px;
}

.add {
  color: #4edea3;
}

.del {
  color: #f87171;
}

.btn-close {
  margin-left: auto;
  padding: 4px;
  color: var(--text-secondary, #94a3b8);
  background: none;
  border: none;
  cursor: pointer;
}

.diff-body {
  flex: 1;
  overflow: auto;
  background-color: var(--bg-app-base, #0f1117);
}

.diff-state {
  padding: 24px;
  font-size: 13px;
  color: var(--text-muted, #64748b);
}

.diff-error {
  color: var(--status-failed, #ef4444);
}

.diff-text {
  margin: 0;
  padding: 8px 0;
  font-size: 12px;
  line-height: 1.5;
}

.diff-line {
  display: block;
  padding: 0 14px;
  white-space: pre;
}

.diff-line.add {
  background-color: rgba(78, 222, 163, 0.1);
}

.diff-line.del {
  background-color: rgba(248, 113, 113, 0.1);
}

.diff-line.file {
  margin-top: 8px;
  font-weight: 600;
  color: #e4b5ff;
  background-color: rgba(116, 71, 145, 0.15);
}

.diff-line.hunk {
  color: #60a5fa;
}
</style>
