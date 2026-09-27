import { createRouter, createWebHistory } from 'vue-router';
import WorkspaceView from '@/views/workspace/WorkspaceView.vue';
import CommandLibraryView from '@/views/commands/CommandLibraryView.vue';
import TemplatesView from '@/views/templates/TemplatesView.vue';
import GroupsView from '@/views/groups/GroupsView.vue';
import HistoryView from '@/views/history/HistoryView.vue';
import SettingsView from '@/views/settings/SettingsView.vue';

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: '/',
      redirect: '/workspace',
    },
    {
      path: '/workspace',
      name: 'workspace',
      component: WorkspaceView,
      meta: { title: 'Terminal Workspace - Command Manager' },
    },
    {
      path: '/commands',
      name: 'commands',
      component: CommandLibraryView,
      meta: { title: 'Thư Viện Lệnh - Command Manager' },
    },
    {
      path: '/templates',
      name: 'templates',
      component: TemplatesView,
      meta: { title: 'Thư Viện Mẫu Lệnh - Command Manager' },
    },
    {
      path: '/groups',
      name: 'groups',
      component: GroupsView,
      meta: { title: 'Nhóm Lệnh & Sequencer - Command Manager' },
    },
    {
      path: '/history',
      name: 'history',
      component: HistoryView,
      meta: { title: 'Lịch Sử Phiên Chạy - Command Manager' },
    },
    {
      path: '/settings',
      name: 'settings',
      component: SettingsView,
      meta: { title: 'Cài Đặt & Sao Lưu - Command Manager' },
    },
  ],
});

router.beforeEach((to, _from, next) => {
  if (to.meta.title) {
    document.title = to.meta.title as string;
  }
  next();
});

export default router;
