# Command Manager Desktop

Ứng dụng Desktop Quản lý, Thực thi Lệnh và Terminal Đa nhiệm (Tauri v2 + Rust + Vue 3 + TypeScript + Vite + SQLite WAL).

## Tài liệu Dự án

- [docs/plan.md](./docs/plan.md): Báo cáo nghiên cứu & thiết kế kiến trúc hệ thống
- [docs/checklist.md](./docs/checklist.md): Danh mục kiểm tra triển khai MVP
- [docs/screens.md](./docs/screens.md): Đặc tả thiết kế chi tiết giao diện các màn hình (SCR-01…SCR-05 & Modals)
- [docs/structure.md](./docs/structure.md): Bản đồ cấu trúc thư mục codebase frontend & backend

---

## Tính năng Cốt lõi (Frontend MVP)

- **SCR-01 Terminal Workspace:** Quản lý cây nhóm lệnh, khởi chạy phiên `run_session`, giao diện tab xterm.js với hợp đồng vòng đời: **Đóng tab chỉ ẩn UI, tiến trình vẫn chạy ngầm**. Hỗ trợ nút Reattach xả lại log gần đây từ Ring Buffer in-memory.
- **SCR-02 Thư viện Lệnh (Command Library):** CRUD danh mục câu lệnh độc lập (`command_definition`), phân biệt rõ `Direct Argv` vs `Shell (bash/sh)`.
- **SCR-03 Nhóm Lệnh & Sequencer:** Quản lý nhóm (`command_group`), bật/tắt cờ `autostart`, và sắp xếp thứ tự thực thi tuần tự (`execution_order`).
- **SCR-04 Lịch Sử Phiên Chạy:** Giám sát `run_session` và nhật ký sự kiện `run_event` chi tiết (mã thoát exit code, thời lượng, PID chẩn đoán tạm thời).
- **SCR-05 Cài Đặt & Sao Lưu DB:** Cấu hình tự khởi động cùng OS, kích thước Ring Buffer in-memory (1MB/2MB), timeout dừng mềm (Graceful Shutdown) và quy trình Sao lưu (VACUUM INTO) / Khôi phục SQLite an toàn kèm tự động Rollback.
- **Titlebar & Command Palette:** Tích hợp thanh tìm kiếm nhanh toàn cục (`Ctrl + K`), phím tắt điều hướng nhanh (`Ctrl + 1` .. `Ctrl + 5`).
- **Graceful Shutdown Overlay:** Chặn sự kiện đóng cửa sổ để dừng an toàn các tiến trình con bằng tín hiệu SIGTERM trước khi cưỡng chế thoát.

---

## Cấu trúc Frontend (`src/`)

```
src/
├── main.ts
├── App.vue                       # Root app shell
├── env.d.ts
├── router.ts                     # Vue Router: /workspace, /commands, /groups, /history, /settings
├── styles/
│   ├── tokens.css                # Dark palette, Primary #744791, Process states
│   └── global.css                # Reset, scrollbar, xterm & dockview dark theme
├── layouts/
│   └── AppShell.vue              # Titlebar + ActivityBar (60px) + Content + StatusBar (28px)
├── views/
│   ├── workspace/WorkspaceView.vue     # SCR-01
│   ├── commands/CommandLibraryView.vue # SCR-02
│   ├── groups/GroupsView.vue           # SCR-03
│   ├── history/HistoryView.vue         # SCR-04
│   └── settings/SettingsView.vue       # SCR-05
├── components/
│   ├── titlebar/ (Titlebar.vue, CommandPalette.vue)
│   ├── nav/ (ActivityBar.vue, SidePanel.vue)
│   ├── statusbar/StatusBar.vue
│   ├── terminal/ (DockHost.vue, XtermPane.vue)
│   ├── explorer/GroupTree.vue
│   └── dialogs/ (ConfirmDialog.vue, StopProcessModal.vue, CommandEditorModal.vue, RestoreWizard.vue, ShutdownOverlay.vue)
├── composables/
│   ├── useCommands.ts
│   ├── useGroups.ts
│   ├── useRunSession.ts
│   ├── usePtyStream.ts
│   └── useAppLifecycle.ts
├── ipc/
│   ├── client.ts                 # Tauri invoke wrappers + Browser Mock fallback
│   └── events.ts                 # Tauri IPC event constants
└── types/
    └── models.ts                 # SQLite schema & state TypeScript interfaces
```

