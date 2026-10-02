<template>
  <aside class="sessions-drawer" :class="{ collapsed: isCollapsed }">
    <!-- Drawer Header -->
    <div class="drawer-header">
      <div class="header-title-box">
        <FolderKanban :size="14" class="header-icon" />
        <span class="header-title">PHIÊN LÀM VIỆC ({{ sessions.length }})</span>
      </div>
      <div class="header-actions">
        <button
          class="btn-icon"
          title="Tạo Phiên Mới (Ctrl+Shift+T)"
          @click="$emit('create-session')"
        >
          <Plus :size="13" />
        </button>
        <button
          class="btn-icon"
          :title="isCollapsed ? 'Mở rộng ngăn kéo' : 'Thu gọn ngăn kéo'"
          @click="isCollapsed = !isCollapsed"
        >
          <PanelLeftClose v-if="!isCollapsed" :size="13" />
          <PanelLeftOpen v-else :size="13" />
        </button>
      </div>
    </div>

    <!-- Sessions List -->
    <div v-if="!isCollapsed" class="drawer-content custom-scrollbar">
      <div
        v-for="session in sessions"
        :key="session.id"
        class="session-card"
        :class="{ active: session.id === activeSessionId }"
        @click="$emit('select-session', session.id)"
      >
        <div class="card-top">
          <!-- Miniature Layout Wireframe Icon -->
          <div class="layout-thumb" :title="`Bố cục: ${session.preset}`">
            <!-- 1+2 Tiled -->
            <svg v-if="session.preset === '1+2-tiled'" viewBox="0 0 16 16" class="thumb-svg">
              <rect x="1" y="1" width="7" height="14" rx="1" class="thumb-rect primary" />
              <rect x="9" y="1" width="6" height="6.5" rx="1" class="thumb-rect" />
              <rect x="9" y="8.5" width="6" height="6.5" rx="1" class="thumb-rect" />
            </svg>
            <!-- 2x2 Grid -->
            <svg v-else-if="session.preset === '2x2-grid'" viewBox="0 0 16 16" class="thumb-svg">
              <rect x="1" y="1" width="6.5" height="6.5" rx="1" class="thumb-rect" />
              <rect x="8.5" y="1" width="6.5" height="6.5" rx="1" class="thumb-rect" />
              <rect x="1" y="8.5" width="6.5" height="6.5" rx="1" class="thumb-rect" />
              <rect x="8.5" y="8.5" width="6.5" height="6.5" rx="1" class="thumb-rect" />
            </svg>
            <!-- 2 Columns -->
            <svg v-else-if="session.preset === '2-columns'" viewBox="0 0 16 16" class="thumb-svg">
              <rect x="1" y="1" width="6.5" height="14" rx="1" class="thumb-rect" />
              <rect x="8.5" y="1" width="6.5" height="14" rx="1" class="thumb-rect" />
            </svg>
            <!-- 2 Rows -->
            <svg v-else-if="session.preset === '2-rows'" viewBox="0 0 16 16" class="thumb-svg">
              <rect x="1" y="1" width="14" height="6.5" rx="1" class="thumb-rect" />
              <rect x="1" y="8.5" width="14" height="6.5" rx="1" class="thumb-rect" />
            </svg>
            <!-- 3 Columns -->
            <svg v-else-if="session.preset === '3-columns'" viewBox="0 0 16 16" class="thumb-svg">
              <rect x="1" y="1" width="4" height="14" rx="1" class="thumb-rect" />
              <rect x="6" y="1" width="4" height="14" rx="1" class="thumb-rect" />
              <rect x="11" y="1" width="4" height="14" rx="1" class="thumb-rect" />
            </svg>
            <!-- Single -->
            <svg v-else viewBox="0 0 16 16" class="thumb-svg">
              <rect x="1" y="1" width="14" height="14" rx="1" class="thumb-rect primary" />
            </svg>
          </div>

          <div class="card-info">
            <span class="session-name truncate">{{ session.name }}</span>
            <div class="session-meta">
              <span
                class="status-dot"
                :class="{ active: session.runningCount > 0 }"
              />
              <span class="running-text">
                {{ session.runningCount > 0 ? `${session.runningCount} đang chạy` : 'Nghỉ (Idle)' }}
              </span>
              <span v-if="session.ramUsage" class="ram-pill">{{ session.ramUsage }}</span>
            </div>
          </div>
        </div>

        <!-- Tags / Technologies -->
        <div v-if="session.tags && session.tags.length > 0" class="card-tags">
          <span v-for="tag in session.tags" :key="tag" class="tag-chip">
            {{ tag }}
          </span>
        </div>
      </div>
    </div>

    <!-- Bottom Shortcuts Legend -->
    <div v-if="!isCollapsed" class="drawer-footer">
      <div class="shortcut-item">
        <kbd>Ctrl+Alt+R</kbd>
        <span>Chia phải</span>
      </div>
      <div class="shortcut-item">
        <kbd>Ctrl+Alt+D</kbd>
        <span>Chia dưới</span>
      </div>
      <div class="shortcut-item">
        <kbd>Ctrl+Shift+Z</kbd>
        <span>Zoom 100%</span>
      </div>
    </div>
  </aside>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import {
  FolderKanban,
  Plus,
  PanelLeftClose,
  PanelLeftOpen,
} from 'lucide-vue-next';
import type { WorkspaceSessionItem } from '@/types/tiling';

