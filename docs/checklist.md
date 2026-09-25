# Checklist triển khai MVP

Nguồn: [plan.md](./plan.md) | Thiết kế màn hình: [screens.md](./screens.md). Mục Post-MVP không thuộc Definition of Done.

**Lưu ý trong plan:** bảng kiến trúc ghi frontend Vue + TypeScript + Vite, nhưng PTY/layout nêu xterm.js + dockview-react. Cần chốt stack trước khi scaffold.

## 0. Chốt trước khi code

- [ ] Chốt frontend: Vue hay React (và dockview tương ứng: Vue vs `dockview-react`)
- [ ] Chốt stack PTY: plugin Tauri PTY nào, Linux trước hay đa nền
- [ ] Chốt chính sách backpressure: pause đọc PTY / drop đầu ra cũ / chunk+debounce IPC
- [ ] Chốt kích thước ring buffer (byte, ví dụ 1–2MB) và timeout graceful shutdown (5–10s)
- [ ] Làm rõ với user: lệnh trong DB = trusted, không sandbox

## 1. Scaffold và nền tảng

- [ ] App Tauri v2: Rust (Tokio) + frontend TS + Vite
- [ ] IPC + Capabilities tối thiểu (không mở quyền thừa)
- [ ] CSP `default-src 'self'`
- [ ] SQLite WAL, 1 writer + 2–3 reader
- [ ] Schema version (để restore kiểm tra được)

## 2. Dữ liệu tĩnh (SQLite)

- [ ] `command_definition`: `id`, `name`, `execution_string`, `is_shell`
- [ ] `command_group`: `id`, `group_name`, `autostart`
- [ ] `group_membership`: `group_id`, `command_id`, `execution_order`
- [ ] `run_session`: `id`, `group_id`, `started_at`, `status`
- [ ] `run_event`: `id`, `session_id`, `command_id`, `started_at`, `ended_at`, `status`, `exit_code`, `pid`
- [ ] CRUD lệnh / nhóm / thứ tự chạy
- [ ] `is_shell` rõ: argv vs shell tường minh
- [ ] DB chỉ ghi lúc lifecycle: start / end + exit code (không ghi log PTY)

## 3. Process Manager (trạng thái động)

- [ ] PID, PTY, ring buffer giữ in-memory (DashMap/RwLock)
- [ ] Chạy nhóm theo `execution_order`
- [ ] Start/stop theo lệnh và theo nhóm
- [ ] `pid` trên `run_event` chỉ để chẩn đoán, không dùng reattach sau restart app
- [ ] Hợp đồng: mọi process app-bound — thoát app thì dừng hết
- [ ] Đóng tab UI: ẩn terminal, process vẫn chạy
- [ ] Stop chỉ khi nút Stop hoặc thoát app
- [ ] Shutdown: SIGTERM / CTRL_C_EVENT → chờ timeout → SIGKILL / TerminateProcess
- [ ] Windows: adapter process group riêng
- [ ] Linux: `PR_SET_PDEATHSIG` trước `exec` (giảm orphan khi crash)

## 4. Terminal PTY

- [ ] PTY thật (TTY + ANSI)
- [ ] Byte thô (kèm ANSI) → xterm.js
- [ ] Backpressure đúng chính sách đã chốt
- [ ] Resize: cols/rows UI đồng bộ kernel PTY khi đổi tab/cửa sổ
- [ ] Reattach: đóng tab UI nhưng process chạy → mở lại xả recent output từ buffer
- [ ] Không kỳ vọng khôi phục màn hình fullscreen (vim/htop)
- [ ] Không ghi luồng PTY ra đĩa (mật khẩu gõ vào PTY)

## 5. UI MVP

- [ ] Tabs terminal (dockview); grid phức tạp = sau
- [ ] Play nhóm → `run_session` + `addPanel` mỗi lệnh
- [ ] Trạng thái start/stop trên UI
- [ ] Xem log phiên (từ ring buffer, không từ DB)
- [ ] Quản lý lệnh/nhóm + cờ autostart

## 6. Autostart và single-instance

- [ ] `tauri-plugin-autostart`: tự mở app theo OS
- [ ] Single-instance lock trước khi khởi tạo process manager
- [ ] Instance thứ hai: focus instance cũ rồi thoát
- [ ] Sau khi giữ lock: chỉ autostart nhóm có cờ, không chạy lại nhóm đã active trong instance này

## 7. Sao lưu / khôi phục (thủ công, local)

### Export

- [ ] `VACUUM INTO`
- [ ] Chỉ báo thành công sau `PRAGMA integrity_check` pass

### Import

- [ ] Copy backup vào vùng tạm
- [ ] `integrity_check` + kiểm tra schema version
- [ ] Cảnh báo + xác nhận user trước khi đụng DB
- [ ] Backup `app.db` hiện tại
- [ ] Stop mọi `run_session`, đóng hết connection
- [ ] Xóa `app.db` + `app.db-wal` + `app.db-shm`, copy file mới
- [ ] Mở lại DB + reload UI; fail → rollback file bước backup

## 8. Bảo mật MVP

- [ ] Tạo/sửa lệnh, sửa autostart, import backup = tác vụ đặc quyền (xác nhận)
- [ ] Import từ nguồn ngoài: cảnh báo rõ
- [ ] Trusted zone: mọi thứ trong DB lệnh được coi là an toàn để chạy
- [ ] Capabilities hẹp; XSS WebView không được spawn tùy ý hơn mức đã chấp nhận

## 9. Definition of Done (MVP)

- [ ] Lưu lệnh/nhóm, chạy argv hoặc shell, terminal PTY theo tab
- [ ] Theo dõi start/stop + log trong phiên
- [ ] Autostart app + nhóm + single-instance
- [ ] Backup/restore khép kín, integrity, rollback
- [ ] Đóng tab ≠ kill process; thoát app = dừng hết (graceful rồi force)
- [ ] Không ghi PTY ra đĩa; SQLite không nuốt log realtime

## 10. Post-MVP (cố ý để sau)

- [ ] Đẩy systemd services
- [ ] RJSF + Rust adapter (ffmpeg/cloudflared…) + Preview Command
- [ ] Layout lưới dockview
- [ ] Google Drive sync + token trong OS keyring
