<template>
  <aside v-if="visible" class="leader-chat" role="dialog" aria-label="Chat với leader">
    <header class="lc-head">
      <MessagesSquare :size="15" />
      <span class="lc-title">Chat với leader</span>
      <span class="lc-meta font-mono">{{ run.leader_cli }}</span>
      <button type="button" class="lc-close" title="Đóng" @click="emit('close')">
        <X :size="15" />
      </button>
    </header>

    <div ref="listEl" class="lc-list custom-scrollbar">
      <p class="lc-hint">
        Hỏi trong chính phiên của leader: leader nhớ phần tìm hiểu, thiết kế, kế hoạch, các lần xác nhận lỗi, review PR…
        của lượt chạy này. Chỉ đọc — muốn sửa gì thì gửi bằng "Làm tiếp".
      </p>
      <div v-for="(message, index) in run.leader_chat" :key="index" class="lc-msg" :class="'m-' + message.role">
        <span class="lc-who">{{ WHO[message.role] ?? message.role }}</span>
        <div class="lc-text">{{ message.text }}</div>
      </div>
      <div v-if="busy" class="lc-msg m-leader pending">
        <span class="lc-who">Leader</span>
        <div class="lc-text">
          <LoaderCircle :size="12" class="spin" />
          {{ driving ? 'Leader đang làm một bước của lượt chạy; câu hỏi sẽ được trả lời ngay khi bước đó xong…' : 'Đang trả lời…' }}
        </div>
      </div>
    </div>

    <div v-if="error" class="lc-error">{{ error }}</div>
    <footer class="lc-input">
      <textarea
        ref="inputEl"
        v-model="text"
        rows="3"
        :disabled="busy"
        placeholder="Ví dụ: Vì sao tách task như vậy? API-361 đã đủ field chưa? (Ctrl+Enter để gửi)"
        @keydown.ctrl.enter.prevent="send"
        @keydown.meta.enter.prevent="send"
      ></textarea>
      <button type="button" class="btn btn-primary btn-sm" :disabled="busy || !text.trim()" @click="send">
        <Send :size="13" />
      </button>
    </footer>
  </aside>
</template>

<script setup lang="ts">
import { nextTick, ref, watch } from 'vue';
import { LoaderCircle, MessagesSquare, Send, X } from 'lucide-vue-next';
import { ipcClient } from '@/ipc/client';
import type { AgentRun } from '@/types/agents';

const props = defineProps<{
  visible: boolean;
  run: AgentRun;
  /** A step of the run is going on: the question waits for the leader. */
  driving: boolean;
}>();

const emit = defineEmits<{
  (e: 'close'): void;
}>();

const WHO: Record<string, string> = {
  user: 'Bạn',
  leader: 'Leader',
  error: 'Lỗi',
};

const text = ref('');
const busy = ref(false);
const error = ref<string | null>(null);
const listEl = ref<HTMLElement | null>(null);
const inputEl = ref<HTMLTextAreaElement | null>(null);

const toBottom = async () => {
  await nextTick();
  listEl.value?.scrollTo({ top: listEl.value.scrollHeight });
};

watch(
  () => props.visible,
  async (visible) => {
    if (!visible) return;
    await toBottom();
    inputEl.value?.focus();
  }
);
watch(() => props.run.leader_chat.length, toBottom);

async function send() {
  const message = text.value.trim();
  if (!message || busy.value) return;
  busy.value = true;
  error.value = null;
  text.value = '';
  await toBottom();
  try {
    await ipcClient.agentsLeaderChat(props.run.id, message);
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    busy.value = false;
    await toBottom();
  }
}
</script>

<style scoped>
.leader-chat {
  position: fixed;
  top: 56px;
  right: 16px;
  bottom: 16px;
  z-index: 900;
  width: min(440px, calc(100vw - 32px));
  display: flex;
  flex-direction: column;
  background-color: var(--surface-container, #1a1e2b);
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 8px;
  box-shadow: 0 12px 40px rgba(0, 0, 0, 0.45);
  color: var(--text-primary, #f1f5f9);
}

.lc-head {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 12px;
  border-bottom: 1px solid var(--border-subtle, #242a3e);
  color: #e4b5ff;
}

.lc-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-primary, #f1f5f9);
}

.lc-meta {
  font-size: 11px;
  color: var(--text-muted, #64748b);
}

.lc-close {
  margin-left: auto;
  padding: 2px;
  color: var(--text-secondary, #94a3b8);
  background: none;
  border: none;
  cursor: pointer;
}

.lc-list {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 12px;
  overflow-y: auto;
}

.lc-hint {
  margin: 0;
  font-size: 11px;
  line-height: 1.5;
  color: var(--text-muted, #64748b);
}

.lc-msg {
  display: flex;
  flex-direction: column;
  gap: 3px;
  max-width: 92%;
}

.lc-who {
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--text-muted, #64748b);
}

.lc-text {
  padding: 8px 10px;
  font-size: 12px;
  line-height: 1.55;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  border-radius: 6px;
  background-color: var(--bg-app-base, #0f1117);
}

.m-user {
  align-self: flex-end;
  align-items: flex-end;
}

.m-user .lc-text {
  background-color: rgba(228, 181, 255, 0.12);
}

.m-error .lc-text {
  color: var(--status-failed, #ef4444);
}

.pending .lc-text {
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--text-secondary, #94a3b8);
}

.lc-error {
  padding: 0 12px 6px;
  font-size: 12px;
  color: var(--status-failed, #ef4444);
}

.lc-input {
  display: flex;
  align-items: flex-end;
  gap: 8px;
  padding: 10px 12px;
  border-top: 1px solid var(--border-subtle, #242a3e);
}

.lc-input textarea {
  flex: 1;
  padding: 8px;
  font-size: 12px;
  line-height: 1.5;
  resize: none;
  color: var(--text-primary, #f1f5f9);
  background-color: var(--bg-app-base, #0f1117);
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 6px;
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