defineProps<{
  sessions: WorkspaceSessionItem[];
  activeSessionId: string;
}>();

defineEmits<{
  (e: 'select-session', sessionId: string): void;
  (e: 'create-session'): void;
}>();

const isCollapsed = ref(false);
</script>

<style scoped>
.sessions-drawer {
  width: 220px;
  background-color: var(--bg-sidebar);
  border-right: 1px solid var(--border-subtle);
  display: flex;
  flex-direction: column;
  user-select: none;
  flex-shrink: 0;
  transition: width 0.2s ease;
}

.sessions-drawer.collapsed {
  width: 36px;
}

.drawer-header {
  height: 36px;
  padding: 0 8px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid var(--border-subtle);
  background-color: #12151f;
  flex-shrink: 0;
}

.header-title-box {
  display: flex;
  align-items: center;
  gap: 6px;
  overflow: hidden;
}

.header-icon {
  color: var(--primary-light, #e4b5ff);
  flex-shrink: 0;
}

.header-title {
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.05em;
  color: var(--text-secondary);
  white-space: nowrap;
}

.header-actions {
  display: flex;
  align-items: center;
  gap: 2px;
}

.btn-icon {
  width: 22px;
  height: 22px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: none;
  background: transparent;
  color: var(--text-muted);
  border-radius: 3px;
  cursor: pointer;
  padding: 0;
}

.btn-icon:hover {
  color: var(--text-primary);
  background-color: var(--bg-surface-hover);
}

.drawer-content {
  flex: 1;
  overflow-y: auto;
  padding: 6px;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.session-card {
  padding: 6px 8px;
  border-radius: 4px;
  background-color: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  cursor: pointer;
  display: flex;
  flex-direction: column;
  gap: 4px;
  transition: all 0.15s ease;
}

.session-card:hover {
  border-color: rgba(116, 71, 145, 0.5);
  background-color: var(--bg-surface-hover);
}

.session-card.active {
  border-color: var(--primary);
  background-color: rgba(116, 71, 145, 0.15);
  box-shadow: inset 0 0 0 1px var(--primary);
}

.card-top {
  display: flex;
  align-items: center;
  gap: 8px;
}

.layout-thumb {
  width: 22px;
  height: 22px;
  flex-shrink: 0;
}

.thumb-svg {
  width: 100%;
  height: 100%;
}

.thumb-rect {
  fill: #1a1e2b;
  stroke: #4c444f;
  stroke-width: 0.8;
}

.thumb-rect.primary {
  fill: var(--primary);
  stroke: var(--primary-light, #e4b5ff);
}

.card-info {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 1px;
}

.session-name {
  font-size: 11px;
  font-weight: 600;
  color: var(--text-primary);
}

.session-meta {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 10px;
  color: var(--text-muted);
}

.status-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background-color: var(--status-idle);
  flex-shrink: 0;
}

.status-dot.active {
  background-color: var(--status-running);
  box-shadow: 0 0 5px rgba(16, 185, 129, 0.6);
}

.running-text {
  font-size: 9px;
  white-space: nowrap;
}

.ram-pill {
  font-size: 9px;
  padding: 0 3px;
  border-radius: 2px;
  background-color: #12151f;
  border: 1px solid var(--border-subtle);
  color: var(--text-secondary);
  font-family: monospace;
}

.card-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 3px;
  padding-top: 2px;
}

.tag-chip {
  font-size: 8px;
  font-family: monospace;
  padding: 1px 4px;
  border-radius: 2px;
  background-color: #141721;
  border: 1px solid var(--border-subtle);
  color: var(--text-muted);
}

.drawer-footer {
  padding: 6px 8px;
  border-top: 1px solid var(--border-subtle);
  background-color: #10121a;
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 9px;
  color: var(--text-muted);
}

.shortcut-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.shortcut-item kbd {
  background-color: #1a1e2b;
  border: 1px solid var(--border-subtle);
  border-radius: 2px;
  padding: 0 3px;
  font-family: monospace;
  color: var(--text-secondary);
}
</style>