---

## Hướng dẫn Khởi chạy Frontend

### 1. Cài đặt Dependencies

```bash
pnpm install
```

### 2. Chạy Dev Server (Browser Preview với Mock IPC)

```bash
pnpm dev
```
Truy cập `http://localhost:5173` để trải nghiệm đầy đủ giao diện, thao tác CRUD, khởi chạy nhóm lệnh, tab terminal xterm.js và các dialogs.

### 3. Kiểm tra Kiểu TypeScript và Build Bundle

```bash
pnpm run build
```

## Hướng dẫn Khởi chạy Native Tauri

Để chạy đầy đủ backend Rust, SQLite và PTY trên Windows, cần có:

- Rust stable với target `x86_64-pc-windows-msvc` (cài qua Rustup)
- Visual Studio với workload C++/MSVC
- Microsoft Edge WebView2 Runtime

Sau khi cài Rustup, mở terminal mới để PATH nhận `%USERPROFILE%\.cargo\bin`, rồi chạy:

```bash
pnpm run tauri:dev
```

Các lệnh kiểm tra native backend:

```bash
cd src-tauri
cargo test
cargo check --features desktop
```

`pnpm dev` chỉ chạy frontend trong browser với Mock IPC; `pnpm run tauri:dev` mới chạy toàn bộ ứng dụng desktop và IPC native.

### Build bộ cài Windows

```powershell
pnpm tauri:build
# Hiện lỗi chi tiết của WiX nếu bước đóng gói MSI thất bại:
pnpm tauri:build --verbose
```

Bộ cài nằm trong `src-tauri/target/release/bundle/msi` và `bundle/nsis`.
Build giữ nguyên ICE validation của WiX; không tự bỏ kiểm tra bằng `-sval`.
Nếu log có `LGHT0217` kèm `Windows Installer Service could not be accessed`,
hãy chạy lại từ PowerShell bên ngoài sandbox để phân biệt hạn chế môi trường
với lỗi bộ cài. Lỗi `LGHT0204` kèm `ICE38`, `ICE43`, `ICE57` cần sửa template:
shortcut không advertised phải có registry key path ở `HKCU`.
Tạo được MSI chưa xác nhận việc cài đặt/nâng cấp/gỡ cài đặt trên máy đích.

### Terminal trong app không tìm thấy pnpm qua Volta

Nếu dùng thư mục Volta tùy chỉnh, lưu `VOLTA_HOME` trong Environment Variables
của tài khoản Windows; chỉ đặt biến trong một phiên shell sẽ không áp dụng
cho app mở từ desktop/autostart. PTY đọc lại môi trường Windows khi mở tab mới.
`portable-pty` tự đọc toàn bộ biến môi trường User/Machine từ Windows Registry,
bao gồm cả các biến mới được thêm sau này; app không duy trì danh sách biến
riêng cho từng công cụ. Vì vậy `PNPM_HOME`, `VOLTA_FEATURE_PNPM` hoặc biến
cấu hình khác sẽ được truyền tự động vào terminal mới. Nếu một biến có giá trị
sai hoặc rỗng, hãy sửa/xóa biến đó trong Environment Variables của Windows
thay vì thêm ngoại lệ vào code.
Sau khi đổi môi trường, mở tab terminal mới; tab đang chạy giữ môi trường cũ.

Test cục bộ (cần pnpm đã cài và `VOLTA_HOME` đã lưu trong Windows):

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib --features desktop persisted_volta_runs_pnpm_in_pty_without_inherited_home -- --ignored --nocapture
```
