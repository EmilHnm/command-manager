# Checklist Triển Khai MVP — Command Manager

> **Cập nhật ngày:** 26/09/2026 (soát lại theo code thực tế) | **Tình trạng tổng thể:** Khung kiến trúc & giao diện MVP chạy được; Gate 11 đã sửa các lỗi native chặn chính, còn QA thủ công/native và Linux runtime mở — xem ghi chú `Soát 26/09` ở từng mục và danh sách lỗi ở §12.
> **Ký hiệu:** `[x]` xong, `[~]` làm một phần / có lỗi, `[ ]` chưa làm hoặc mô tả cũ không đúng với code.  
> Nguồn tham chiếu: [plan.md](./plan.md) | [structure.md](./structure.md) | [screens.md](./screens.md).

---

## 0. Chốt Kiến Trúc & Quyết Định Kỹ Thuật (Pre-Code Decisions)

- [~] **Chốt Frontend Stack:** Đã chọn **Vue 3 + TypeScript + Vite + dockview-vue** (kèm bộ tab host tuỳ chỉnh tại [DockHost.vue](../src/components/terminal/DockHost.vue) và [XtermPane.vue](../src/components/terminal/XtermPane.vue)). *— Soát 26/09: `dockview-vue` có trong package.json nhưng chưa được import; `DockHost.vue` là tab list tự viết, chỉ dùng CSS dockview.*
- [x] **Chốt Stack PTY & Đa nền:** Sử dụng crate Rust `portable-pty` v0.8 tại [session.rs](../src-tauri/src/pty/session.rs), hỗ trợ Linux/Unix qua `nix` signals và Windows qua adapter taskkill/process group. *— Soát 26/09: Windows chỉ dùng `taskkill /T`, chưa có Job Object/process group.*
- [x] **Chốt Chính Sách Backpressure:** Đã triển khai tại [backpressure.rs](../src-tauri/src/pty/backpressure.rs): Ring Buffer in-memory luôn ghi trọn vẹn luồng dữ liệu; IPC pipe dùng kênh không khóa `try_send` với chunk 8KB (drop gói IPC khi UI quá tải, không chặn PTY reader; UI có thể gọi `pty_reattach` để lấy snapshot từ Ring Buffer). *— Soát 26/09: hằng `IPC_CHUNK` khai báo nhưng không dùng; kênh mpsc dung lượng 32.*
- [x] **Chốt Kích Thước Ring Buffer & Timeout Shutdown:** Mặc định `ring_buffer_bytes = 2097152` (2MB) và `shutdown_timeout_secs = 8` (8 giây) trong SQLite `app_setting` ([001_init.sql](../src-tauri/migrations/001_init.sql)). Cho phép tinh chỉnh từ 1MB/2MB/4MB và 5–15s tại màn hình [SettingsView.vue](../src/views/settings/SettingsView.vue). *— Soát 26/09: `backup/import.rs` hardcode 8s thay vì đọc setting.*
- [x] **Làm Rõ Trusted Zone:** Lệnh trong cơ sở dữ liệu được định nghĩa là **trusted**, không chạy sandbox. Đã đưa cảnh báo xác nhận vào [RestoreWizard.vue](../src/components/dialogs/RestoreWizard.vue) và cơ chế xác thực quyền thực thi. *— Soát 26/09: chỉ có cảnh báo UI, không có cơ chế xác thực quyền thực thi riêng.*

---

## 1. Scaffold và Nền Tảng (Core Scaffold)

- [x] **Khung ứng dụng Tauri v2:** Rust (Tokio Runtime) tại `src-tauri/` + Frontend TypeScript & Vite tại `src/` (Build & type-check sạch qua `vue-tsc` và `cargo check --features desktop`).
- [x] **IPC & Capabilities tối thiểu:** Cấu hình quyền hạn tại [default.json](../src-tauri/capabilities/default.json), chỉ mở window, event và autostart; không mở shell hay filesystem tùy tiện ra ngoài WebView. *— Sửa 27/09: frontend gọi `app_shutdown`, backend dọn tiến trình và `app.exit(0)`; không còn gọi `window.destroy()` nên không cần quyền `allow-destroy`.*
- [x] **Chính sách CSP:** Đã kích hoạt `"csp": "default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:"` trong [tauri.conf.json](../src-tauri/tauri.conf.json). *— Sửa 27/09: `dangerousDisableAssetCspModification: ["style-src"]` ngăn Tauri tự thêm nonce/hash vào riêng CSS, để `'unsafe-inline'` cho phép xterm tạo CSS con trỏ, kích thước ô và theme lúc chạy trong bản build. CSP và cơ chế bảo vệ script vẫn bật. Đã tái hiện cơ chế chặn CSS bằng Edge headless; `pnpm tauri build --features desktop --no-bundle` đạt; còn QA trực tiếp cửa sổ release.*
- [~] **SQLite WAL Connection Pool:** Đã triển khai tại [pool.rs](../src-tauri/src/db/pool.rs): 1 Mutex writer duy nhất + 3 Mutex readers, kích hoạt `PRAGMA journal_mode = WAL`, `foreign_keys = ON`, `busy_timeout = 5000ms`. *— Soát 26/09: pragma đúng, mở 3 reader nhưng `read()` chỉ dùng `readers[0]`.*
- [x] **Kiểm tra Schema Version:** Bảng `schema_version` lưu phiên bản DB hiện hành (**v7** — `SCHEMA_VERSION = 7`, có migration privacy đến `007_history_privacy_patterns.sql`), phục vụ xác thực trước khi Import bản sao lưu.

---

## 2. Dữ Liệu Tĩnh (SQLite Persistence)

- [x] **Bảng `command_definition`:** `id (TEXT)`, `name`, `execution_string`, `is_shell (0/1)` trong [001_init.sql](../src-tauri/migrations/001_init.sql).
- [x] **Bảng `command_group`:** `id (TEXT)`, `group_name`, `autostart (0/1)`.
- [x] **Bảng `group_membership`:** `group_id`, `command_id`, `execution_order` (khoá chính tổng hợp + ràng buộc UNIQUE theo group_id và execution_order).
- [x] **Bảng `run_session`:** `id (TEXT)`, `group_id` nullable, `template_id` nullable, `started_at`, `status`; hỗ trợ session nhóm, template và lệnh đơn.
- [x] **Bảng `run_event`:** `id (TEXT)`, `session_id`, `command_id`, `started_at`, `ended_at`, `status`, `exit_code`, `pid`.
- [x] **CRUD Lệnh / Nhóm / Thứ Tự Chạy:** `replace_memberships` chạy trong transaction; xoá nhóm đã có lịch sử dùng `ON DELETE SET NULL`.
  - Backend Rust: [commands.rs](../src-tauri/src/db/repos/commands.rs), [groups.rs](../src-tauri/src/db/repos/groups.rs).
  - Frontend UI: [CommandLibraryView.vue](../src/views/commands/CommandLibraryView.vue) (SCR-02), [GroupsView.vue](../src/views/groups/GroupsView.vue) (SCR-03 - Sequencer nâng/hạ thứ tự).
