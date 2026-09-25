# Checklist Triển Khai MVP — Command Manager

> **Cập nhật ngày:** 25/09/2026 | **Tình trạng tổng thể:** Hoàn thành khung kiến trúc cốt lõi & giao diện MVP (Rust Backend + Vue 3 Frontend).  
> Nguồn tham chiếu: [plan.md](./plan.md) | [structure.md](./structure.md) | [screens.md](./screens.md).

---

## 0. Chốt Kiến Trúc & Quyết Định Kỹ Thuật (Pre-Code Decisions)

- [x] **Chốt Frontend Stack:** Đã chọn **Vue 3 + TypeScript + Vite + dockview-vue** (kèm bộ tab host tuỳ chỉnh tại [DockHost.vue](../src/components/terminal/DockHost.vue) và [XtermPane.vue](../src/components/terminal/XtermPane.vue)).
- [x] **Chốt Stack PTY & Đa nền:** Sử dụng crate Rust `portable-pty` v0.8 tại [session.rs](../src-tauri/src/pty/session.rs), hỗ trợ Linux/Unix qua `nix` signals và Windows qua adapter taskkill/process group.
- [x] **Chốt Chính Sách Backpressure:** Đã triển khai tại [backpressure.rs](../src-tauri/src/pty/backpressure.rs): Ring Buffer in-memory luôn ghi trọn vẹn luồng dữ liệu; IPC pipe dùng kênh không khóa `try_send` với chunk 8KB (drop gói IPC khi UI quá tải, không chặn PTY reader; UI có thể gọi `pty_reattach` để lấy snapshot từ Ring Buffer).
- [x] **Chốt Kích Thước Ring Buffer & Timeout Shutdown:** Mặc định `ring_buffer_bytes = 2097152` (2MB) và `shutdown_timeout_secs = 8` (8 giây) trong SQLite `app_setting` ([001_init.sql](../src-tauri/migrations/001_init.sql)). Cho phép tinh chỉnh từ 1MB/2MB/4MB và 5–15s tại màn hình [SettingsView.vue](../src/views/settings/SettingsView.vue).
- [x] **Làm Rõ Trusted Zone:** Lệnh trong cơ sở dữ liệu được định nghĩa là **trusted**, không chạy sandbox. Đã đưa cảnh báo xác nhận vào [RestoreWizard.vue](../src/components/dialogs/RestoreWizard.vue) và cơ chế xác thực quyền thực thi.

---

## 1. Scaffold và Nền Tảng (Core Scaffold)

- [x] **Khung ứng dụng Tauri v2:** Rust (Tokio Runtime) tại `src-tauri/` + Frontend TypeScript & Vite tại `src/` (Build & type-check sạch qua `vue-tsc` và `cargo check --features desktop`).
- [x] **IPC & Capabilities tối thiểu:** Cấu hình quyền hạn chặt chẽ tại [default.json](../src-tauri/capabilities/default.json), chỉ mở window, event và autostart; không mở shell hay filesystem tùy tiện ra ngoài WebView.
- [x] **Chính sách CSP nghiêm ngặt:** Đã kích hoạt `"csp": "default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:"` trong [tauri.conf.json](../src-tauri/tauri.conf.json).
- [x] **SQLite WAL Connection Pool:** Đã triển khai tại [pool.rs](../src-tauri/src/db/pool.rs): 1 Mutex writer duy nhất + 3 Mutex readers, kích hoạt `PRAGMA journal_mode = WAL`, `foreign_keys = ON`, `busy_timeout = 5000ms`.
- [x] **Kiểm tra Schema Version:** Bảng `schema_version` lưu phiên bản DB hiện hành (v1), phục vụ xác thực trước khi Import bản sao lưu.

---

## 2. Dữ Liệu Tĩnh (SQLite Persistence)

- [x] **Bảng `command_definition`:** `id (TEXT)`, `name`, `execution_string`, `is_shell (0/1)` trong [001_init.sql](../src-tauri/migrations/001_init.sql).
- [x] **Bảng `command_group`:** `id (TEXT)`, `group_name`, `autostart (0/1)`.
- [x] **Bảng `group_membership`:** `group_id`, `command_id`, `execution_order` (khoá chính tổng hợp + ràng buộc UNIQUE theo group_id và execution_order).
- [x] **Bảng `run_session`:** `id (TEXT)`, `group_id`, `started_at`, `status`.
- [x] **Bảng `run_event`:** `id (TEXT)`, `session_id`, `command_id`, `started_at`, `ended_at`, `status`, `exit_code`, `pid`.
- [x] **CRUD Lệnh / Nhóm / Thứ Tự Chạy:**
  - Backend Rust: [commands.rs](../src-tauri/src/db/repos/commands.rs), [groups.rs](../src-tauri/src/db/repos/groups.rs).
  - Frontend UI: [CommandLibraryView.vue](../src/views/commands/CommandLibraryView.vue) (SCR-02), [GroupsView.vue](../src/views/groups/GroupsView.vue) (SCR-03 - Sequencer nâng/hạ thứ tự).
