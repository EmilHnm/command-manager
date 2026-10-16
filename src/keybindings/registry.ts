import type { KeyCommand } from '@/types/keybindings';

export const KEY_CATEGORIES = [
  { id: 'app', title: 'Ứng Dụng & Điều Hướng (Application)' },
  { id: 'nav', title: 'Chuyển Màn Hình (Navigation)' },
  { id: 'workspace', title: 'Không Gian Làm Việc (Workspace)' },
  { id: 'terminal', title: 'Terminal & Gợi Ý Lệnh (Terminal & Suggest)' },
  { id: 'suggest', title: 'Popup Gợi Ý Cố Định (Fixed Suggest Controls)' },
] as const;

export const COMMAND_REGISTRY: KeyCommand[] = [
  // Application
  {
    id: 'app.commandPalette',
    title: 'Mở / đóng Command Palette',
    category: 'app',
    categoryTitle: 'Ứng Dụng & Điều Hướng (Application)',
    defaults: [{ key: 'ctrl+k', when: 'global' }],
    scopes: ['global'],
  },
  {
    id: 'app.quit',
    title: 'Thoát ứng dụng (Graceful Shutdown)',
    category: 'app',
    categoryTitle: 'Ứng Dụng & Điều Hướng (Application)',
    defaults: [{ key: 'ctrl+q', when: 'global' }],
    scopes: ['global'],
  },
  // The webview inspector exists only in debug builds.
  ...(import.meta.env.DEV
    ? [{
      id: 'app.toggleDevtools',
      title: 'Mở / đóng Inspect (chỉ bản dev)',
      category: 'app',
      categoryTitle: 'Ứng Dụng & Điều Hướng (Application)',
      defaults: [{ key: 'ctrl+shift+i', when: 'global' }],
      scopes: ['global'],
    } satisfies KeyCommand]
    : []),

  // Navigation
  {
    id: 'nav.workspace',
    title: 'Đi đến Workspace Terminal',
    category: 'nav',
    categoryTitle: 'Chuyển Màn Hình (Navigation)',
    defaults: [{ key: 'ctrl+1', when: 'global' }],
    scopes: ['global'],
  },
  {
    id: 'nav.commands',
    title: 'Đi đến Thư viện lệnh',
    category: 'nav',
    categoryTitle: 'Chuyển Màn Hình (Navigation)',
    defaults: [{ key: 'ctrl+2', when: 'global' }],
    scopes: ['global'],
  },
  {
    id: 'nav.groups',
    title: 'Đi đến Nhóm lệnh & Sequencer',
    category: 'nav',
    categoryTitle: 'Chuyển Màn Hình (Navigation)',
    defaults: [{ key: 'ctrl+3', when: 'global' }],
    scopes: ['global'],
  },
  {
    id: 'nav.history',
    title: 'Đi đến Lịch sử phiên chạy',
    category: 'nav',
    categoryTitle: 'Chuyển Màn Hình (Navigation)',
    defaults: [{ key: 'ctrl+4', when: 'global' }],
    scopes: ['global'],
  },
  {
    id: 'nav.settings',
    title: 'Đi đến Cài đặt',
    category: 'nav',
    categoryTitle: 'Chuyển Màn Hình (Navigation)',
    defaults: [
      { key: 'ctrl+5', when: 'global' },
      { key: 'ctrl+shift+s', when: 'global' },
    ],
    scopes: ['global'],
  },
  {
    id: 'nav.templates',
    title: 'Đi đến Mẫu lệnh',
    category: 'nav',
    categoryTitle: 'Chuyển Màn Hình (Navigation)',
    defaults: [{ key: 'ctrl+6', when: 'global' }],
    scopes: ['global'],
  },
  {
    id: 'nav.servers',
    title: 'Đi đến Máy chủ SSH',
    category: 'nav',
    categoryTitle: 'Chuyển Màn Hình (Navigation)',
    defaults: [{ key: 'ctrl+7', when: 'global' }],
    scopes: ['global'],
  },
  {
    id: 'nav.keybindings',
    title: 'Mở trang Phím tắt',
    category: 'nav',
    categoryTitle: 'Chuyển Màn Hình (Navigation)',
    defaults: [{ key: 'ctrl+shift+k', when: 'global' }],
    scopes: ['global'],
  },

  // Workspace
  {
    id: 'explorer.focusFilter',
    title: 'Focus ô lọc nhóm & lệnh',
    category: 'workspace',
    categoryTitle: 'Không Gian Làm Việc (Workspace)',
    defaults: [{ key: 'ctrl+f', when: 'workspace' }],
    scopes: ['workspace'],
    skipInEditable: true,
  },
  {
    id: 'workspace.newTerminal',
    title: 'Mở terminal mới (cùng cwd)',
    category: 'workspace',
    categoryTitle: 'Không Gian Làm Việc (Workspace)',
    defaults: [{ key: 'ctrl+n', when: 'workspace' }],
    scopes: ['workspace'],
  },
  {
    id: 'workspace.newWindow',
    title: 'Mở Cửa Sổ workspace mới',
    category: 'workspace',
    categoryTitle: 'Không Gian Làm Việc (Workspace)',
    defaults: [{ key: 'ctrl+shift+t', when: 'workspace' }],
    scopes: ['workspace'],
  },
  {
    id: 'workspace.splitRight',
    title: 'Chia đôi panel sang phải',
    category: 'workspace',
    categoryTitle: 'Không Gian Làm Việc (Workspace)',
    defaults: [{ key: 'ctrl+alt+r', when: 'workspace' }],
    scopes: ['workspace'],
  },
  {
    id: 'workspace.splitDown',
    title: 'Chia đôi panel xuống dưới',
    category: 'workspace',
    categoryTitle: 'Không Gian Làm Việc (Workspace)',
    defaults: [{ key: 'ctrl+alt+d', when: 'workspace' }],
    scopes: ['workspace'],
  },
  {
    id: 'workspace.balanceSplits',
    title: 'Cân bằng tỉ lệ chia 50:50',
    category: 'workspace',
    categoryTitle: 'Không Gian Làm Việc (Workspace)',
    defaults: [{ key: 'ctrl+alt+0', when: 'workspace' }],
    scopes: ['workspace'],
  },
  {
    id: 'workspace.toggleZoom',
    title: 'Phóng to / khôi phục panel',
    category: 'workspace',
    categoryTitle: 'Không Gian Làm Việc (Workspace)',
    defaults: [{ key: 'ctrl+shift+z', when: 'workspace' }],
    scopes: ['workspace'],
  },
  {
    id: 'workspace.toggleSync',
    title: 'Bật / tắt phát phím đồng bộ',
    category: 'workspace',
    categoryTitle: 'Không Gian Làm Việc (Workspace)',
    defaults: [{ key: 'ctrl+alt+s', when: 'workspace' }],
    scopes: ['workspace'],
  },
  {
    id: 'workspace.closePanel',
    title: 'Đóng panel đang chọn',
    category: 'workspace',
    categoryTitle: 'Không Gian Làm Việc (Workspace)',
    defaults: [{ key: 'ctrl+shift+w', when: 'workspace' }],
    scopes: ['workspace'],
  },

  // Terminal & Suggestions
  {
    id: 'terminal.toggleSuggest',
    title: 'Bật / tắt popup gợi ý',
    category: 'terminal',
    categoryTitle: 'Terminal & Gợi Ý Lệnh (Terminal & Suggest)',
    defaults: [{ key: 'ctrl+space', when: 'terminal' }],
    scopes: ['terminal'],
  },
  {
    id: 'terminal.copy',
    title: 'Sao chép vùng chọn',
    category: 'terminal',
    categoryTitle: 'Terminal & Gợi Ý Lệnh (Terminal & Suggest)',
    defaults: [{ key: 'ctrl+shift+c', when: 'terminal' }],
    scopes: ['terminal'],
  },
  {
    id: 'terminal.paste',
    title: 'Dán vào terminal',
    category: 'terminal',
    categoryTitle: 'Terminal & Gợi Ý Lệnh (Terminal & Suggest)',
    defaults: [{ key: 'ctrl+shift+v', when: 'terminal' }],
    scopes: ['terminal'],
  },
  {
    id: 'terminal.find',
    title: 'Tìm kiếm trong terminal',
    category: 'terminal',
    categoryTitle: 'Terminal & Gợi Ý Lệnh (Terminal & Suggest)',
    defaults: [{ key: 'ctrl+shift+f', when: 'terminal' }],
    scopes: ['terminal'],
  },
  {
    id: 'terminal.kill',
    title: 'Dừng tiến trình terminal đang focus',
    category: 'terminal',
    categoryTitle: 'Terminal & Gợi Ý Lệnh (Terminal & Suggest)',
    defaults: [{ key: 'ctrl+shift+x', when: 'terminal' }],
    scopes: ['terminal'],
  },
  {
    id: 'terminal.acceptSuggestWord',
    title: 'Nhận một từ của gợi ý mờ',
    category: 'terminal',
    categoryTitle: 'Terminal & Gợi Ý Lệnh (Terminal & Suggest)',
    defaults: [{ key: 'ctrl+right', when: 'suggest' }],
    scopes: ['suggest'],
  },
  {
    id: 'terminal.acceptSuggest',
    title: 'Nhận toàn bộ gợi ý mờ',
    category: 'terminal',
    categoryTitle: 'Terminal & Gợi Ý Lệnh (Terminal & Suggest)',
    defaults: [
      { key: 'end', when: 'suggest' },
      { key: 'right', when: 'suggest' },
    ],
    scopes: ['suggest'],
    locked: true,
  },

  // Fixed Suggest Controls
  {
    id: 'suggest.navigate',
    title: 'Di chuyển trong danh sách gợi ý',
    category: 'suggest',
    categoryTitle: 'Popup Gợi Ý Cố Định (Fixed Suggest Controls)',
    defaults: [
      { key: 'up', when: 'suggest' },
      { key: 'down', when: 'suggest' },
    ],
    scopes: ['suggest'],
    locked: true,
  },
  {
    id: 'suggest.accept',
    title: 'Chọn gợi ý lệnh',
    category: 'suggest',
    categoryTitle: 'Popup Gợi Ý Cố Định (Fixed Suggest Controls)',
    defaults: [{ key: 'enter', when: 'suggest' }],
    scopes: ['suggest'],
    locked: true,
  },
  {
    id: 'suggest.close',
    title: 'Đóng popup gợi ý',
    category: 'suggest',
    categoryTitle: 'Popup Gợi Ý Cố Định (Fixed Suggest Controls)',
    defaults: [{ key: 'escape', when: 'suggest' }],
    scopes: ['suggest'],
    locked: true,
  },
];

export const COMMAND_MAP = new Map<string, KeyCommand>(
  COMMAND_REGISTRY.map((cmd) => [cmd.id, cmd])
);