- [x] **Phân định rõ ràng `is_shell`:** 
  - Khi `is_shell == true`: Unix chạy `sh -c "<string>"`; Windows chạy `%COMSPEC% /D /S /C %COMMAND_MANAGER_SHELL_COMMAND%` (chuỗi lệnh truyền qua biến môi trường).
  - Khi `is_shell == false`: dùng parser `split_argv` theo nền tảng để lấy mảng `argv` trực tiếp; trên Windows giữ nguyên dấu `\` trong path ([argv.rs](../src-tauri/src/argv.rs), [pty/session.rs](../src-tauri/src/pty/session.rs)).
- [x] **DB chỉ ghi nhận Lifecycle:** SQLite chỉ ghi khi bắt đầu tiến trình (`mark_event_started`) và khi tiến trình kết thúc (`mark_event_ended` kèm exit code). Tuyệt đối không ghi stream PTY vào đĩa. *— Soát 26/09: `command_run` và `template_run` đã có session/event; spawn lỗi được đánh dấu `failed`; terminal trống vẫn không ghi lịch sử theo thiết kế.*

---

## 3. Process Manager (Trạng Thái Động In-Memory)

- [x] **Bộ quản lý tiến trình in-memory:** [manager.rs](../src-tauri/src/process/manager.rs) sử dụng `DashMap<String, Arc<Live>>` lưu giữ `pid`, `pty` handle và `RingBuffer`. *— Soát 26/09: đã xử lý race tiến trình thoát trước khi `processes.insert` bằng pending-finished set.*
- [ ] **Chạy nhóm tuần tự theo `execution_order`:** [commands.rs](../src-tauri/src/ipc/commands.rs) vẫn spawn song song theo thứ tự thành viên; spawn lỗi không còn kẹt session nhưng behavior tuần tự thật vẫn là việc riêng.
- [x] **Điều khiển Start / Stop linh hoạt:**
  - Hỗ trợ dừng theo từng tiến trình (`process_stop`) và dừng theo phiên nhóm (`session_stop`).
- [x] **Vai trò của trường `pid`:** `pid` chỉ được lưu tạm để chẩn đoán (hiển thị trên giao diện và kiểm toán lịch sử), không dùng để reattach sau khi ứng dụng khởi động lại.
- [~] **Hợp đồng App-Bound:** Mọi tiến trình con gắn liền với vòng đời của ứng dụng; thoát ứng dụng sẽ dừng toàn bộ tiến trình. *— Soát 26/09: chỉ đúng khi đóng app bình thường; app crash/bị kill thì tiến trình con mồ côi (chưa có PDEATHSIG / Job Object).*
- [x] **Đóng tab UI ≠ Dừng tiến trình:** Đóng thẻ tab tại [DockHost.vue](../src/components/terminal/DockHost.vue) chỉ ẩn giao diện terminal, tiến trình ở Rust vẫn tiếp tục chạy ngầm và ghi nhận log vào Ring Buffer.
- [~] **Chính sách dừng duyên dáng (Graceful Shutdown):** *— Sửa 27/09: nút Close và Alt+F4 dùng worker backend, chống shutdown trùng, gửi trạng thái thực qua `app://shutdown-progress`, rồi dừng process và thoát; nút ẩn tray chỉ gọi `app_hide`. Menu tray “Thoát hoàn toàn” cũng dùng worker backend. Windows `terminate_graceful` vẫn fallback `/F` ngay nếu taskkill thường lỗi. Cần QA native khi đóng có terminal/process đang chạy.*
- [~] **Chạy nền và system tray:** Nút ẩn tray sẽ ẩn cửa sổ và giữ process; Close/Alt+F4 vẫn thoát ứng dụng. Menu tray có Mở lại và Thoát hoàn toàn, click đúp icon để mở lại. Khi bật OS Autostart, plugin truyền cờ `--command-manager-autostart` để app khởi động ẩn trong tray; mở thủ công vẫn hiện cửa sổ. Cần QA native trên Windows sau khi đăng nhập lại.
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
- [x] **Terminal shell trống:** Nút `Terminal mới` mở shell tương tác độc lập qua `terminal_open`, không cần tạo command/group và không ghi lịch sử SQLite; vẫn hỗ trợ input, resize, reattach và stop như terminal của command ([DockHost.vue](../src/components/terminal/DockHost.vue), [commands.rs](../src-tauri/src/ipc/commands.rs)).
- [x] **Giới hạn phạm vi Reattach:** Ghi rõ tài liệu: cơ chế Ring Buffer thô hỗ trợ xem lại recent log, không khôi phục trạng thái toàn màn hình chuyên biệt (vim/htop).
- [x] **Bảo mật bí mật trong PTY:** Tuyệt đối không ghi luồng stream PTY ra đĩa cứng để tránh lộ mật khẩu người dùng nhập vào terminal.

---

## 5. Giao Diện Người Dùng Chuẩn Stitch (Frontend UI - Project 14914224436748087443)

- [x] **Hệ Thống Design Tokens & Typography:** Đồng bộ chuẩn màu tối kỹ thuật (Dark Technical Precision: `#0f1117` base, `#141721` sidebar, `#1a1e2b` surface, `#744791` primary plum/violet, `#4edea3` secondary, `#4cd7f6` tertiary). Tích hợp phông chữ `Inter` (UI Chrome) và `JetBrains Mono` (mọi mã code, tham số argv, PID, exit codes) tại [tokens.css](../src/styles/tokens.css) và [index.html](../index.html). *— Soát 26/09: màu base thực tế là `#0c0e14` (`#0f1117` chỉ là nền pre-boot trong `index.html`).*
- [~] **SCR-01 Terminal Workspace & Execution Dashboard:** *— Soát 26/09: xem các dòng con.*
  - [~] Dải ruy băng trạng thái phiên chạy trên đỉnh màn hình (Session Telemetry Ribbon): hiển thị tên phiên/nhóm, ID phiên `#sess-91a`, trạng thái daemon hoạt động, bộ nhớ RAM ước tính, số lượng tiến trình con `2 / 7 active`, nút chạy/dừng toàn bộ nhóm. *— Soát 26/09: ID phiên `sess-91a` hardcode, RAM giả (`192 × running`), tổng tiến trình fallback `7`, nút Chạy Nhóm luôn chạy `groups[0]`.*
  - [~] Cột điều hướng Explorer bên trái tích hợp thẻ tóm tắt Target Group với nút bấm thao tác nhanh (`Chạy`, `Dừng`), cây tiến trình phân cấp tuần tự hiển thị số thứ tự `01.`, `02.`, thẻ kiểu `argv`/`shell`, chấm trạng thái có hoạt ảnh phát sáng (pulse) khi chạy, và nhãn chẩn đoán PID ([WorkspaceView.vue](../src/views/workspace/WorkspaceView.vue), [GroupTree.vue](../src/components/explorer/GroupTree.vue)). *— Soát 26/09: Target Group luôn là `groups[0]`; số thứ tự thành `010.` từ mục thứ 10; badge ghi `sh` thay vì `shell`; placeholder ghi Ctrl+F nhưng chưa có handler.*
  - [~] Trình duyệt Terminal tabbed dockview với thanh công cụ điều khiển PTY: Reattach, Xóa màn hình (Clear), Khởi động lại (Restart), Dừng lệnh ([DockHost.vue](../src/components/terminal/DockHost.vue), [XtermPane.vue](../src/components/terminal/XtermPane.vue)). *— Soát 26/09: không dùng dockview; nút **Khởi động lại** chỉ `console.log` (DockHost.vue).*
- [~] **SCR-02 Thư viện Lệnh (Command Registry):** *— Soát 26/09: xem các dòng con.*
  - [x] Tiêu đề tích hợp huy hiệu `v2.1 Engine`, nút `Nhập JSON` và `+ Tạo Lệnh Mới`. *— Soát 26/09: `v2.1` hardcode (app là 1.0.0).*
  - [x] 4 thẻ Quick Stats Telemetry Ribbon: `TỔNG SỐ LỆNH`, `SHELL WRAPPER`, `DIRECT ARGV`, `CHƯA GÁN NHÓM`. *— Soát 26/09: dữ liệu thật.*
  - [x] Thanh công cụ lọc tìm kiếm theo từ khóa và phân loại kiểu thực thi `Direct Argv` vs `Shell Commands`.
  - [~] Bảng Registry chi tiết: cột trạng thái tiến trình, tên lệnh + `#ID`, thẻ kiểu thực thi, đường dẫn thư mục làm việc `CWD`, khối mã lệnh monospaced kèm nút một chạm sao chép (copy), nhóm nút hành động Chạy/Sửa/Nhân bản/Xóa ([CommandLibraryView.vue](../src/views/commands/CommandLibraryView.vue)). *— Soát 26/09: cột **CWD giả** — backend không có trường `cwd`, luôn hiện `./`; `#ID` là ID số phía frontend, không phải UUID.*
  - [x] Nút `Chạy` gọi `command_run` native, mở tab PTY thật và liên kết `run_event_id` để nhập lệnh, resize, reattach và dừng tiến trình.
  - [x] `Nhập JSON` đọc mảng lệnh hoặc `{ "commands": [...] }`, lưu từng định nghĩa vào SQLite và hiển thị kết quả/lỗi trên UI. *— Soát 26/09: trường `cwd` trong JSON bị bỏ qua.*