- [x] **Phân định rõ ràng `is_shell`:** 
  - Khi `is_shell == true`: khởi chạy qua `sh -c "<string>"`.
  - Khi `is_shell == false`: phân tích chuỗi an toàn bằng `shell_words::split` để lấy mảng `argv` trực tiếp ([pty/session.rs](../src-tauri/src/pty/session.rs)).
- [x] **DB chỉ ghi nhận Lifecycle:** SQLite chỉ ghi khi bắt đầu tiến trình (`mark_event_started`) và khi tiến trình kết thúc (`mark_event_ended` kèm exit code). Tuyệt đối không ghi stream PTY vào đĩa.

---

## 3. Process Manager (Trạng Thái Động In-Memory)

- [x] **Bộ quản lý tiến trình in-memory:** [manager.rs](../src-tauri/src/process/manager.rs) sử dụng `DashMap<String, Arc<Live>>` lưu giữ `pid`, `pty` handle và `RingBuffer`.
- [x] **Chạy nhóm tuần tự theo `execution_order`:** [commands.rs](../src-tauri/src/ipc/commands.rs) hàm `start_group_inner` truy vấn danh sách thành viên nhóm đã sắp xếp tăng dần theo `execution_order`.
- [x] **Điều khiển Start / Stop linh hoạt:**
  - Hỗ trợ dừng theo từng tiến trình (`process_stop`) và dừng theo phiên nhóm (`session_stop`).
- [x] **Vai trò của trường `pid`:** `pid` chỉ được lưu tạm để chẩn đoán (hiển thị trên giao diện và kiểm toán lịch sử), không dùng để reattach sau khi ứng dụng khởi động lại.
- [x] **Hợp đồng App-Bound:** Mọi tiến trình con gắn liền với vòng đời của ứng dụng; thoát ứng dụng sẽ dừng toàn bộ tiến trình.
- [x] **Đóng tab UI ≠ Dừng tiến trình:** Đóng thẻ tab tại [DockHost.vue](../src/components/terminal/DockHost.vue) chỉ ẩn giao diện terminal, tiến trình ở Rust vẫn tiếp tục chạy ngầm và ghi nhận log vào Ring Buffer.
- [x] **Chính sách dừng duyên dáng (Graceful Shutdown):**
  - Đã triển khai tại [shutdown.rs](../src-tauri/src/process/shutdown.rs): Gửi `SIGTERM` / `taskkill` → chờ thời gian timeout (5–10s) → cưỡng chế dừng bằng `SIGKILL` / `taskkill /F /T` nếu còn tiến trình sót lại.
  - Giao diện người dùng chặn đóng cửa sổ và hiển thị [ShutdownOverlay.vue](../src/components/dialogs/ShutdownOverlay.vue) (MOD-09).
- [x] **Adapter nền tảng Windows:** [windows.rs](../src-tauri/src/process/platform/windows.rs) dùng `taskkill` và cờ `/T` để tiêu diệt toàn bộ cây tiến trình con.
- [~] **Linux `PR_SET_PDEATHSIG` trước `exec`:** Hàm `pdeathsig_pre_exec` đã được hiện thực trong [unix.rs](../src-tauri/src/process/platform/unix.rs). *(Ghi chú: portable-pty 0.8 chưa mở hook pre-exec công khai; cần chuyển hướng sang std::process::Command nếu cần ghim trực tiếp).*

---

## 4. Terminal PTY & Hiển Thị (xterm.js)

