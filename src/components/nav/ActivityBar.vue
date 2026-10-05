<template>
  <aside class="activity-bar">
    <div class="nav-top">
      <router-link
        v-for="item in navItems"
        :key="item.path"
        :to="item.path"
        class="nav-button"
        :class="{ active: currentPath === item.path }"
        :title="item.label"
      >
        <component :is="item.icon" :size="20" class="nav-icon" />
        <span class="nav-label">{{ item.shortLabel }}</span>
        <div v-if="item.badge && item.badge > 0" class="badge-dot">
          {{ item.badge }}
        </div>
      </router-link>
    </div>

    <div class="nav-bottom">
      <router-link
        to="/settings"
        class="nav-button"
        :class="{ active: currentPath === '/settings' }"
        title="Cài đặt & Sao lưu"
      >
        <Settings :size="20" class="nav-icon" />
        <span class="nav-label">Cài đặt</span>
      </router-link>
    </div>
  </aside>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { useRoute } from 'vue-router';
import { Terminal, Zap, Puzzle, Layers, History, Settings } from 'lucide-vue-next';
import { useRunSession } from '@/composables/useRunSession';

const route = useRoute();
const currentPath = computed(() => route.path);
const { activeProcesses } = useRunSession();

const activeRunningCount = computed(() => {
  let count = 0;
  activeProcesses.value.forEach(p => {
    if (p.status === 'running') count++;
  });
  return count;
});

const navItems = computed(() => [
  {
    path: '/workspace',
    label: 'Bảng điều khiển & Terminal Workspace (SCR-01)',
    shortLabel: 'Terminal',
    icon: Terminal,
    badge: activeRunningCount.value,
  },
  {
    path: '/commands',
    label: 'Thư viện Lệnh (SCR-02)',
    shortLabel: 'Lệnh',
    icon: Zap,
  },
  {
    path: '/templates',
    label: 'Thư viện Mẫu Lệnh (SCR-06)',
    shortLabel: 'Template',
    icon: Puzzle,
  },
  {
    path: '/groups',
    label: 'Nhóm Lệnh & Sequencer (SCR-03)',
    shortLabel: 'Nhóm',
    icon: Layers,
  },
  {
    path: '/history',
    label: 'Lịch sử Phiên Chạy (SCR-04)',
    shortLabel: 'Lịch sử',
    icon: History,
  },
]);
</script>

<style scoped>
.activity-bar {
  width: var(--activity-bar-width);
  height: 100%;
  background-color: #0b0d13;
  border-right: 1px solid var(--border-subtle);
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  align-items: center;
  padding: 10px 0;
  user-select: none;
  flex-shrink: 0;
}

.nav-top, .nav-bottom {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  width: 100%;
}

.nav-button {
  width: 48px;
  height: 48px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 3px;
  border-radius: var(--radius-md);
  color: var(--text-secondary);
  text-decoration: none;
  position: relative;
  transition: all 0.15s ease;
}

.nav-button:hover {
  background-color: var(--bg-surface);
  color: var(--text-primary);
}

.nav-button.active {
  background-color: var(--primary-subtle);
  color: #cda8ee;
}

.nav-button.active::before {
  content: '';
  position: absolute;
  left: 0;
  top: 8px;
  bottom: 8px;
  width: 3px;
  background-color: var(--primary);
  border-radius: 0 2px 2px 0;
  box-shadow: 0 0 8px var(--primary);
}

.nav-icon {
  flex-shrink: 0;
}

.nav-label {
  font-size: 9.5px;
  font-weight: 500;
  letter-spacing: 0.1px;
}

.badge-dot {
  position: absolute;
  top: 4px;
  right: 6px;
  background-color: var(--status-running);
  color: #ffffff;
  font-size: 9px;
  font-weight: 700;
  min-width: 15px;
  height: 15px;
  border-radius: 9999px;
  display: flex;
  align-items: center;
  justify-content: center;
  box-shadow: 0 0 6px var(--status-running-glow, #10b98199);
}
</style>