- [~] **SCR-03 Nhóm Lệnh & Sequencer (Workflow Orchestrator):** *— Soát 26/09: xem các dòng con.*
  - [~] Tiêu đề tích hợp huy hiệu `v2.1 Orchestrator` kèm nút `Chạy Tất Cả Nhóm` và `+ Tạo Nhóm Mới`. *— Soát 26/09: nút `Chạy Tất Cả Nhóm` chỉ chạy nhóm đầu tiên.*
  - [~] 4 thẻ Telemetry Ribbon: `TỔNG SỐ NHÓM`, `AUTOSTART KÍCH HOẠT`, `DỪNG KHI LỖI`, `TỔNG LỆNH TRONG CHUỖI`. *— Soát 26/09: thẻ `DỪNG KHI LỖI` giả (hiển thị số nhóm), chưa có cờ stop-on-error.*
  - [~] Thẻ nhóm hiển thị biểu tượng drag handle, nhãn chế độ thực thi `Tuần tự`, công tắc bật/tắt Autostart. *— Soát 26/09: drag handle chỉ trang trí; nhãn `Tuần tự` tĩnh (thực tế chạy song song, xem §3).*
  - [x] Danh sách bước thực thi tuần tự có số thứ tự `01.`, `02.`, tên lệnh, đoạn mã lệnh trích dẫn, thẻ `argv`/`shell`, nút di chuyển nâng/hạ thứ tự `▲` `▼` ([GroupsView.vue](../src/views/groups/GroupsView.vue)). *— Soát 26/09: ▲/▼ hoạt động; lỗi hiển thị `010.` như GroupTree.*
- [x] **SCR-04 Lịch Sử Phiên Chạy & Sự Kiện (Audit & Diagnostics):** *— Soát 26/09: badge `v2.1` và `SQLite WAL: Synced` hardcode; tỉ lệ thành công hiện 100% khi chưa có phiên; `RING BUFFER STREAM` chỉ là số tiến trình; xem log chỉ được khi tiến trình còn trong RAM.*
  - Tiêu đề tích hợp huy hiệu `v2.1 Audit & Diagnostics` và huy hiệu `SQLite WAL: Synced`.
  - 4 thẻ Metric Ribbon: `TỔNG SỐ PHIÊN`, `TỈ LỆ THÀNH CÔNG (%)`, `PHIÊN ĐANG CHẠY`, `RING BUFFER STREAM`.
  - Thanh bên tìm kiếm lọc phiên chạy; vùng chẩn đoán chi tiết với thẻ tóm tắt phiên và bảng nhật ký sự kiện `run_event` chi tiết (thời gian bắt đầu, kết thúc, mã thoát exit code nổi bật màu xanh/đỏ, PID chẩn đoán, xem log PTY in-memory) ([HistoryView.vue](../src/views/history/HistoryView.vue)).
- [~] **SCR-05 Cài Đặt Hệ Thống & Quản Trị Dữ Liệu (System & Disaster Recovery):** *— Soát 26/09: xem các dòng con.*
  - [x] Điều hướng 3 tab: `Sao Lưu & Phục Hồi (Disaster Recovery)`, `Hệ Thống & Khởi Động (OS & Autostart)`, `Terminal & Hiệu Năng (PTY & Memory)`.
  - [x] Bảng sao lưu dữ liệu toàn diện với lệnh SQLite `VACUUM INTO` live-safe snapshot, bảng thống kê số lượng dữ liệu thực tế trong DB, các huy hiệu kiểm tra toàn vẹn `Zero Read Locks`, `WAL Safe Flush`. *— Soát 26/09: export và số liệu DB là thật; badge `Zero Read Locks` / `WAL Safe Flush` là chữ tĩnh.*
  - [~] Bảng phục hồi dữ liệu với **trình theo dõi trực quan quy trình Rollback 7 bước** an toàn (Dừng PTY → Đóng pool DB → Tạo checkpoint dự phòng → Kiểm tra toàn vẹn → Hoán đổi nguyên tử → Khởi động lại engine → Tự động hoàn nguyên nếu lỗi) ([SettingsView.vue](../src/views/settings/SettingsView.vue)). *— Soát 26/09: **tracker 7 bước là `<ol>` tĩnh**, thứ tự không khớp `import.rs`; badge `SHA-256 Validated` giả; Tauri đã khôi phục qua `backup_import_bytes`, không còn dùng `File.path`; chưa có `@tauri-apps/plugin-dialog`.*
- [x] **Thanh Tiêu Đề (Titlebar) & ActivityBar:** *— Soát 26/09: xem các dòng con.*
  - [x] Titlebar tích hợp biểu tượng CM gradient, nhãn phiên bản `v2.1`, viên con nhộng hiển thị số lượng daemon đang chạy kèm hiệu ứng pulse phát sáng, thanh tìm kiếm kích hoạt Command Palette `Ctrl+K`, nút điều khiển cửa sổ ([Titlebar.vue](../src/components/titlebar/Titlebar.vue)). *— Soát 26/09: logo là `/logo.svg`; `v2.1` hardcode.*
  - [x] ActivityBar với các nhãn định vị `Terminal`, `Lệnh`, `Template`, `Nhóm`, `Lịch sử`, `Cài đặt` với viền sáng màu primary active ([ActivityBar.vue](../src/components/nav/ActivityBar.vue)).
- [~] **Thanh Trạng Thái (StatusBar):** Hiển thị số lượng tiến trình đang chạy, dung lượng Ring Buffer, chế độ SQLite WAL và trạng thái Single-Instance Lock ([StatusBar.vue](../src/components/statusbar/StatusBar.vue)). *— Soát 26/09: chỉ số tiến trình là thật; dung lượng buffer giả (`running × 210KB / 2MB`), `SQLite: WAL`, `Single-Instance: Locked`, `Tự khởi động: Bật` đều hardcode.*

---


## 6. Tự Khởi Động & Khóa Đơn Bản Thể (Autostart & Single-Instance)

- [x] **Tích hợp `tauri-plugin-autostart`:** Hỗ trợ đăng ký khởi động cùng hệ điều hành cho cả Windows và Linux/macOS ([autostart.rs](../src-tauri/src/app/autostart.rs)).
- [x] **Single-Instance Lock trước khi tạo Process Manager:** Plugin `tauri-plugin-single-instance` được kích hoạt ngay trong `Builder::default()` trước hook `setup` ([lib.rs](../src-tauri/src/lib.rs)).
- [~] **Xử lý Instance thứ hai:** Khi mở bản thể thứ hai, [single_instance.rs](../src-tauri/src/app/single_instance.rs) tự động hiển thị cửa sổ hiện hành (`w.show()`), lấy tiêu điểm (`w.set_focus()`) và phát sự kiện `app://instance` rồi thoát instance mới. *— Soát 26/09: backend show/focus/emit đúng, nhưng frontend chưa lắng nghe `app://instance`.*
- [x] **Khởi chạy nhóm có cờ Autostart an toàn:** Sau khi giữ lock, backend duyệt các nhóm có `autostart = 1` và kiểm tra `if processes.group_has_live(&g.id) { continue; }` để không kích hoạt lại nhóm đã đang chạy ([autostart.rs](../src-tauri/src/app/autostart.rs)). *— Soát 26/09: một nhóm lỗi làm dừng các nhóm sau (`?`), kết quả bị bỏ qua ở `lib.rs`.*