- [x] **Pseudo-Terminal thật:** Sử dụng `portable_pty::native_pty_system()` tạo cặp master-slave PTY đầy đủ khả năng TTY & mã màu ANSI ([session.rs](../src-tauri/src/pty/session.rs)).
- [x] **Byte thô kèm ANSI đến xterm.js:** Reader luồng PTY đẩy dữ liệu mã hóa base64 qua sự kiện `pty://data` tới xterm.js trên giao diện.
- [x] **Kiểm soát áp lực ngược (Backpressure):** Triển khai `IpcPipe` non-blocking; Ring Buffer bảo lưu toàn vẹn log mới nhất trong bộ nhớ đệm byte.
- [x] **Đồng bộ kích thước (Resize):** [XtermPane.vue](../src/components/terminal/XtermPane.vue) dùng `ResizeObserver` + `FitAddon` gửi invoke `pty_resize(cols, rows)` cập nhật trực tiếp đến kernel PTY.
- [x] **Gắn lại Terminal (Reattach Buffer):** Khi mở lại tab đã ẩn, gọi invoke `pty_reattach` để lấy toàn bộ dữ liệu gần đây từ [RingBuffer](../src-tauri/src/pty/buffer.rs) và xả vào màn hình xterm.js.
- [x] **Giới hạn phạm vi Reattach:** Ghi rõ tài liệu: cơ chế Ring Buffer thô hỗ trợ xem lại recent log, không khôi phục trạng thái toàn màn hình chuyên biệt (vim/htop).
- [x] **Bảo mật bí mật trong PTY:** Tuyệt đối không ghi luồng stream PTY ra đĩa cứng để tránh lộ mật khẩu người dùng nhập vào terminal.

---

## 5. Giao Diện Người Dùng (Frontend UI MVP)

- [x] **SCR-01 Terminal Workspace:** Quản lý không gian làm việc terminal dạng thẻ, cây thư mục nhóm lệnh bên trái ([WorkspaceView.vue](../src/views/workspace/WorkspaceView.vue)).
- [x] **SCR-02 Thư viện Lệnh (Command Library):** Bảng quản lý danh mục câu lệnh, tìm kiếm, lọc theo `argv`/`shell`, tạo/sửa/nhân bản lệnh ([CommandLibraryView.vue](../src/views/commands/CommandLibraryView.vue)).
- [x] **SCR-03 Nhóm Lệnh & Sequencer:** Quản lý nhóm, cờ autostart, sắp xếp thứ tự thực thi câu lệnh ([GroupsView.vue](../src/views/groups/GroupsView.vue)).
- [x] **SCR-04 Lịch Sử Phiên Chạy:** Giám sát các phiên chạy `run_session` và từng sự kiện `run_event` chi tiết (mã thoát, thời gian chạy, PID chẩn đoán) ([HistoryView.vue](../src/views/history/HistoryView.vue)).
- [x] **SCR-05 Cài Đặt Hệ Thống:** Cấu hình autostart OS, kích thước Ring Buffer, timeout dừng mềm, và bảng điều khiển sao lưu/khôi phục ([SettingsView.vue](../src/views/settings/SettingsView.vue)).
- [x] **Thanh Tiêu Đề & Command Palette:** Tích hợp thanh tìm kiếm toàn cục `Ctrl + K` ([Titlebar.vue](../src/components/titlebar/Titlebar.vue), [CommandPalette.vue](../src/components/titlebar/CommandPalette.vue)).
- [x] **Thanh Trạng Thái (StatusBar):** Hiển thị số lượng tiến trình đang chạy, dung lượng Ring Buffer, chế độ SQLite WAL và trạng thái Single-Instance Lock ([StatusBar.vue](../src/components/statusbar/StatusBar.vue)).

---

## 6. Tự Khởi Động & Khóa Đơn Bản Thể (Autostart & Single-Instance)

- [x] **Tích hợp `tauri-plugin-autostart`:** Hỗ trợ đăng ký khởi động cùng hệ điều hành cho cả Windows và Linux/macOS ([autostart.rs](../src-tauri/src/app/autostart.rs)).
- [x] **Single-Instance Lock trước khi tạo Process Manager:** Plugin `tauri-plugin-single-instance` được kích hoạt ngay trong `Builder::default()` trước hook `setup` ([lib.rs](../src-tauri/src/lib.rs)).
- [x] **Xử lý Instance thứ hai:** Khi mở bản thể thứ hai, [single_instance.rs](../src-tauri/src/app/single_instance.rs) tự động hiển thị cửa sổ hiện hành (`w.show()`), lấy tiêu điểm (`w.set_focus()`) và phát sự kiện `app://instance` rồi thoát instance mới.
- [x] **Khởi chạy nhóm có cờ Autostart an toàn:** Sau khi giữ lock, backend duyệt các nhóm có `autostart = 1` và kiểm tra `if processes.group_has_live(&g.id) { continue; }` để không kích hoạt lại nhóm đã đang chạy ([autostart.rs](../src-tauri/src/app/autostart.rs)).

---

## 7. Sao Lưu / Khôi Phục Dữ Liệu (Backup & Restore)

