<template>
  <Teleport to="body">
    <div
      v-if="visible"
      ref="menuEl"
      class="new-terminal-menu"
      :style="{ left: `${position.x}px`, top: `${position.y}px` }"
      role="menu"
      @contextmenu.prevent.stop
      @pointerdown.stop
    >
      <div class="ntm-header">Mở terminal mới</div>
      <div v-if="loading && profiles.length === 0" class="ntm-empty">
        <LoaderCircle :size="12" class="spin" />
        <span>Đang tìm các loại terminal…</span>
      </div>
      <div v-else-if="profiles.length === 0" class="ntm-empty">
        Không tìm thấy terminal nào.
      </div>
      <button
        v-for="item in profiles"
        :key="item.id"
        class="ntm-item"
        :class="{ default: item.id === defaultId }"
        role="menuitem"
        @click="pick(item.id)"
      >
        <component :is="iconFor(item.kind)" :size="13" class="ntm-icon" />
        <span class="ntm-name">{{ item.name }}</span>
        <span v-if="item.id === defaultId" class="ntm-badge">Mặc định</span>
      </button>
      <div class="ntm-separator" />
      <button class="ntm-item ntm-settings" role="menuitem" @click="openSettings">
        <Settings2 :size="13" class="ntm-icon" />
        <span class="ntm-name">Chọn terminal mặc định…</span>
      </button>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch } from 'vue';
import { useRouter } from 'vue-router';
import { Boxes, LoaderCircle, Settings2, SquareTerminal, Terminal } from 'lucide-vue-next';
import { useTerminalProfiles } from '@/composables/useTerminalProfiles';
import type { TerminalProfile } from '@/types/models';

const props = defineProps<{
  visible: boolean;
  /** Viewport coordinates of the right click. */
  x: number;
  y: number;
  /** The `terminalShell` setting; empty: automatic. */
  defaultId: string;
}>();

const emit = defineEmits<{
  (e: 'pick', profileId: string): void;
  (e: 'close'): void;
}>();

const { profiles, loading } = useTerminalProfiles();
const router = useRouter();
const menuEl = ref<HTMLElement | null>(null);
const position = ref({ x: 0, y: 0 });

const iconFor = (kind: TerminalProfile['kind']) => {
  if (kind === 'wsl') return Boxes;
  if (kind === 'cmd') return Terminal;
  return SquareTerminal;
};

const pick = (id: string) => {
  emit('pick', id);
  emit('close');
};

const openSettings = () => {
  emit('close');
  void router.push('/settings');
};

/** Keeps the menu inside the window, flipping it left/up near the edges. */
const place = async () => {
  position.value = { x: props.x, y: props.y };
  await nextTick();
  const el = menuEl.value;
  if (!el) return;
  const { width, height } = el.getBoundingClientRect();
  const margin = 8;
  let x = props.x;
  let y = props.y;
  if (x + width > window.innerWidth - margin) x = Math.max(margin, x - width);
  if (y + height > window.innerHeight - margin) y = Math.max(margin, window.innerHeight - height - margin);
  position.value = { x, y };
};

const close = () => emit('close');
const onKey = (event: KeyboardEvent) => {
  if (event.key === 'Escape') {
    event.stopPropagation();
    close();
  }
};

const listen = (on: boolean) => {
  if (on) {
    window.addEventListener('pointerdown', close);
    window.addEventListener('keydown', onKey, true);
    window.addEventListener('resize', close);
    window.addEventListener('blur', close);
  } else {
    window.removeEventListener('pointerdown', close);
    window.removeEventListener('keydown', onKey, true);
    window.removeEventListener('resize', close);
    window.removeEventListener('blur', close);
  }
};

watch(
  () => props.visible,
  (visible) => {
    listen(false);
    if (!visible) return;
    void place();
    // After the click that opened the menu has finished propagating.
    setTimeout(() => {
      if (props.visible) listen(true);
    });
  },
);
// The list can arrive after the menu opened and change its height.
watch(
  () => profiles.value.length,
  () => {
    if (props.visible) void place();
  },
);

onBeforeUnmount(() => listen(false));
</script>

<style scoped>
.new-terminal-menu {
  position: fixed;
  z-index: 1000;
  min-width: 240px;
  max-width: 320px;
  max-height: calc(100vh - 16px);
  overflow-y: auto;
  background-color: #141721;
  border: 1px solid var(--border-medium, #282d3f);
  border-radius: 6px;
  padding: 4px;
  box-shadow: 0 8px 24px #00000099;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.ntm-header {
  padding: 4px 10px 6px;
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--text-muted, #64748b);
}

.ntm-empty {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  font-size: 12px;
  color: var(--text-muted, #64748b);
}

.ntm-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  border-radius: 4px;
  background: transparent;
  border: none;
  color: var(--text-secondary, #94a3b8);
  font-size: 12px;
  text-align: left;
  cursor: pointer;
  transition: all 0.12s ease;
}

.ntm-item:hover {
  background-color: var(--surface-white-05, #ffffff0d);
  color: #ffffff;
}

.ntm-item.default {
  color: var(--primary-accent, #e4b5ff);
}

.ntm-icon {
  flex-shrink: 0;
}

.ntm-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ntm-badge {
  flex-shrink: 0;
  font-size: 10px;
  padding: 1px 6px;
  border-radius: 999px;
  background-color: var(--primary-alpha-25, #74479140);
  color: var(--primary-accent, #e4b5ff);
}

.ntm-separator {
  height: 1px;
  margin: 3px 4px;
  background-color: var(--border-medium, #282d3f);
}

.ntm-settings {
  color: var(--text-muted, #64748b);
}

.spin {
  animation: ntm-spin 1s linear infinite;
}

@keyframes ntm-spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