---

## 7. Sao Lưu / Khôi Phục Dữ Liệu (Backup & Restore)

### Xuất Bản Sao Lưu (Export)
- [x] **`VACUUM INTO`:** Sử dụng câu lệnh chuẩn SQLite `VACUUM INTO` để tạo bản snapshot độc lập ngay cả khi DB đang mở ở chế độ WAL ([export.rs](../src-tauri/src/backup/export.rs)).
- [x] **Kiểm tra tính toàn vẹn:** Tự động chạy `PRAGMA integrity_check` trên file sao lưu và chỉ thông báo thành công khi kết quả trả về là `"ok"`.
- [x] **Bao gồm lịch sử lệnh:** Snapshot `VACUUM INTO` chứa cả `command_history`; `backup_info` báo số bản ghi lịch sử và restore staging giữ nguyên bảng này.

### Nhập Bản Sao Lưu (Import & Rollback)
- [x] **Kiểm tra sơ bộ tệp sao lưu:** Chạy `PRAGMA integrity_check`, migrate bản sao lưu cũ lên schema hiện tại trên file staging, rồi kiểm tra `read_schema_version(staged) == SCHEMA_VERSION` ([import.rs](../src-tauri/src/backup/import.rs)).
- [x] **Xác nhận đặc quyền từ người dùng:** Yêu cầu cờ `confirmed = true` từ IPC và hiển thị hộp thoại cảnh báo [RestoreWizard.vue](../src/components/dialogs/RestoreWizard.vue) (MOD-08). *— Soát 26/09: `client.ts` luôn gửi `confirmed: true`.*
- [x] **Tạo bản sao lưu dự phòng (Snapshot Rollback):** Checkpoint WAL trước, sau đó copy `app.db` hiện tại sang `app.db.rollback` trước khi thực hiện ghi đè.
- [x] **Dừng tiến trình & Đóng kết nối:** Gọi `shutdown_all` dừng toàn bộ lệnh đang chạy và gọi `db.close_all()` đóng toàn bộ pool kết nối SQLite.
- [x] **Dọn dẹp WAL Set:** Xóa sạch bộ ba `app.db`, `app.db-wal`, `app.db-shm` trước khi copy file mới vào vị trí; lỗi xoá không bị bỏ qua.
- [x] **Tự động Rollback khi lỗi:** Nếu thay file hoặc `db.reopen()` lỗi, tự động hoàn trả từ `app.db.rollback`, thử mở lại và trả riêng lỗi rollback/reopen nếu tiếp tục thất bại.

---

## 8. Bảo Mật & Ranh Giới An Toàn (Security Hardening)

- [~] **Kiểm soát thao tác đặc quyền:** Mọi thao tác thêm/sửa/xóa câu lệnh, thay đổi autostart nhóm và nhập dữ liệu sao lưu đều yêu cầu cờ `confirmed: bool` ở tầng backend Rust ([ipc/commands.rs](../src-tauri/src/ipc/commands.rs)). *— Soát 26/09: có ở command/template/preset/group/autostart nhóm/import; thiếu ở `settings_set`, `autostart_os_set`. Frontend hardcode `confirmed: true` nên cờ không bảo vệ thực sự.*
- [x] **Cảnh báo nguồn ngoài:** Hộp thoại khôi phục hiển thị rõ cảnh báo chỉ nạp tệp từ nguồn tin cậy (Trusted Zone).
- [ ] **Ranh giới Capability:** Không cho phép WebView gọi thẳng các lệnh spawn hệ thống; WebView chỉ có thể gửi yêu cầu thực thi Command ID đã được lưu hợp lệ trong SQLite. *— Soát 26/09: **mô tả không còn đúng** — `terminal_open` + `pty_write` cho WebView một shell tương tác đầy đủ (theo thiết kế); `commands_create` + `command_run` nhận `execution_string` tuỳ ý. Cần viết lại ranh giới cho đúng thực tế.*

---

## 9. Nhiệm Vụ Hoàn Thiện Tích Hợp (Wiring & Polish Tasks)

- [x] Đã cấu hình và kiểm tra build thành công frontend (`pnpm run build`). *— Soát 26/09: pass; cảnh báo chunk 630 kB > 500 kB.*
- [x] Unit tests Rust: `cargo test` pass 34/34; `cargo test --lib --features desktop` pass 37/37. `terminal_ipc_tests` cần `--features ipc-tests`.
- [x] Đã xác minh khả năng biên dịch đầy đủ tính năng desktop (`cargo check --features desktop`). *— Soát 26/09: pass, không cảnh báo.*
- [x] **Chuyển đổi IPC Client từ Mock sang Native Tauri IPC:** *— Soát 26/09: `USE_MOCK_IPC = false`; native dùng khi có Tauri runtime, map UUID ↔ ID số cho mọi thực thể. Browser mock đã có dữ liệu template/history tối thiểu để preview và smoke UI; các luồng backup native vẫn cần Tauri runtime.*
- [x] **Gắn kết phím tắt toàn cục:** Bổ sung hotkey `Ctrl+1` đến `Ctrl+6` và phím tắt điều hướng nhanh trong AppShell; `Ctrl+K` mở Command Palette.

---

## 10. Command Template & Tham Số (SCR-06)

> Mục tiêu: định nghĩa một lần mẫu lệnh có placeholder (vd. `ffmpeg -i {{input}} -crf {{crf}} {{output}}`), mỗi lần chạy chỉ cần điền params. Đây là bản tối giản của mục RJSF ở Post-MVP — form sinh từ danh sách param, không cần JSON Schema.

### Dữ liệu & Migration
- [x] **Bộ chạy migration tuần tự:** [pool.rs](../src-tauri/src/db/pool.rs) áp dụng lần lượt từng version còn thiếu (`002` đến `007`), dừng nếu thiếu migration và cập nhật `SCHEMA_VERSION = 7` ([schema.rs]). Mỗi migration thay đổi schema chạy trong transaction riêng.
- [x] **Migration template/history:** `002`–`004` tạo template và liên kết lịch sử; `005_command_history.sql` tạo bảng lịch sử lệnh; `006_shell_history.sql` mở rộng source cho shell/typed và thêm settings privacy/retention; `007_history_privacy_patterns.sql` thay mẫu mặc định quá rộng nhưng giữ nguyên cấu hình người dùng.
  - Bảng `command_template`: `id (TEXT)`, `name`, `template_string`, `is_shell (0/1)`, `description`.
  - Bảng `template_param`: `template_id` (FK `ON DELETE CASCADE`), `name`, `label`, `kind` (`string` / `number` / `enum` / `path` / `bool`), `default_value`, `required (0/1)`, `options` (JSON cho `enum`), `is_secret (0/1)`, `param_order`; PK `(template_id, name)`.
  - Bảng `template_preset`: bộ giá trị đã lưu theo tên để chạy lại nhanh (`id`, `template_id`, `name`, `values_json`, `last_used_at`) — **không lưu param `is_secret`**.
  - `run_session` thêm `template_id` (nullable) và cho phép `group_id` nullable (rebuild bảng, `ON DELETE SET NULL`) để vừa lưu lượt chạy template vừa lưu lệnh đơn.
- [x] **Backup/Restore:** [import.rs](../src-tauri/src/backup/import.rs) staging bản sao lưu, migrate từ v1 lên schema hiện tại trước khi thay file; vẫn từ chối file lỗi integrity hoặc schema mới hơn.