### Xuất Bản Sao Lưu (Export)
- [x] **`VACUUM INTO`:** Sử dụng câu lệnh chuẩn SQLite `VACUUM INTO` để tạo bản snapshot độc lập ngay cả khi DB đang mở ở chế độ WAL ([export.rs](../src-tauri/src/backup/export.rs)).
- [x] **Kiểm tra tính toàn vẹn:** Tự động chạy `PRAGMA integrity_check` trên file sao lưu và chỉ thông báo thành công khi kết quả trả về là `"ok"`.

### Nhập Bản Sao Lưu (Import & Rollback)
- [x] **Kiểm tra sơ bộ tệp sao lưu:** Chạy `PRAGMA integrity_check` và kiểm tra `read_schema_version(src) == SCHEMA_VERSION` ([import.rs](../src-tauri/src/backup/import.rs)).
- [x] **Xác nhận đặc quyền từ người dùng:** Yêu cầu cờ `confirmed = true` từ IPC và hiển thị hộp thoại cảnh báo [RestoreWizard.vue](../src/components/dialogs/RestoreWizard.vue) (MOD-08).
- [x] **Tạo bản sao lưu dự phòng (Snapshot Rollback):** Tự động copy `app.db` hiện tại sang `app.db.rollback` trước khi thực hiện ghi đè.
- [x] **Dừng tiến trình & Đóng kết nối:** Gọi `shutdown_all` dừng toàn bộ lệnh đang chạy và gọi `db.close_all()` đóng toàn bộ pool kết nối SQLite.
- [x] **Dọn dẹp WAL Set:** Xóa sạch bộ ba `app.db`, `app.db-wal`, `app.db-shm` trước khi copy file mới vào vị trí.
- [x] **Tự động Rollback khi lỗi:** Mở lại DB bằng `db.reopen()`; nếu phát sinh lỗi sẽ tự động hoàn trả từ `app.db.rollback`.

---

## 8. Bảo Mật & Ranh Giới An Toàn (Security Hardening)

- [x] **Kiểm soát thao tác đặc quyền:** Mọi thao tác thêm/sửa/xóa câu lệnh, thay đổi autostart nhóm và nhập dữ liệu sao lưu đều yêu cầu cờ `confirmed: bool` ở tầng backend Rust ([ipc/commands.rs](../src-tauri/src/ipc/commands.rs)).
- [x] **Cảnh báo nguồn ngoài:** Hộp thoại khôi phục hiển thị rõ cảnh báo chỉ nạp tệp từ nguồn tin cậy (Trusted Zone).
- [x] **Ranh giới Capability:** Không cho phép WebView gọi thẳng các lệnh spawn hệ thống; WebView chỉ có thể gửi yêu cầu thực thi Command ID đã được lưu hợp lệ trong SQLite.

---

## 9. Nhiệm Vụ Hoàn Thiện Tích Hợp (Wiring & Polish Tasks)

- [x] Đã cấu hình và kiểm tra build thành công frontend (`npm run build`).
- [x] Đã viết và vượt qua các unit tests cốt lõi ở Rust backend (`cargo test` pass 4/4 tests: ring buffer overwrite, oversize buffer write, empty manager shutdown, vacuum into integrity check).
- [x] Đã xác minh khả năng biên dịch đầy đủ tính năng desktop (`cargo check --features desktop`).
- [ ] **Chuyển đổi IPC Client từ Mock sang Native Tauri IPC:**
  - File [client.ts](../src/ipc/client.ts) hiện có cờ `USE_MOCK_IPC = true` phục vụ chạy độc lập trên trình duyệt. Cần hoàn tất chuyển đổi mapping kiểu ID (UUID `string` ở Rust vs mock `number` ở frontend) khi chạy hoàn toàn trong môi trường Tauri.
- [ ] **Gắn kết phím tắt toàn cục:** Bổ sung hotkey `Ctrl+1` đến `Ctrl+5` và phím tắt điều hướng nhanh trong AppShell.

---

## 10. Kế Hoạch Sau MVP (Post-MVP Scope)

- [ ] **Systemd Services Exporter:** Hỗ trợ xuất nhóm lệnh thành service unit file chạy trên Linux nền server.
- [ ] **Biểu mẫu động JSON Schema (RJSF):** Hỗ trợ sinh giao diện tham số dòng lệnh động cho các công cụ như `ffmpeg`, `cloudflared`.
- [ ] **Lưới đa cửa sổ (Dockview Grid Layout):** Cho phép kéo thả chia ô chia cột lưới đa nhiệm phức tạp.
- [ ] **Đồng bộ đám mây (Google Drive Sync):** Lưu trữ token xác thực trong OS Keyring / Credential Manager.
