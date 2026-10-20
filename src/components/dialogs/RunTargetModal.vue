<template>
  <div
    v-if="request"
    class="modal-backdrop"
    @keydown="handleKeydown"
  >
    <div ref="dialogEl" class="run-target-modal" role="dialog" aria-modal="true" tabindex="-1">
      <header class="rt-header">
        <Play :size="15" class="rt-icon" />
        <div class="rt-titles">
          <span class="rt-title">Chạy "{{ request.command.name }}" ở đâu?</span>
          <code class="rt-command">{{ request.command.execution_string }}</code>
        </div>
      </header>

      <div class="rt-list">
        <button
          v-for="(target, index) in request.targets"
          :key="target"
          type="button"
          class="rt-item"
          :class="{ active: index === activeIndex }"
          @mouseenter="activeIndex = index"
          @click="answer(target)"
        >
          <kbd v-if="index < 9" class="rt-key">{{ index + 1 }}</kbd>
          <template v-if="target === LOCAL_TARGET">
            <Monitor :size="14" class="rt-target-icon" />
            <span class="rt-name">Máy này</span>
            <span class="rt-meta">terminal local</span>
          </template>
          <template v-else>
            <span class="rt-dot" :style="{ backgroundColor: host(target)?.color || '#744791' }" />
            <span class="rt-name">{{ host(target)?.name ?? 'Máy chủ đã xóa' }}</span>
            <span class="rt-meta font-mono">
              {{ host(target) ? `${host(target)!.username}@${host(target)!.host}` : target }}
            </span>
          </template>
        </button>
      </div>

      <footer class="rt-footer">
        <span>↑↓ chọn · Enter chạy · 1–9 chạy ngay · Esc hủy</span>
        <button type="button" class="btn btn-ghost btn-sm" @click="answer(null)">Hủy</button>
      </footer>
    </div>
  </div>
</template>

<script setup lang="ts">
import { nextTick, ref, watch } from 'vue';
import { Monitor, Play } from 'lucide-vue-next';
import { LOCAL_TARGET } from '@/types/models';
import { useRunTargetPicker } from '@/composables/useRunTargetPicker';
import { useSshHosts } from '@/composables/useSshHosts';

const { request, answer } = useRunTargetPicker();
const { hosts, fetchHosts } = useSshHosts();
const activeIndex = ref(0);
const dialogEl = ref<HTMLElement | null>(null);

const host = (id: string) => hosts.value.find((candidate) => candidate.id === id);

watch(request, (current) => {
  if (!current) return;
  const preselected = current.preselect ? current.targets.indexOf(current.preselect) : -1;
  activeIndex.value = Math.max(0, preselected);
  if (hosts.value.length === 0) void fetchHosts();
  void nextTick(() => dialogEl.value?.focus());
});

const handleKeydown = (event: KeyboardEvent) => {
  const current = request.value;
  if (!current || event.isComposing) return;
  const count = current.targets.length;
  if (event.key === 'Escape') {
    answer(null);
  } else if (event.key === 'ArrowDown') {
    activeIndex.value = (activeIndex.value + 1) % count;
  } else if (event.key === 'ArrowUp') {
    activeIndex.value = (activeIndex.value - 1 + count) % count;
  } else if (event.key === 'Enter') {
    answer(current.targets[activeIndex.value]);
  } else if (/^[1-9]$/.test(event.key) && Number(event.key) <= count) {
    answer(current.targets[Number(event.key) - 1]);
  } else {
    return;
  }
  event.preventDefault();
  event.stopPropagation();
};
</script>

<style scoped>
.modal-backdrop {
  position: fixed;
  inset: 0;
  z-index: 1060;
  display: flex;
  align-items: flex-start;
  justify-content: center;
  padding-top: 15vh;
  background-color: rgba(11, 13, 19, 0.7);
}

.run-target-modal {
  width: 440px;
  max-width: calc(100vw - 32px);
  background-color: var(--bg-surface);
  border: 1px solid var(--border-focus);
  border-radius: var(--radius-lg);
  box-shadow: 0 16px 40px rgba(0, 0, 0, 0.6);
  overflow: hidden;
  outline: none;
}

.rt-header {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  padding: 12px 14px;
  background-color: var(--bg-sidebar);
  border-bottom: 1px solid var(--border-subtle);
}

.rt-icon {
  color: #e4b5ff;
  margin-top: 2px;
}

.rt-titles {
  display: flex;
  flex-direction: column;
  gap: 3px;
  min-width: 0;
}

.rt-title {
  font-size: 13px;
  font-weight: 600;
}

.rt-command {
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--text-muted);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.rt-list {
  display: flex;
  flex-direction: column;
  padding: 6px;
  gap: 2px;
  max-height: 50vh;
  overflow-y: auto;
}

.rt-item {
  display: flex;
  align-items: center;
  gap: 10px;
  height: 36px;
  padding: 0 10px;
  background: transparent;
  border: 1px solid transparent;
  border-radius: var(--radius-sm);
  color: var(--text-primary);
  font-size: 12.5px;
  text-align: left;
  cursor: pointer;
}

.rt-item.active {
  background-color: var(--primary-subtle, #7447911f);
  border-color: var(--border-focus);
}

.rt-key {
  width: 18px;
  text-align: center;
  font-size: 10.5px;
  color: var(--text-muted);
  border: 1px solid var(--border-subtle);
  border-radius: 3px;
}

.rt-target-icon {
  color: var(--text-secondary);
}

.rt-dot {
  width: 9px;
  height: 9px;
  margin: 0 2px;
  border-radius: 50%;
  flex-shrink: 0;
}

.rt-name {
  font-weight: 500;
}

.rt-meta {
  margin-left: auto;
  font-size: 11px;
  color: var(--text-muted);
}

.rt-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 12px;
  border-top: 1px solid var(--border-subtle);
  background-color: var(--bg-sidebar);
  font-size: 11px;
  color: var(--text-muted);
}
</style>