### Render an toàn (Backend Rust)
- [x] **Parse placeholder `{{name}}`:** Module [template/mod.rs](../src-tauri/src/template/mod.rs) trích placeholder, báo placeholder chưa khai báo và cảnh báo param chưa được dùng; có unit test.
- [x] **Render phía Rust, không phía WebView:** Frontend chỉ gửi `template_id` + map giá trị; backend validate required, number, bool, `enum` rồi mới render.
- [x] **Chống injection theo `is_shell`:** `argv` tokenize template trước rồi mới thay sentinel trong từng token; `shell` quote theo ngữ cảnh quote và không cho placeholder nằm trong single quote. Giá trị chứa `$(...)`, dấu cách, `&` hoặc placeholder lồng không được biến thành lệnh/tham số mới.
  - `argv`: parser theo nền tảng tokenize template **trước**, sau đó thay placeholder trong từng token → một giá trị luôn là một (phần) argv, không tách thêm token; trên Windows dấu `\` trong đường dẫn được giữ nguyên.
  - `shell`: mỗi giá trị được `shell_words::quote` trước khi chèn vào `sh -c`.
- [~] **Param `is_secret`:** không ghi vào `template_preset`, không ghi vào DB lịch sử, UI dùng input kiểu password và preview che secret. *— Giá trị secret vẫn cần đi qua PTY/biến môi trường khi tiến trình thực thi, nên không thể coi là tuyệt đối không lộ trong tiến trình con.*

### IPC
- [x] `templates_list` / `templates_create` / `templates_update` / `templates_delete` (yêu cầu `confirmed`), `template_presets_*`, `template_preview` (trả chuỗi đã render, che giá trị secret), `template_run` (tạo `run_session` + `run_event`, spawn qua [spawn.rs](../src-tauri/src/process/spawn.rs), trả `run_event_id` để mở tab PTY). Backend cập nhật `last_run_at`; preset chỉ lưu giá trị không secret.

### Giao diện
- [x] **SCR-06 Thư viện Template** ([TemplatesView.vue](../src/views/templates/TemplatesView.vue)), route `/templates`, mục ActivityBar `Template`, phím tắt `Ctrl+6`; danh sách lấy dữ liệu native và cột lần chạy gần nhất đọc `last_run_at`.
  - Danh sách template (tên, kiểu `argv`/`shell`, số param, lần chạy gần nhất) + tìm kiếm.
  - Nút Chạy / Sửa / Nhân bản / Xóa.
- [x] **Trình soạn template** ([TemplateEditorModal.vue](../src/components/dialogs/TemplateEditorModal.vue)): ô chuỗi lệnh highlight `{{…}}`; bảng cấu hình hỗ trợ label, kiểu, mặc định, bắt buộc, secret, options enum và thứ tự.
- [x] **Hộp thoại chạy template — MOD-10** ([TemplateRunModal.vue](../src/components/dialogs/TemplateRunModal.vue)): form sinh từ `template_param`, điền mặc định hoặc preset gần nhất; preview do `template_preview` Rust trả về, lỗi type/enum/required hiển thị theo field; chọn/lưu/xoá preset; Enter chỉ chạy từ ô nhập liệu và bỏ qua nút/select/IME; kết quả preview cũ bị bỏ qua khi có request mới; đường dẫn nhập trực tiếp, không còn nút điền dữ liệu giả.
- [x] **Command Palette:** chọn template (`Ctrl+K`) điều hướng tới `/templates?openTemplateId=...` và mở thẳng MOD-10.
- [x] **SCR-04:** `template_run` ghi `run_session`/`run_event`, History hiển thị tên template thay cho tên nhóm và lifecycle cập nhật khi tiến trình kết thúc.

---

## 11. Lịch Sử Lệnh & Gợi Ý (Autosuggestions / Autocomplete)

> Mục tiêu: trong tab terminal, gõ `git ch` → hiện phần còn lại mờ (ghost text) từ lệnh đã chạy, `→` để nhận; `Ctrl+Space` mở danh sách gợi ý. Chạy **giống nhau trên Windows và Linux**. Chi tiết kiến trúc: [plan.md §4.3](./plan.md).
>
> **Quyết định đã chốt (26/09/2026):**
> - **Hệ thống gợi ý riêng của app, không phụ thuộc shell:** dữ liệu chỉ lấy từ bảng `command_history`; không đọc lịch sử PSReadLine / `.bash_history` / `.zsh_history`; app tự vẽ ghost text, popup và xử lý phím. Shell chỉ cung cấp ranh giới prompt (OSC 633).
> - **Tắt gợi ý có sẵn của shell chỉ trong terminal của app:** script integration chỉ có hiệu lực trong process shell do app mở; không ghi vào `$PROFILE`, `.bashrc`, `.zshrc` hay cấu hình hệ thống.
> - Shell mặc định Windows: `pwsh` → `powershell.exe` → `cmd.exe`; Linux: `$SHELL` → `/bin/bash` → `/bin/sh`.
> - Lịch sử **dùng chung toàn app** (ưu tiên cùng `cwd`).
> - Windows dùng **ConPTY sideload** (`conpty.dll` + `OpenConsole.exe`) vì ConPTY của Windows 10 làm lệch thứ tự OSC so với text.
> - **Linux:** viết code và script ngay từ đầu, kiểm thử và sửa trên môi trường khác ở Giai đoạn 8.
> - Checkbox "kèm lịch sử" khi sao lưu: **post-MVP** (xem §13).

> *Cập nhật sau triển khai 26/09: migration `006`, OSC scanner/tracker, lịch sử cấp 1/cấp 2, privacy/retention, event `history://added`, marker xterm, ghost text, popup Ctrl+Space, cài đặt shell/ghost text và ConPTY sideload resource đã có. CI/QA shell và Linux runtime chưa hoàn tất.*
>
> *Spike A (26/09): PowerShell 7.6.5 + PSReadLine 2.4.5 qua ConPTY hệ thống phát đủ frame OSC 633, escape và nonce đúng ([kết quả](../scratchpad/osc633-spike/RESULT.md)). Nhưng đối chiếu với text: ConPTY của Windows 10 (19045) **đẩy OSC đi trước text đã render** → output của lệnh xuất hiện sau prompt kế tiếp; text giữa `A` và `B` rỗng.*
>
> *Spike B (26/09): sideload `conpty.dll` + `OpenConsole.exe` từ NuGet `Microsoft.Windows.Console.ConPTY` 1.24.260710001 (MIT, ~1,2 MB x64); portable-pty 0.8.1 tự nạp `conpty.dll` cạnh exe. 7/7 lần chạy: output luôn nằm giữa `C` và `D`, text giữa `A` và `B` đúng prompt, exit code (`0`/`1`/`3`) và nonce đúng; dòng wrap được OpenConsole in liền để xterm tự xuống dòng. PSReadLine 2.4 tự vẽ ghost text theo mặc định → script phải tắt trong phiên.*
>
> *Harness rerun 26/09: 28 frame, 27 chunk, nonce đúng và không mất frame; lệnh nhiều dòng phát các `E/C` trung gian trước `D`, nên `ShellTracker` giữ buffer cuối cùng rồi mới ghi lịch sử.*
>
> *Kiểm thử hồi quy 26/09: `cargo test --lib --features desktop` 37/37, `cargo test` 34/34, `cargo clippy --features desktop -- -D warnings` và `cargo fmt -- --check` đều đạt. Bổ sung smoke test PTY thật chứng minh shell cấp 1 ghi được lệnh, kiểm tra static script tắt autosuggestion của shell, test resource ConPTY sideload và test mở DB WAL với các reader chỉ-đọc; binary Tauri dev biên dịch/chạy được sau khi tách cấu hình reader khỏi `journal_mode=WAL`. Git Bash PTY smoke (POSIX-compatible, không thay thế Linux) xác nhận compound command/CWD/`HISTCONTROL=ignorespace`. QA thao tác native và runtime Linux vẫn chưa có bằng chứng trong môi trường hiện tại.*
>
> *Sửa hồi quy native 26/09: marker `B` của PowerShell/cmd/sh giờ nằm sau prompt hiển thị, PowerShell `D` đọc `$?` để không trả exit code cũ cho cmdlet, và `sh.sh` đã được kiểm tra lại bằng `sh -n`. Xterm chặn phím popup trước khi phát `onData`; level 2 chuyển sang `running` ngay khi Enter để không ghi các Enter trong chương trình tương tác. Test PTY Windows kiểm tra trực tiếp vùng `A…B` có prompt và PowerShell cmdlet lỗi phát `D;1`.*
>
> *Browser preview QA 26/09 (mock IPC): mở `Terminal mới`, nhận marker prompt, nhập `docker` thấy ghost text ` compose up -d postgres`, `ArrowRight` nhận phần còn lại rồi chạy thành công; command đã nhận được ghi thành `docker compose up -d postgres` trong HistoryView qua điều hướng SPA. `Ctrl+Space` hiển thị danh sách fuzzy, chọn `docker compose…` từ input `compose` đã xoá input cũ rồi thay bằng lệnh đầy đủ; autocomplete cũng nhận template `ffmpeg ... {{input}}`. Level 2 có input shadow làm fallback khi marker chưa kịp echo. Đây là bằng chứng frontend preview, không thay thế QA Tauri native.*

