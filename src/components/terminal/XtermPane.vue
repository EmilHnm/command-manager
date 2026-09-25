<template>
  <div class="xterm-pane-container">
    <!-- Pane Toolbar -->
    <div class="pane-toolbar">
      <div class="toolbar-left">
        <span class="cmd-badge" :class="processStatus">
          <span class="status-dot" :class="{ active: processStatus === 'running' }" />
          {{ commandName }} (ID: #{{ commandId }})
        </span>
        <span v-if="pid" class="pid-tag">PID: {{ pid }}</span>
        <span v-if="processStatus === 'completed'" class="exit-tag success">Exit: 0</span>
        <span v-if="processStatus === 'failed'" class="exit-tag error">Exit: 1</span>
      </div>

      <div class="toolbar-right">
        <button class="btn btn-ghost btn-sm" title="Xả lại dữ liệu từ Ring Buffer in-memory" @click="handleReattach">
          <RefreshCw :size="13" />
          <span>Reattach</span>
        </button>
        <button class="btn btn-ghost btn-sm" title="Xóa màn hình cục bộ" @click="handleClear">
          <Trash2 :size="13" />
          <span>Clear</span>
        </button>
        <button
          v-if="processStatus === 'running'"
          class="btn btn-danger btn-sm"
          title="Dừng tiến trình (Gửi SIGTERM)"
          @click="$emit('stop-process', commandId)"
        >
          <Square :size="12" />
          <span>Dừng Lệnh</span>
        </button>
        <button
          v-else
          class="btn btn-primary btn-sm"
          title="Khởi động lại lệnh"
          @click="$emit('restart-process', commandId)"
        >
          <RotateCw :size="12" />
          <span>Khởi Động Lại</span>
        </button>
      </div>
    </div>

    <!-- Terminal Viewport -->
    <div ref="terminalElement" class="terminal-viewport" />
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount, watch } from 'vue';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { RefreshCw, Trash2, Square, RotateCw } from 'lucide-vue-next';
import { usePtyStream } from '@/composables/usePtyStream';
import type { ProcessLifecycleStatus } from '@/types/models';

const props = withDefaults(
  defineProps<{
    commandId: number;
    commandName: string;
    pid?: number;
    processStatus?: ProcessLifecycleStatus;
  }>(),
  {
    processStatus: 'idle',
  }
);

defineEmits<{
  (e: 'stop-process', id: number): void;
  (e: 'restart-process', id: number): void;
}>();

const terminalElement = ref<HTMLDivElement | null>(null);
let term: Terminal | null = null;
let fitAddon: FitAddon | null = null;
let resizeObserver: ResizeObserver | null = null;
let unsubscribePty: (() => void) | null = null;

const { subscribePty, sendInput, resize, reattachBuffer } = usePtyStream();

onMounted(async () => {
  if (!terminalElement.value) return;

  // Khởi tạo xterm.js với Dark Theme đồng bộ primary #744791
  term = new Terminal({
    fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
    fontSize: 13,
    lineHeight: 1.4,
    cursorBlink: true,
    cursorStyle: 'block',
    convertEol: true,
    theme: {
      background: '#0b0d13',
      foreground: '#f1f5f9',
      cursor: '#8956aa',
      cursorAccent: '#ffffff',
      selectionBackground: 'rgba(116, 71, 145, 0.4)',
      black: '#1a1e2b',
      red: '#ef4444',
      green: '#10b981',
      yellow: '#f59e0b',
      blue: '#3b82f6',
      magenta: '#a855f7',
      cyan: '#06b6d4',
      white: '#f8fafc',
      brightBlack: '#475569',
      brightRed: '#f87171',
      brightGreen: '#34d399',
      brightYellow: '#fbbf24',
      brightBlue: '#60a5fa',
      brightMagenta: '#c084fc',
      brightCyan: '#22d3ee',
      brightWhite: '#ffffff',
    },
  });

  fitAddon = new FitAddon();
  term.loadAddon(fitAddon);
  term.open(terminalElement.value);
  fitAddon.fit();

  // Gửi input bàn phím tới Rust PTY
  term.onData((data) => {
    sendInput(props.commandId, data);
  });

  // Đăng ký nhận luồng byte từ PTY
  unsubscribePty = subscribePty(props.commandId, (chunk) => {
    term?.write(chunk);
  });

  // Xả dữ liệu gần nhất từ Ring Buffer in-memory
  const initialData = await reattachBuffer(props.commandId);
  if (initialData) {
    term.write(initialData);
  }

  // Tự động căn kích thước và đồng bộ cols/rows với PTY
  resizeObserver = new ResizeObserver(() => {
    if (fitAddon && term) {
      fitAddon.fit();
      resize(props.commandId, term.cols, term.rows);
    }
  });
  resizeObserver.observe(terminalElement.value);
});

onBeforeUnmount(() => {
  if (unsubscribePty) unsubscribePty();
  if (resizeObserver) resizeObserver.disconnect();
  if (term) term.dispose();
});

const handleClear = () => {
  term?.clear();
};

const handleReattach = async () => {
  const data = await reattachBuffer(props.commandId);
  if (term && data) {
    term.clear();
    term.write(data);
  }
};
</script>

<style scoped>
.xterm-pane-container {
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  background-color: var(--bg-terminal);
  overflow: hidden;
}

.pane-toolbar {
  height: 34px;
  background-color: var(--bg-surface);
  border-bottom: 1px solid var(--border-subtle);
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 10px;
  flex-shrink: 0;
}

.toolbar-left, .toolbar-right {
  display: flex;
  align-items: center;
  gap: 8px;
}

.cmd-badge {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 11.5px;
  font-weight: 600;
  color: var(--text-primary);
}

.status-dot {
  width: 7px;
  height: 7px;
  border-radius: 9999px;
  background-color: var(--status-idle);
}

.status-dot.active {
  background-color: var(--status-running);
  box-shadow: 0 0 6px var(--status-running);
}

.pid-tag {
  background-color: var(--bg-app-base);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  padding: 1px 5px;
  font-size: 10px;
  font-family: var(--font-mono);
  color: var(--text-secondary);
}

.exit-tag {
  padding: 1px 5px;
  border-radius: var(--radius-sm);
  font-size: 10px;
  font-family: var(--font-mono);
  font-weight: 600;
}

.exit-tag.success {
  background: var(--status-running-bg);
  color: var(--status-running);
}

.exit-tag.error {
  background: var(--status-failed-bg);
  color: var(--status-failed);
}

.terminal-viewport {
  flex: 1;
  padding: 8px;
  overflow: hidden;
  background-color: var(--bg-terminal);
}
</style>