### Mức hỗ trợ theo shell

| Shell | Marker A/B | Ghi dòng lệnh | Tắt gợi ý có sẵn (chỉ trong app) |
|---|---|---|---|
| pwsh 7 (Windows/Linux) | hàm `prompt` | Cấp 1: `E` qua handler Enter của PSReadLine | `Set-PSReadLineOption -PredictionSource None` |
| Windows PowerShell 5.1 | hàm `prompt` | Cấp 1 qua handler Enter của PSReadLine; nếu PSReadLine không có thì cấp 2 | Không cần |
| cmd.exe | biến `PROMPT` chứa `$e]633;A…B` | Cấp 2 | Không cần |
| bash | `PS1` / `PROMPT_COMMAND` | Cấp 1: `PS0` hoặc trap `DEBUG` + `history 1` | `bleopt complete_auto_complete=` nếu có ble.sh |
| zsh | `precmd` / `preexec` | Cấp 1: `$1` của `preexec` | `ZSH_AUTOSUGGEST_STRATEGY=()` |
| sh / dash | `PS1` chứa byte ESC | Cấp 2 | Không cần |
| Không thấy marker (ssh, vim/htop) | — | Không ghi | Ẩn gợi ý |

- **Cấp 1:** shell phát `633;E;<command>;<nonce>` → chính xác, có nonce, bắt được cả lệnh sửa bằng mũi tên hoặc lấy từ lịch sử của shell.
- **Cấp 2 (dự phòng):** frontend đọc vùng nhập từ marker `B` khi bấm Enter → IPC `history_record_typed`, lưu `source = 'typed'`.

### Giai đoạn 0 — Spike còn lại trên Windows
- [~] **cmd.exe:** runtime đã đặt `PROMPT` có marker sau prompt hiển thị và có test PTY chọn đích danh `cmd` (ConPTY sideload); chưa có capture riêng để chạy `analyze.py`.
- [x] **PowerShell 5.1:** hàm `prompt` phát A/B/D/P, B nằm sau prompt, D dùng `$?` và handler Enter của PSReadLine phát `E`; nếu PSReadLine không khả dụng, backend tự hạ phiên xuống cấp 2. Test PTY chọn đích danh kiểm tra cả cmdlet lỗi → `D;1`; harness `analyze.py` mới chạy với pwsh 7, chưa có capture riêng cho PowerShell 5.1.
- [~] **Cấp 2:** frontend đã đọc vùng nhập từ marker `B` lúc Enter; cmd bổ sung `P;Cwd=$P` để lưu ngữ cảnh thư mục; đã ghi nhận và bỏ qua phần right prompt có sẵn tại thời điểm `B` (oh-my-posh/starship); chưa có QA đầy đủ cho wrap/IME/sửa giữa dòng.
- [~] **Đạt một phần:** pwsh, PowerShell 5.1 và cmd đã được xếp đúng cấp qua test PTY; fixture Spike A đã chạy qua `analyze.py` local và được đưa vào CI, nhưng còn thiếu capture riêng cho từng shell và QA thủ công.

### Giai đoạn 1 — Tầng PTY và shell
- [x] **ConPTY sideload (Windows):** `src-tauri/resources/conpty/win-x64` chứa cặp binary đã ghim; `tauri.conf.json` bật bundle resources, `build.rs` copy dev/release profile, `PtySession` đặt DLL directory trước `native_pty_system()` và cảnh báo khi thiếu resource; test Windows xác nhận resource được tìm thấy; `pnpm run tauri:build` đã tạo thành công cả MSI và NSIS.
- [x] **Chọn shell** ([shell_integration/mod.rs](../src-tauri/src/pty/shell_integration/)): enum `ShellKind` và thứ tự Windows `pwsh → powershell.exe → cmd`; Linux `$SHELL → /bin/bash → /bin/sh` (nhận cả tên ngắn như `zsh` và đường dẫn), trả metadata `shell_kind/history_level`.
- [~] **Script integration:** đã có `powershell.ps1`, cmd `PROMPT`, `bash.sh`, `zsh.zsh`, `sh.sh`; marker B được đặt sau prompt ở các shell có level 2, PowerShell dùng `$?` cho exit code và `sh.sh` đã qua `sh -n`; bash ưu tiên `history 1` để giữ cả compound command, script được ghi vào `app_data/shell-integration/1`, tự xoá theo vòng đời shell, và PowerShell tự hạ cấp khi thiếu PSReadLine; còn thiếu QA từng shell.
- [x] **`spawn_interactive_on_slave`** ([session.rs](../src-tauri/src/pty/session.rs)): sinh nonce riêng cho PowerShell, trả `shell_kind/history_level` về frontend.
- [~] **Độ trễ mở terminal:** `terminal_open` vẫn là command đồng bộ và còn dò `where.exe`/PSReadLine mỗi lần mở; cần cache/async ở lượt hoàn thiện sau.
- [~] **Lưu lệnh từ HistoryView:** lệnh pwsh đang lưu thành command shell chung, khi chạy lại trên Windows có thể đi qua `cmd /C`; cần lưu và khôi phục `shell_kind` riêng.
- [~] **Đóng gói ConPTY theo nền tảng:** resource sideload hiện còn khai báo trong `tauri.conf.json` chung; cần chuyển vào config Windows để không đưa binary Windows vào bundle Linux.
- [~] **Kiểm thử hồi quy:** test PTY Rust cho terminal, pwsh, PowerShell 5.1, cmd, resource ConPTY và smoke test `ShellTracker` ghi lệnh cấp 1 đã chạy qua với ConPTY sideload; Git Bash PTY smoke đã kiểm tra compound command/CWD/`HISTCONTROL=ignorespace`; chưa chạy lại nhóm/lệnh thật và QA thủ công.

### Giai đoạn 2 — `OscScanner` và ghi lịch sử
- [x] **Migration `005_command_history.sql`:** bảng `command_history` (`id`, `command_line`, `shell_kind`, `cwd`, `last_exit_code`, `run_count`, `first_used_at`, `last_used_at`, `source`), `UNIQUE(command_line, shell_kind)` → upsert tăng `run_count`.
- [x] **Command / Template:** lượt chạy command/group và `template_run` ghi command line vào `command_history` (`source = command | template`); template lưu chuỗi đã che `is_secret`.
- [x] **`OscScanner`** ([osc.rs](../src-tauri/src/pty/osc.rs)): state machine theo byte, BEL/ST, chunk split, giới hạn 64 KB, unescape và giữ raw bytes trong Ring Buffer.
- [x] **Gắn vào `pump_reader`** qua `Option<ShellTracker>` chỉ cho terminal tương tác; command/group không đổi.
- [x] **Cấp 1:** đủ `E` nonce đúng → `C` → `D`, lưu shell/cwd/exit code; sai nonce bị bỏ qua.
- [x] **Cấp 2:** IPC `history_record_typed`; backend chỉ nhận từ terminal level 2.
- [x] **Migration `006`:** mở rộng source và thêm settings history mặc định.
- [x] **Migration `007`:** thu hẹp mẫu privacy mặc định; không chặn nhầm PowerShell `-Path`/`-Property` hoặc `ssh -p2222`, nhưng vẫn giữ nguyên mẫu tùy chỉnh của người dùng.
- [x] **Quyền riêng tư:** bỏ qua leading-space, mẫu nhạy cảm, history disabled và trim theo max entries; migration 007 không còn chặn nhầm PowerShell `-Path`/`-Property` hoặc `ssh -p2222`.
- [x] **IPC:** `history_record_typed`, event `history://added` và listener frontend đã có.
- [x] **Unit test:** split mọi vị trí, BEL/ST, sai nonce, frame quá lớn, Unicode/wrong-order và privacy/retention đều có; test tích hợp từng shell nằm ở Giai đoạn 7/8.

### Giai đoạn 3 — Theo dõi vùng nhập ở frontend
- [x] **XtermPane:** `terminal.parser.registerOscHandler(633)` tạo marker B và trạng thái `prompt → input → executing`.
- [~] **`readInput()`:** đọc từ marker `B` tới con trỏ, nối dòng và dùng `translateToString`; bỏ qua phần right prompt đã tồn tại tại thời điểm `B` (oh-my-posh/starship) khi đọc input và kiểm tra cursor cuối dòng. Khi ghi history cấp 2, đọc hết dòng hiện tại (vẫn cắt trước right prompt) để không mất phần sau con trỏ; đã chặn suggestion/history trong IME composition, chưa QA đủ ký tự rộng/IME thực tế.
- [~] **Chỉ hiện gợi ý khi:** đã chặn executing/alternate screen/bracketed paste, yêu cầu marker và kiểm tra cursor không có nội dung phía sau; căn chính xác theo wrap/ký tự rộng còn cần QA xterm.
- [x] **Cấp 2:** bắt Enter → chụp vùng nhập trước khi shell thay marker prompt; dùng input shadow khi marker chưa kịp echo và fallback đọc trễ → `history_record_typed`.

### Giai đoạn 4 — Ghost text (Autosuggestions)
- [~] **SuggestionEngine:** [useSuggestions.ts](../src/composables/useSuggestions.ts) cache lịch sử + command/template definitions trong RAM, refresh theo `history://added`, xếp hạng shell/cwd/exit/run count; không gọi IPC mỗi phím. Đã sửa right prompt oh-my-posh/starship, nhưng chưa QA với profile native thực tế.
- [~] **Vẽ ghost text:** overlay theo theme đã tính vị trí theo cell/cursor và viewport của xterm; chiều rộng bị giới hạn tới right prompt để không vẽ đè oh-my-posh/starship; chưa QA đầy đủ khi wrap, scroll và ký tự rộng.
- [~] **Nhận gợi ý & lịch sử:** `→`/`End` nhận cả gợi ý, `Ctrl+→` nhận một từ; `↑`/`↓` khi đang gõ dở chỉ đổi ghost text (dòng đã gõ giữ nguyên): gõ `rc` hiện lệnh mới nhất bắt đầu bằng `rc`, mỗi `↑` chuyển sang lệnh cũ hơn và dừng ở lệnh cũ nhất, `↓` quay về lệnh mới hơn, `→` nhận gợi ý đang hiện; dòng trống thì `↑`/`↓` gọi lại trực tiếp theo thứ tự `last_used_at`, dùng độ dài lệnh app vừa gửi để chống key-repeat, bỏ qua focus report, cho phép các dòng xterm `isWrapped` nhưng bỏ qua continuation nhiều dòng thật; `↓` khôi phục dòng đang gõ; ghost text theo kiểu zsh-autosuggestions: luôn là lệnh `↑` sẽ gọi ra đầu tiên (mới nhất cùng shell), ẩn khi đang duyệt `↑/↓`, cập nhật theo output shell đã vẽ thay vì hẹn giờ theo phím; kịch bản browser mock đã đạt; chưa có QA native giữ phím dài và trường hợp lịch sử vừa ghi chưa kịp tải lại.
- [~] **Trường hợp biên:** đã tránh escape/paste đơn giản và chặn IME composition; chưa QA đủ wrap, ký tự rộng và IME thực tế.

### Giai đoạn 5 — Popup `Ctrl+Space` (Autocomplete)
- [x] **Popup fuzzy:** popup inline có Ctrl+Space, gộp lịch sử/command/template và chọn bằng mũi tên/Enter; `useSuggestions` có fuzzy scorer (prefix/substring/subsequence), lựa chọn khác prefix thay thế input cũ bằng Backspace + lệnh đầy đủ, Escape đóng popup, Tab vẫn dành cho shell.
- [x] **Không chiếm `Tab`:** Tab vẫn được gửi cho completion của shell.
- [x] **Dùng lại engine** cho ô `execution_string` trong [CommandEditorModal.vue](../src/components/dialogs/CommandEditorModal.vue) và Command Palette (`Ctrl+K`); lịch sử được xếp hạng cùng một nguồn với ghost text.

### Giai đoạn 6 — Cài đặt và quản lý
- [x] **SCR-05 — tab Terminal:** đã có bật/tắt ghi lịch sử, `history_max_entries`, danh sách mẫu chặn, chọn shell mặc định, bật/tắt ghost text và xoá lịch sử ở HistoryView.
- [x] **SCR-05 — Sao lưu:** `VACUUM INTO` và restore staging tự động bao gồm `command_history`; `backup_info.history_count` đọc số bản ghi. Tuỳ chọn loại trừ lịch sử khi export: post-MVP.
- [x] **SCR-04 — Lịch sử lệnh:** HistoryView có tìm kiếm, lọc source (`typed/shell/command/template`), xoá từng dòng, xoá toàn bộ và lưu dòng lịch sử thành Command hoặc Template.

### Giai đoạn 7 — Kiểm thử Windows và CI
- [~] Test tích hợp PTY: `pwsh`, PowerShell 5.1, `cmd`, Git Bash compatibility smoke và tracker cấp 1 đều có test/runtime kiểm tra trên Windows; test mới kiểm tra vùng A-B không rỗng, PowerShell cmdlet lỗi → D;1 và script `sh` hợp lệ; migration 007 kiểm tra không chặn `-Path`/`-Property`/`ssh -p2222`; chưa có capture `analyze.py` cho PowerShell 5.1 hoặc profile oh-my-posh; browser preview đã kiểm tra input/ghost/popup/template/history; QA native thủ công còn chờ.
- [x] Job `windows-latest` trong CI: workflow `gate11.yml` chạy test/clippy/frontend build, parse `powershell.ps1` và tạo Tauri MSI/NSIS để kiểm tra resource ConPTY trong bundle.
- [ ] QA thủ công: resize, vim/htop, IME tiếng Việt, paste nhiều dòng, dòng rất dài, cwd Unicode, đóng/mở lại tab, reattach.

### Giai đoạn 8 — Kiểm thử và sửa trên Linux (môi trường khác, làm sau)
- [~] Test runtime `bash`/`zsh`/`sh` đã được thêm vào bộ Rust test Linux; chạy harness spike B + `analyze.py` riêng cho bash, zsh (có và không có zsh-autosuggestions), sh/dash, pwsh vẫn chờ môi trường Linux thực tế.
- [ ] **Đạt khi:** output nằm giữa `C`/`D`; text giữa `A`/`B` đúng prompt; exit code, nonce, dòng wrap đúng; `.bashrc`/`.zshrc` vẫn được nạp; xử lý đúng `HISTCONTROL=ignorespace`; zsh-autosuggestions chỉ bị tắt trong terminal của app.
- [~] Test tích hợp PTY trên Linux: đã thêm smoke test cho `bash`/`zsh`/`sh`, kiểm tra cú pháp ba script và job `ubuntu-latest` cài `zsh`, `dash`; chưa có kết quả chạy CI trong môi trường hiện tại.
- [ ] **Không đạt:** hạ shell đó xuống cấp 2, hoặc chỉ dùng popup (không ghost text).

### Thứ tự và mốc

| # | Giai đoạn | Phụ thuộc | Kết quả dùng được |
|---|---|---|---|
| 0 | Spike cmd / PowerShell 5.1 | — | Chốt cấp hỗ trợ từng shell Windows |
| 1 | PTY và shell | 0 | Terminal mở đúng shell kèm integration |
| 2 | Scanner và lịch sử | 1 | **Lệnh gõ tay được ghi vào lịch sử** |
| 3–4 | Vùng nhập + ghost text | 2 | **Autosuggestion** |
| 5 | Popup | 4 | **Autocomplete** |
| 6 | Cài đặt và quản lý | 2 | Quản lý lịch sử |
| 7 | Kiểm thử Windows + CI | 1 trở đi (song song) | Ổn định Windows |
| 8 | Kiểm thử và sửa Linux | 1–5 | Ổn định Linux |

**Rủi ro đã biết:** sideload ConPTY đổi mọi PTY của app (phải thử lại lệnh/nhóm thường); phải tự cập nhật `conpty.dll` khi Microsoft vá; lịch sử cấp 2 có thể ghi sai nếu shell tự vẽ lại dòng lệnh; thiết kế Linux dựa theo shell integration của VS Code nhưng chưa chạy thử.

---

## 12. Lỗi Phát Hiện Khi Soát Code (26/09/2026)

> Sắp theo mức độ ưu tiên. Mỗi dòng là một việc cần sửa; tick khi đã sửa.

### Bảo mật / đúng đắn khi chạy lệnh
- [x] **Shell injection trên Unix qua placeholder có dấu nháy:** renderer mới quote theo ngữ cảnh; `echo "{{msg}}"` với `$(id)` chỉ in dữ liệu.
- [x] **Argv injection qua placeholder có dấu nháy:** tokenize template trước, thay sentinel sau; giá trị có quote hoặc dấu cách không sinh thêm token và backslash cuối đường dẫn được giữ nguyên.
- [x] **Thay thế lồng nhau:** renderer chỉ quét template gốc một lần; giá trị chứa `{{param_sau}}` được giữ nguyên.
- [x] **Cú pháp placeholder lệch nhau:** parser Rust và frontend đều chấp nhận khoảng trắng quanh tên, renderer dùng tên chuẩn hoá.
- [x] **Windows shell làm sai giá trị:** renderer không chèn caret vào dữ liệu, nên `a&b` giữ nguyên.
- [x] **Windows direct argv làm mất dấu `\` trong đường dẫn:** dùng parser argv theo quy tắc Windows cho cả template và command thường; lệnh `G:\Work\llama.cpp\build\bin\llama-server.exe` được truyền tới `CreateProcessW` đúng dạng executable + arguments.
- [x] **Preview không trung thực:** preview native gọi `template_preview`; trạng thái chỉ ghi `Rust Safe-Quoted` sau khi IPC thành công, fallback được gắn nhãn rõ và Tauri không cho chạy khi Rust preview chưa xác thực.

### Tiến trình & lịch sử
- [ ] **Nhóm chạy song song, không tuần tự** (`start_group_inner`) — cần quyết định: chạy tuần tự thật (chờ exit) hay đổi mô tả/UI thành "khởi động theo thứ tự".
- [~] **Spawn lỗi giữa nhóm** đã đánh dấu event/session thất bại thay vì kẹt `running`; nhóm vẫn khởi động song song theo behavior hiện tại, chưa đổi thành tuần tự.
- [x] **Race insert/remove** trong `ProcessManager`: `reserve → insert/cancel` được khóa nguyên tử; lệnh thoát ngay không để lại `Live` treo và `remove` ID lạ không tạo tombstone.
- [x] **Chạy lẻ có lịch sử:** `command_run` và `template_run` ghi `run_session` / `run_event`; terminal trống vẫn loại khỏi audit theo thiết kế.
- [x] **Xoá nhóm đã có lịch sử:** `run_session.group_id` dùng `ON DELETE SET NULL`.
- [x] **`replace_memberships` và migration:** migration 002/003/004/005 và thao tác thay membership đều chạy trong transaction.

### Sao lưu / khôi phục
- [x] **Khôi phục trong Tauri:** frontend đọc bytes của `File` và gửi IPC `backup_import_bytes`; backend staging file, không dùng `File.path`.
- [x] **Bản rollback có thể thiếu dữ liệu:** checkpoint WAL trước khi snapshot rollback.
- [x] **Nhận bản sao lưu v1:** staging file và migrate lên schema hiện tại trước khi thay thế.

### UI hardcode / giả lập
- [ ] Thay giá trị giả bằng dữ liệu thật hoặc bỏ đi: `sess-91a`, RAM ước tính, fallback `7`, badge `v2.1`, `SQLite WAL: Synced`, `Zero Read Locks`, `WAL Safe Flush`, `SHA-256 Validated`, thẻ `DỪNG KHI LỖI`, cột `CWD`, StatusBar (buffer, WAL, lock, autostart).
- [ ] Nút **Khởi động lại** (DockHost), **Chạy Tất Cả Nhóm**, **Chạy Nhóm** trên ribbon (luôn nhóm đầu).
- [~] Phím tắt `Ctrl+F` (GroupTree); `Ctrl+6` và `Enter` trong MOD-10 đã có.
- [ ] Số thứ tự `0{{idx+1}}.` → hiện `010.` từ mục thứ 10 (GroupTree, GroupsView).
- [x] Sửa đường đóng app: đóng cửa sổ ẩn xuống tray; menu tray “Thoát hoàn toàn” dùng backend shutdown nên không cần capability `core:window:allow-destroy`. *— 27/09: build/test xác nhận; QA native còn ghi tại §3.*
  - Kiểm tra 27/09: frontend build và clippy desktop đạt; 4/4 test shutdown đạt. Toàn bộ `cargo test --lib --features desktop`: 44/45, test `powershell_without_profile_uses_the_default_prompt` nhận prompt `C:\Users\HOA>` thay vì `PS …>` trong môi trường chạy test; chưa xác nhận QA đóng cửa sổ native.
- [ ] Frontend chưa lắng nghe `app://instance`.

### Kiểm thử
- [x] Logic render đã tách ra module `template/` không phụ thuộc Tauri; test template chạy trong `cargo test` mặc định.

---

## 13. Kế Hoạch Sau MVP (Post-MVP Scope)

- [ ] **Systemd Services Exporter:** Hỗ trợ xuất nhóm lệnh thành service unit file chạy trên Linux nền server.
- [ ] **Biểu mẫu động JSON Schema (RJSF):** Hỗ trợ sinh giao diện tham số dòng lệnh động cho các công cụ như `ffmpeg`, `cloudflared` (mở rộng từ Command Template ở mục 10: param phụ thuộc lẫn nhau, validate phức tạp).
- [ ] **Lưới đa cửa sổ (Dockview Grid Layout):** Cho phép kéo thả chia ô chia cột lưới đa nhiệm phức tạp.
- [ ] **Đồng bộ đám mây (Google Drive Sync):** Lưu trữ token xác thực trong OS Keyring / Credential Manager.
- [ ] **Sao lưu tuỳ chọn kèm lịch sử lệnh:** checkbox "kèm lịch sử" khi export (mặc định bật) và cảnh báo trong RestoreWizard khi bản sao lưu không kèm lịch sử (lịch sử hiện tại sẽ bị thay bằng rỗng).
