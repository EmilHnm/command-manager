# Tài liệu Thiết kế Giao diện và Trải nghiệm Người dùng (UI/UX Screen Design Specification)

> **Dự án:** Ứng dụng Desktop Quản lý, Thực thi Lệnh và Terminal Đa nhiệm (Command Manager Desktop)  
> **Tài liệu tham chiếu:** [plan.md](./plan.md) | [checklist.md](./checklist.md)  
> **Nền tảng mục tiêu:** Tauri v2 (Rust Backend + Vite/TypeScript Frontend + SQLite WAL + PTY xterm.js + Dockview)

---

## 1. Nguyên tắc Thiết kế & Kiến trúc Giao diện Tổng thể

### 1.1. Triết lý Thiết kế (Design Principles)
1. **Developer-First & Information-Dense:** Giao diện tối ưu hóa cho lập trình viên, sysadmin và DevOps; mật độ thông tin cao nhưng gọn gàng, giảm thiểu các khoảng trống vô nghĩa, ưu tiên hiển thị trạng thái lệnh và luồng terminal.
2. **Trạng thái Trực quan Thời gian thực (Real-time Observability):** Mỗi lệnh và nhóm lệnh phải phản ánh tức thì trạng thái thực tế: *Idle / Ready*, *Running (kèm PID tạm & thời gian chạy)*, *Success (Exit code 0)*, *Failed (Exit code != 0)*, *Terminating*.
3. **Phân định Rõ ràng: Đóng Tab ≠ Hủy Tiến trình:** Thiết kế UI phải thể hiện rõ hợp đồng vòng đời: Đóng tab terminal chỉ ẩn giao diện hiển thị, tiến trình vẫn chạy ngầm trong backend. Nút "Stop" đỏ và hành động thoát ứng dụng mới kích hoạt quy trình Graceful Shutdown.
4. **Dark Mode Chủ đạo (Terminal Aesthetic):** Bảng màu tối chuẩn hiện đại (Slate/Zinc pha tím than), độ tương phản cao đạt chuẩn WCAG AA, phông chữ đơn cách (Monospace) chất lượng cao (JetBrains Mono / Fira Code) cho mọi chuỗi lệnh và màn hình terminal.
5. **Bảo vệ Vùng Tin cậy (Trusted Zone & Safe Fallback):** Các tác vụ đặc quyền (sửa chuỗi lệnh, nhập bản sao lưu đè DB, kích hoạt autostart) luôn có cờ cảnh báo rõ ràng và bước xác nhận an toàn.

---

### 1.2. Bố cục Khung Ứng dụng (App Shell Architecture)

Cấu trúc cửa sổ tuân thủ mô hình chuẩn của một Desktop IDE/Workstation hiện đại:

```
+-----------------------------------------------------------------------------------------------+
| [Icon] Command Manager v1.1.0        [Search Commands / Groups... Ctrl+K]      [_] [□] [X]    | <- Custom Titlebar
+-----------------------------------------------------------------------------------------------+
| NAV   | SUB-SIDEBAR       | MAIN WORKSPACE (Dockview Tabs & Content)                         |
| BAR   | (Groups / Tree)   |                                                                  |
| (64px)| (260px)           | +--------------------------------------------------------------+ |
|       |                   | | [Tab 1: API Server (PID: 1042)] [x] | [Tab 2: Web Dev] [x] | + | | <- Tab Bar
| [DSH] | > Active Sessions | +--------------------------------------------------------------+ |
| [CMD] |   ● Web Stack (2) | $ npm run dev -- --host                                        | |
| [GRP] |     ├─ vite (run) | VITE v5.4.2  ready in 280 ms                                   | |
| [LOG] |     └─ api  (run) | ➜  Local:   http://localhost:5173/                             | |
|       |                   | ➜  Network: http://192.168.1.15:5173/                          | |
|       | > Quick Launch    |                                                                | |
|       |   [▷ Run All]     |                                                                | |
|       |   [⏹ Stop All]    |                                                                | |
| ----- |                   |                                                                | |
| [SET] | [Autostart: ON]   |                                                                | |
+-----------------------------------------------------------------------------------------------+
| [STATUS BAR] ● 2 running | Ring Buffer: 420KB/2MB | SQLite: WAL | Single-Instance: Locked     |
+-----------------------------------------------------------------------------------------------+
```

* **Custom Titlebar (40px):** Tích hợp vùng kéo thả cửa sổ (`data-tauri-drag-region`), logo, thanh tìm kiếm nhanh toàn cục (Command Palette / Quick Jump `Ctrl+K`), nút thu nhỏ/phóng to/đóng với logic can thiệp sự kiện Close để thực hiện Graceful Shutdown.
* **Activity Bar / Nav Rail (60px):** Cố định bên trái với các biểu tượng điều hướng màn hình chính:
  * 🖥️ **Dashboard & Workspace (`/workspace`):** Không gian quản lý phiên chạy và Tabs Terminal.
  * ⚡ **Command Library (`/commands`):** Thư viện lệnh, cấu hình argv/shell.
  * 🧩 **Command Templates (`/templates`):** Thư viện mẫu lệnh có tham số (`{{param}}`), tạo preset và xem trước lệnh.
  * 🌐 **SSH Servers (`/servers`):** Danh sách máy chủ SSH, mở terminal từ xa và trình quản lý file trên server (SCR-07, SCR-08).
  * 📁 **Groups & Workflows (`/groups`):** Quản lý nhóm lệnh và thứ tự thực thi (`execution_order`).
  * 📜 **Session History & Logs (`/history`):** Lịch sử các `run_session` và sự kiện `run_event`.
  * ⚙️ **Settings & Backup (`/settings`):** Cấu hình Autostart, dung lượng Buffer, Sao lưu & Khôi phục SQLite.
* **Side Panel (Collapsible 260px):** Hiển thị danh sách ngữ cảnh tùy thuộc vào màn hình được chọn (ví dụ: Cây nhóm lệnh và trạng thái phiên chạy ở Workspace).
* **Main Content Area (Fluid):** Khu vực hiển thị linh hoạt với Dockview (hỗ trợ tab đa nhiệm và tương lai chia lưới Split View).
* **Global Status Bar (28px):** Thanh trạng thái kỹ thuật chân trang: số lượng tiến trình đang chạy, dung lượng ring buffer đang chiếm dụng, trạng thái kết nối SQLite WAL, cờ Autostart.

---

## 2. Bản đồ Màn hình & Cấu trúc Điều hướng (Screen Sitemap)

```mermaid
graph TD
    App[Command Manager Desktop App] --> Titlebar[Global Titlebar & Command Palette Ctrl+K]
    App --> MainShell[Main Shell Navigation]

    MainShell --> SCR01[SCR-01: Terminal Workspace & Execution Dashboard]
    MainShell --> SCR02[SCR-02: Command Library - Danh mục Lệnh]
    MainShell --> SCR06[SCR-06: Command Templates - Thư viện Mẫu Lệnh]
    MainShell --> SCR07[SCR-07: SSH Servers - Quản lý Máy chủ SSH]
    MainShell --> SCR03[SCR-03: Command Groups & Workflow Sequencer]
    MainShell --> SCR04[SCR-04: Session History & Run Event Logs]
    MainShell --> SCR05[SCR-05: System Settings, Backup & Restore]

    SCR01 --> MOD01[MOD-01: Quick Process Stop / Terminate Modal]
    SCR01 --> DOCK01[Dockview Tab Controller & Reattach Buffer View]
    DOCK01 --> MOD14[MOD-14: Rename Manual Terminal Modal - Hộp thoại Đổi tên Terminal Thủ công]
    DOCK01 --> SCR08[SCR-08: Files Pane & Kéo Thả Truyền File]
    SCR08 --> MOD20[MOD-20: Transfer Conflict - Xung đột Tệp]
    SCR08 --> XFER[Ngăn Transfers - Hàng đợi Truyền File]

    SCR07 --> MOD17[MOD-17: SSH Host Editor - Thêm/Sửa Máy chủ]
    SCR07 --> DOCK01

    SCR02 --> MOD02[MOD-02: Command Editor Modal - Create/Edit is_shell]
    SCR02 --> MOD03[MOD-03: Delete Command Confirmation Modal]

    SCR06 --> MOD10[MOD-10: Template Run & Preset Selector Modal]
    SCR06 --> MOD11[MOD-11: Template Editor & Parameter Spec Modal]

    SCR03 --> MOD04[MOD-04: Group Composer & Execution Order Sequencer]
    SCR03 --> MOD05[MOD-05: Autostart Toggle Warning Modal]

    SCR04 --> MOD06[MOD-06: Run Event Diagnostic Drawer - PID & Exit Code]

    SCR05 --> MOD07[MOD-07: Backup Export Verification Modal]
    SCR05 --> MOD08[MOD-08: Restore Database Wizard & Rollback Protection]
    SCR05 --> KEYS[SCR-05 Tab Phím Tắt - Keyboard Shortcuts Editor]
    KEYS --> MOD16[MOD-16: Keybinding Capture Modal - Ghi Phím Tắt]
    
    App --> MOD09[MOD-09: Graceful Shutdown Progress Overlay]
    App --> MOD12[MOD-12: Single-Instance Alert Toast]
    App --> MOD13[MOD-13: Privileged Action & Confirmation Modal - ConfirmDialog]
    App --> MOD15[MOD-15: Interactive Prompt & Quick Input Modal - PromptDialog]
    App --> MOD18[MOD-18: SSH Host Key Verification]
    App --> MOD19[MOD-19: SSH Auth Prompt - Mật khẩu / Passphrase / OTP]
```

---

## 3. Thiết kế Chi tiết Từng Màn hình (Screen Specifications)

---

### Màn hình 1: SCR-01 — Terminal Workspace & Execution Dashboard (Tilix-Style Tiling Architecture)

> **Chi tiết thiết kế kiến trúc nâng cao:** Xem toàn bộ đặc tả giải thuật cây nhị phân, cấu trúc dữ liệu và ma trận phím tắt tại [docs/tilix-tiling-spec.md](./tilix-tiling-spec.md).

#### 1. Mục đích & Vai trò
Màn hình trung tâm điều hành hàng ngày của lập trình viên và kỹ sư hệ thống. Cho phép kích hoạt chạy theo nhóm hoặc từng lệnh riêng lẻ, giám sát trực quan các terminal PTY, xem log đầu ra gần đây từ ring buffer và kiểm soát vòng đời tiến trình. 

Đặc biệt, hệ thống kế thừa triết lý phân chia không gian tối ưu từ **Tilix Tiling Terminal Emulator**:
1. **Kiến trúc Chia Lát Gạch Tự Do (Recursive Binary Tree Tiling):** Vượt qua giới hạn chia 2 khung cứng nhắc (Dual-Pane A/B), cho phép chia đệ quy vô hạn cấp (Split Right `Ctrl+Alt+R`, Split Down `Ctrl+Alt+D`), dễ dàng tạo bố cục 3 panel (1 chính + 2 phụ), lưới 4 panel (2x2 Grid), hoặc 3 cột song song.
2. **Quản lý Đa Phiên Làm Việc (Multi-Workspace Sessions & Sessions Drawer):** Tổ chức các luồng công việc thành các Session độc lập (vd: Session "Web Dev", Session "Docker Cluster", Session "Monitoring").
3. **Mini-Headerbar Trên Từng Panel (Per-Pane Chrome - 28px):** Mỗi ô terminal sở hữu một header nhỏ tích hợp chỉ báo trạng thái tiến trình, tiêu đề động (OSC 0/2), huy hiệu PID, và các nút thao tác nhanh: Chia Phải, Chia Dưới, Phóng To (Zoom 100%), Đồng Bộ Phím (Sync), Dừng và Đóng.
4. **Thu Phóng Tức Thì (Pane Zoom / Maximize - Ctrl+Shift+Z):** Cho phép 1 panel mở rộng chiếm 100% diện tích workspace để soi log mà không làm hỏng cấu trúc cây tiling đã dựng.
5. **Đồng Bộ Bàn Phím Đa Nhiệm (Synchronized Keystroke Broadcasting):** Phát sóng cùng lúc nội dung gõ phím tới tất cả các terminal trong Session hoặc theo nhóm màu (Group), lý tưởng cho quản trị cụm máy chủ và container.
6. **Thư Viện Bố Cục Mẫu (Layout Presets):** 1-click kích hoạt ngay các layout quen thuộc: Single, 2 Cột, 2 Hàng, 2x2 Grid, 1 Lớn + 2 Nhỏ (1+2 Tiled), 3 Cột, hoặc lưu layout tùy biến.

#### 2. Wireframe Chi tiết

##### 2.1. Wireframe Chế độ Tilix Tiling N-Panes (Bố cục 1 Lớn + 2 Nhỏ & Thanh Sessions Drawer)

```
+-------------------------------------------------------------------------------------------------------------------------+
| [=] Workspace   | Phiên: Dev Stack (Started 14:20:05)   [⚡ 3 Ngầm] [Layout: ⊞ 1+2 ▾] [🔗 Sync: Session] [⏹ Stop All]   |
+-----------------+-------------------------------------------------------------------------------------------------------+
| SESSIONS DRAWER | KHUNG CHÍNH (PANE 1 - Active Focus)       | KHUNG PHỤ 1 (PANE 2 - Top Right)                          |
| ≡ SESSIONS  [+] | [●] api-server · PID 3102  ~/repo/api     | [●] vite-dev · PID 4091  [🔗 SYNC]  ~/repo/web            |
|-----------------| [◫ Chia Phải] [⬒ Chia Dưới] [🗖 Zoom] [✕] | [◫ Chia Phải] [⬒ Chia Dưới] [🗖 Zoom] [✕]                 |
| v Dev Stack (3) |-------------------------------------------+-----------------------------------------------------------|
|   [⊞ 1+2 Tiled] | [NestFactory] Starting Nest application.. | VITE v5.4.2 ready in 280 ms                               |
|   ● 3 running   | [RoutesResolver] AppController {/api}:    | ➜ Local:   http://localhost:5173/                         |
|                 |   Mapped {/api/v1/commands, GET} +2ms     | ➜ Network: http://192.168.1.15:5173/                      |
| > Docker Infra  | [HTTP] 10:14:08 GET /api/v1/health 200 OK |=================== SASH PHÂN CÁCH NGANG (ROW-RESIZE) =====|
|   [◫ 2 Cột Dọc] | ➜ cm-api git:(main) $                     | KHUNG PHỤ 2 (PANE 3 - Bottom Right)                       |
|   ● 1 running   |                                           | [●] worker-daemon · PID 4120  [🔗 SYNC]  ~/repo           |
|                 |                                           | [◫ Chia Phải] [⬒ Chia Dưới] [🗖 Zoom] [✕]                 |
| > Production    |                                           |-----------------------------------------------------------|
|   [⊞ 2x2 Grid]  |                                           | 1:M 28 Sep 10:15:01 * Ready to accept Redis connections   |
|   ○ Inactive    | <==== Viền Active Focus tím #744791 ====> | [WorkerEngine] Heartbeat acknowledged (queue: 0)          |
+-----------------+-------------------------------------------+-----------------------------------------------------------+
| Active: 3 panes | Ring Buffer: 680KB/2MB | Tiling: 1+2 Main/Sub | Sync: 2 Panes Linked | Shortcuts: Ctrl+Alt+R / D / S     |
+-------------------------------------------------------------------------------------------------------------------------+
```

##### 2.2. Wireframe Bố cục Lưới 4 Ô Cân Xứng (2x2 Grid Layout Mode)

```
+----------------------------------------------------------------------------------------------------+
| [=] Workspace   | Phiên: Microservices Cluster [Layout: ⊞ 2x2 Grid] [🔗 Sync: Off]   [⏹ Stop All]  |
+-----------------+-------------------------------------------------+--------------------------------+
| GROUPS / SESS   | PANEL 1: auth-service (PID: 3201)               | PANEL 2: billing-api (PID: 3205)|
| Search..        | [◫][⬒][🗖][✕] ~/repo/auth                        | [◫][⬒][🗖][✕] ~/repo/billing    |
| v Services (4)  |-------------------------------------------------+--------------------------------|
|   ● auth        | [AuthService] Initialized JWT public keys       | [Billing] Stripe webhook ready |
|   ● billing     | ➜ listening on :4001                            | ➜ listening on :4002           |
|   ● notifications========================= SASH NGANG (50%) =======================================|
|   ● analytics   | PANEL 3: notifications (PID: 3210)              | PANEL 4: analytics (PID: 3218) |
|                 | [◫][⬒][🗖][✕] ~/repo/notify                      | [◫][⬒][🗖][✕] ~/repo/analytics  |
|                 |-------------------------------------------------+--------------------------------|
|                 | [RabbitMQ] Channel connected: 'email_queue'     | [ClickHouse] Sink buffer 1024  |
|                 | ➜ listening on :4003                            | ➜ streaming telemetry to OLAP  |
+-----------------+-------------------------------------------------+--------------------------------+
```

##### 2.3. Wireframe Chế độ Thu Phóng Toàn Màn hình (Pane Zoom / Maximize State - Ctrl+Shift+Z)

```
+----------------------------------------------------------------------------------------------------+
| [=] Workspace   | Phiên: Dev Stack   [🗗 ĐANG ZOOM PANEL: api-server] [Ctrl+Shift+Z để khôi phục]  |
+-----------------+----------------------------------------------------------------------------------+
| SESSIONS DRAWER | PANEL 1: api-server (PID: 3102) - PHÓNG TO 100% DIỆN TÍCH      [🗗 Thu nhỏ] [⏹] [✕]|
| ≡ SESSIONS      | [● Running] · ttys004 · ~/repo/api · Kích thước viewport: 180x48                 |
|-----------------|----------------------------------------------------------------------------------+
| v Dev Stack (3) | [NestFactory] Starting Nest application...                                       |
|   [ZOOM ACTIVE] | [InstanceLoader] DatabaseModule dependencies initialized +36ms                   |
|   ● 3 running   | [DB] Connected to PostgreSQL pool (host: 127.0.0.1:5432, db: cm_dev, pool: 10)   |
|                 | [RoutesResolver] CommandController {/api/v1/commands}:                           |
|                 |   Mapped {/api/v1/commands, GET} route +2ms                                      |
|                 |   Mapped {/api/v1/commands/run, POST} route +1ms                                 |
|                 |   Mapped {/api/v1/health, GET} route +1ms                                        |
|                 | [Nest] application successfully started on port 3000                             |
|                 | [HTTP] 10:14:08.922 GET /api/v1/health 200 OK (2.1ms - 48 bytes)                 |
|                 | [HTTP] 10:14:12.304 POST /api/v1/commands/run 201 Created (14.2ms)              |
|                 | ➜ cm-api git:(main) $ _                                                          |
+-----------------+----------------------------------------------------------------------------------+
| Zoomed: 1 pane  | Bố cục gốc (1+2 Tiled) được bảo toàn trong bộ nhớ; bấm Ctrl+Shift+Z để trở lại    |
+----------------------------------------------------------------------------------------------------+
```

##### 2.4. Wireframe Bố Cục Thanh Bên Kèm Section Quick Access (Dual-Section Sidebar Wireframe)===== THANH PHÂN CÁCH SASH NGANG (KÉO CHUỘT / RESIZABLE) ========|
|                 | KHUNG DƯỚI (PANE B)                                                              |
|                 | [● bash-interactive-test] [x]  [+]                          [Actions: ⎚ | ⏹]     |
|                 |----------------------------------------------------------------------------------+
|                 | $ curl -X POST http://localhost:4000/api/v1/test-event                           |
|                 | {"success":true,"queued":1,"timestamp":"2026-09-28T10:15:05Z"}                   |
|                 | $ _                                                                              |
+-----------------+----------------------------------------------------------------------------------+
| Active: 2 panes | Pane A: 136x18 (Top)                          | Pane B: 136x14 (Bottom)          |
+----------------------------------------------------------------------------------------------------+
```

##### 2.4. Wireframe Bố Cục Thanh Bên Kèm Section Quick Access (Dual-Section Sidebar Wireframe)

```
+---------------------------+
| ≡ NHÓM LỆNH & PHIÊN   [⟳] |   <- header danh mục nhóm lệnh
|---------------------------|
| v Web Platform            |
|   [▷] [⏹] [Aut]           |
|   ● 01. db-init           |   Section 1: GroupTree (cuộn riêng)
|   ● 02. backend           |
| > Microservices           |
| > Standalone              |
|===========================|   <- sash kéo được (row-resize), nháy đúp = 50:50
| ⚡ QUICK ACCESS  (5)   [⌄] |   <- header section 2, [⌄] thu gọn / mở rộng
| → Terminal 2 · Khung A     |   <- chỉ báo terminal đích (§3.5)
| [🔍 Lọc...]               |   <- chỉ hiện khi có > 8 item
|---------------------------|
| Git Status        [▷][⎘]  |
|   git status -sb          |
| Docker PS         [▷][⎘]  |   Section 2: danh sách Quick Access (cuộn riêng)
|   docker ps --format ...  |
| Tail API log  ⚠   [▷][⎘]  |
|   tail -f logs/api.log    |
+---------------------------+
```

#### 3. Các Thành phần Giao diện & Data Binding
* **Left Sub-Sidebar: Execution Explorer (Tree View) & Quick Access Panel:**
  * **Cấu trúc 2 Section Dọc:** Thanh bên được chia dọc thành 2 section độc lập bằng thanh phân cách co giãn (Resizable Sash):
    * **Section 1 (Top - GroupTree):** Cây phân cấp nhóm lệnh (`command_group` -> `group_membership` -> `command_definition`), số thứ tự `execution_order` (`01.`, `02.`), trạng thái tiến trình (Idle, Starting, Running, Completed, Failed) và nút điều khiển nhanh nhóm (`[▷]` Play, `[⏹]` Stop).
    * **Thanh Phân Cách Sash Ngang (Row Splitter):** Cho phép kéo thả chuột để điều chỉnh tỉ lệ chiều cao giữa GroupTree và QuickAccessPanel (`cursor: row-resize`). Ràng buộc tối thiểu `min-height: 120px` cho mỗi section. Thao tác nháy đúp (`dblclick`) lập tức khôi phục tỉ lệ cân bằng chuẩn **50:50**. Trạng thái tỉ lệ và thu gọn được lưu vào `localStorage` (`cm_quick_access_ratio_v1`, `cm_quick_access_collapsed_v1`).
    * **Section 2 (Bottom - Quick Access Panel):** Danh sách các lệnh đã được bật cờ `quick_access`, hỗ trợ gửi chuỗi thực thi trực tiếp vào terminal đang focus mà không cần mở tab PTY riêng.
  * **Các Thành Phần Của Quick Access Panel:**
    * **Header Panel:** Tiêu đề `⚡ QUICK ACCESS`, huy hiệu đếm số lượng lệnh `(N)`, và nút toggle thu gọn/mở rộng `[⌄]`. Khi thu gọn, Section 2 co lại thành header thanh mảnh (28px), Section 1 chiếm trọn không gian còn lại.
    * **Chỉ Báo Terminal Đích (Target Terminal Indicator):** Hiển thị nhãn realtime `→ <tên tab> · Khung A/B` (sử dụng `tabLabel()`, phản ánh tiêu đề chương trình như `→ ✳ Claude Code · Khung A`). Khi tab đích đang bận chạy chương trình (`shellPhase = running`), hiển thị thêm `· đang bận`. Khi tab đích đã kết thúc tiến trình, hiển thị màu cảnh báo `(đã dừng)`.
    * **Thanh Lọc Nhanh (Filter Input):** Ô tìm kiếm `[🔍 Lọc...]` tự động kích hoạt hiển thị khi số lượng lệnh ghim Quick Access vượt quá 8 item.
    * **Quick Access Item Card:**
      * **Dòng 1:** Tên lệnh (`name`, chống tràn `text-overflow: ellipsis`), huy hiệu cảnh báo `⚠` nếu có độ lệch môi trường shell hoặc lệnh đa dòng trên shell thiếu bracketed paste, và 2 nút icon hành động:
        - `[▷]` **Run (`Play`):** Dán chuỗi lệnh vào terminal đích và gửi Enter thực thi ngay. Nếu dòng lệnh đang có chữ dở và con trỏ ở cuối dòng, hệ thống tự xóa dòng cũ bằng ký tự DEL rồi dán và chạy; nếu con trỏ ở giữa dòng, hệ thống chặn gửi và hiện toast nhắc nhở an toàn.
        - `[⎘]` **Paste (`ClipboardPaste`):** Chỉ dán chuỗi lệnh vào vị trí con trỏ của terminal đích mà không gửi Enter, cho phép người dùng kiểm tra và chỉnh sửa trước khi chạy.
      * **Dòng 2:** Chuỗi thực thi (`execution_string`) hiển thị dạng code monospace thu nhỏ 1 dòng; nếu lệnh có nhiều dòng, hiển thị dòng đầu kèm badge `↵ +N`.
      * **Tương tác Nháy Đúp (`dblclick`):** Nháy đúp vào item tương đương thao tác **Paste** an toàn.
      * **Tooltip & Khả Năng Tiếp Cận (`aria-label`):** Nêu rõ hành động và tên terminal đích, ví dụ: *"Run: Chạy 'Git Status' trong Terminal 1 · Khung A"*.
  * **Quy Tắc Hiển Thị Động Của Quick Access Panel:**
    * **Khi không có terminal đích:** (Workspace chưa mở tab nào, hoặc ở chế độ Split mà khung active đang trống): Section 2 và thanh phân cách sash **ẩn hoàn toàn**, GroupTree tự động giãn 100% chiều cao thanh bên.
    * **Empty State:** Khi chưa có lệnh nào được bật Quick Access, hiển thị giao diện rỗng gồm icon `Zap`, thông điệp *"Chưa có lệnh Quick Access. Bật switch ⚡ trong màn Lệnh"* và nút bấm `[Mở màn Lệnh]` điều hướng sang `/commands`.
    * **Trạng thái Tiến trình Đích Dừng:** Nếu tab đích đã dừng tiến trình, section vẫn hiển thị để theo dõi nhưng 2 nút Run và Paste chuyển sang trạng thái disabled kèm tooltip lý do.
* **Top Status Strip & Background Processes Button:**
  * Thẻ chỉ báo **`[⚡ 2 Ngầm]` / `[Active Daemons]`**: Cho biết số lượng tiến trình PTY đang chạy ngầm trong kernel backend Rust.
  * Click vào nút để bật/tắt **Menu Quản lý Tiến trình Chạy ngầm (Background Processes Manager Panel / Popover)**.
* **Menu Quản lý Tiến trình Chạy Ngầm (Background Processes Panel / Modal):**
  * **Header Panel:** Tiêu đề *"Tiến trình đang chạy ngầm (Active Daemons)"*, tổng số tiến trình active, nút **"Dừng Tất Cả Ngầm"** (Stop All Daemons) và nút đóng menu (Esc).
  * **Thanh Tiêu Đề Cột (List Column Headers):** Hiển thị 3 cột phân định thẳng hàng: `TIẾN TRÌNH / TÊN`, `TRẠNG THÁI TAB & BỘ NHỚ`, `THAO TÁC`.
  * **Danh sách tiến trình ngầm (Daemon List Items) - Bố cục 3 cột cố định, không rớt dòng:**
    * **Cột 1 - Định danh & Tên:** Dot trạng thái hoạt động (active running pulse); Huy hiệu phân loại (`[Terminal]` màu tím PTY vs `[Lệnh]` màu xanh command); Tên hiển thị đầy đủ (hiển thị tên tùy biến đã đổi, ví dụ `Dev Server`, hoặc `Terminal (POWERSHELL)` nếu là mặc định, với cơ chế chống rớt dòng `text-overflow: ellipsis`); Dòng meta hiển thị `#commandId`, `PID` và loại shell.
    * **Cột 2 - Trạng thái Tab & Bộ nhớ Ring Buffer:** Căn lề thẳng hàng tuyệt đối trên toàn bộ danh sách; Huy hiệu trạng thái UI Tab (**`[👁️ Tab đang hiển thị]`** màu xanh blue Attached hoặc **`[👁️‍🗨️ Tab đang ẩn (Detached)]`** màu vàng cam khi đóng ẩn tab); Dòng telemetry hiển thị dung lượng bộ đệm Ring Buffer (ví dụ `Buffer: 118 B · Running`).
    * **Cột 3 - Thao tác Hành động trực tiếp:** Nút **`[👁️ Mở lại Tab / Focus Tab]`** (khôi phục tab trên Dockview kèm tên tùy biến; khi ở chế độ Split Mode có tùy chọn mở vào Pane A hoặc Pane B); nút **`[📜 Xem Log]`** (mở drawer Ring Buffer); nút **`[⏹ Dừng]`** (SIGTERM); nút **`[💀 Force Kill]`** (SIGKILL).
* **Main Area: Chế độ Tilix Tiling Terminal Workspace (Recursive Binary Tree & Multi-Session PTY):**
  * **Kiến trúc Cây Nhị phân Đệ quy (Recursive Binary Tree Tiling Engine):**
    - Thay thế mô hình 2 pane tĩnh bằng cấu trúc cây đệ quy `TilingNode` (chi tiết tại [docs/tilix-tiling-spec.md](./tilix-tiling-spec.md)).
    - Mỗi Session quản lý một cây layout độc lập, hỗ trợ chia đôi N lần theo chiều ngang hoặc dọc.
    - Hỗ trợ thêm/bớt panel linh hoạt: khi chia đôi (Split Right `Ctrl+Alt+R` hoặc Split Down `Ctrl+Alt+D`), panel mới được chèn làm nhánh con kề cạnh; khi đóng panel (`[✕]` hoặc `Ctrl+Shift+W`), không gian được tự động hoàn lại cho sibling node kề cạnh.
  * **Thanh Công Cụ Workspace & Layout Presets Toolbar:**
    - **Menu Layout Presets (Bố cục Mẫu):**
      - `[◻] Single Pane` (100% diện tích).
      - `[◫] 2 Columns` (50:50 trái - phải).
      - `[⬒] 2 Rows` (50:50 trên - dưới).
      - `[⊞] 2x2 Grid` (4 ô vuông đối xứng).
      - `[◨] 1+2 Tiled` (1 panel chính 60% bên trái + 2 panel phụ xếp tầng bên phải).
      - `[⬓] 1 Top Main + 2 Bottom` (1 panel chính nằm trên, 2 panel phụ nằm dưới).
      - `[⚏] 3 Columns` (3 cột song song 33:33:33).
      - `[💾] Lưu Layout Thành Mẫu Mới...`: Lưu cấu trúc phân nhánh hiện tại vào danh sách preset.
    - **Bộ Điều Khiển Phát Sóng Gõ Phím Đồng Bộ (Synchronized Input Broadcast):**
      - Toggle switch: `Off` | `Session` | `Group`.
      - Khi bật `Session` hoặc `Group`, bất kỳ ký tự nào gõ vào panel đang active sẽ được phát sóng đồng thời qua IPC stdin tới tất cả các terminal liên kết.
      - Panel đang được liên kết hiển thị viền phát xung và huy hiệu `[🔗 SYNC]` màu tím/xanh nổi bật.
    - **Nút Batch Action:** `[▷ Chạy lại cả Session]`, `[⏹ Dừng cả Session]`.
  * **Mini-Headerbar Trên Từng Terminal Panel (28px Per-Pane Chrome):**
    - Nằm ngay trên đỉnh mỗi terminal viewport, trang bị đầy đủ controls cục bộ:
      - Dot trạng thái hoạt động: 🟢 Running, 🟡 Starting, ⚪ Exited, 🔴 Failed.
      - Tên terminal tùy biến (hỗ trợ nháy đúp đổi tên MOD-14) và tiêu đề chương trình (OSC 0/2).
      - Huy hiệu PID và thư mục CWD hiện tại.
      - Huy hiệu `[🔗 SYNC]` khi đang ở chế độ gõ phím đồng bộ.
      - Cụm nút thao tác nhanh:
        - `[◫]` Split Right (Chia sang phải).
        - `[⬒]` Split Down (Chia xuống dưới).
        - `[🗖]` Zoom / Maximize (Phóng to 100% / Khôi phục layout cũ bằng `Ctrl+Shift+Z`).
        - `[🔄]` Reattach Buffer (Xả lại 1MB/2MB log từ ring buffer).
        - `[⎚]` Clear (Xóa màn hình xterm cục bộ).
        - `[⏹]` Stop Process (Gửi SIGTERM).
        - `[✕]` Close Pane (Thu hồi panel khỏi cây nhị phân, tiến trình vẫn chạy ngầm nếu là daemon).
  * **Thanh Phân Cách Điều Chỉnh Kích Thước Đa Trục (Interactive Multi-Axis Resizable Sashes):**
    - Phân bố dọc và ngang giữa các panel kề nhau.
    - Độ dày 5px, hover/drag chuyển sang màu chủ đạo `--primary: #744791` kèm hiệu ứng glow.
    - Con trỏ tự động chuyển `col-resize` hoặc `row-resize`.
    - Ràng buộc an toàn: Tối thiểu `min-width: 200px` và `min-height: 120px` mỗi ô terminal để tránh co rút xterm.js.
    - Nháy đúp (`dblclick`) vào thanh sash bất kỳ lập tức cân bằng lại 2 panel con về tỉ lệ chuẩn **50:50**.
  * **Cơ Chế Thu Phóng Tức Thì (Pane Zoom / Maximize):**
    - Cho phép lập trình viên tập trung quan sát log của 1 panel duy nhất trong chốc lát mà không cần đóng các panel khác.
    - Khi Zoom được kích hoạt, layout cây nhị phân được "đóng băng" (freeze) trong bộ nhớ, viewport được mở rộng 100% diện tích workspace.
    - Nhấn `Ctrl+Shift+Z` lần nữa sẽ "giải đông" (unfreeze) và trả lại chính xác vị trí, kích thước dòng cột ban đầu.
  * **Thanh Quản Lý Phiên Làm Việc (Sessions Drawer / Sidebar):**
    - Cho phép tạo nhiều Phiên làm việc (Sessions), mỗi phiên có 1 cây tiling layout riêng.
    - Hiển thị danh sách card kèm sơ đồ thu nhỏ (mini layout thumbnail), số lượng terminal đang chạy.
    - Chuyển đổi qua lại giữa các Session mượt mà bằng phím tắt `Ctrl+PageUp` / `Ctrl+PageDown`.
  * **Cơ Chế Đồng Bộ Kernel PTY & FitAddon Độc Lập:**
    - Mỗi `XtermPane` trong từng ô panel sở hữu một instance `ResizeObserver` độc lập.
    - Khi kéo bất kỳ thanh sash nào hoặc đổi kích thước cửa sổ app:
      - Từng instance xterm gọi `fitAddon.fit()` theo kích thước pixel thực tế của ô panel đó.
      - Backend Rust nhận sự kiện `pty_resize({ commandId, cols, rows })` và cập nhật kích thước terminal kernel.
      - Đảm bảo các giao diện tương tác CLI (như `htop`, `vim`, `nano`, các bảng log kẻ khung) tự động vẽ lại vừa khít với kích thước của từng Pane mà không bị vỡ chữ hay lệch hàng.

#### 4. Các Trạng thái Giao diện (UI States)
* **Single View State:** Chế độ 1 khung mặc định. Toàn bộ các tab nằm chung trên một Tab Strip duy nhất chiếm 100% chiều rộng.
* **Split Horizontal State (Left | Right):** Chia đôi theo chiều dọc. Hai khung Pane A và Pane B hiển thị song song trái-phải, ngăn cách bởi thanh Sash dọc.
* **Split Vertical State (Top / Bottom):** Chia đôi theo chiều ngang. Hai khung hiển thị trên-dưới, ngăn cách bởi thanh Sash ngang.
* **Sash Dragging State:** Trạng thái người dùng đang nhấn giữ chuột kéo thanh phân cách. Cả 2 viewport terminal hiển thị lớp phủ trong suốt (transparent pointer-events guard) để việc kéo chuột không bị chặn bởi iframe hoặc canvas xterm, con trỏ giữ nguyên dạng `col-resize` hoặc `row-resize`.
* **Cross-Pane Dragging State:** Trạng thái đang kéo một tab bay qua ranh giới giữa 2 khung. Tab Strip của khung đích hiển thị vạch placeholder đánh dấu vị trí sẽ chèn vào.
* **Empty State:** Chưa có nhóm hoặc lệnh nào chạy: Hiển thị minh họa đồ họa, thông điệp *"Không có phiên chạy nào đang mở. Chọn một nhóm lệnh từ danh sách bên trái hoặc nhấn Play để bắt đầu"*.
* **Reattaching State:** Khi người dùng mở lại một tab đã bị đóng trước đó: Hiển thị spinner nhẹ *"Đang kết nối lại PTY & xả dữ liệu từ bộ nhớ đệm..."* trước khi xterm render lại nội dung.
* **Process Exit State:** Khi tiến trình kết thúc (dù thành công hay thất bại), xterm vẫn giữ nguyên nội dung kèm một thông báo thanh mảnh ở góc trên terminal: `[Process exited with code 0 - Press Enter or Click Restart to rerun]`.

---

### Màn hình 2: SCR-02 — Quản lý Thư viện Lệnh (Command Library)

#### 1. Mục đích & Vai trò
Nơi định nghĩa, cấu hình và quản trị toàn bộ các câu lệnh độc lập (`command_definition`). Quản lý cách thức thực thi rõ ràng: Chạy trực tiếp qua mảng đối số (argv) hay qua vỏ lệnh (shell), bảo vệ tính toàn vẹn của chuỗi thực thi.

#### 2. Wireframe Chi tiết

```
+----------------------------------------------------------------------------------------------------------+
| [=] Command Library   [🔍 Search...]   [Filter: All / Shell / Argv / ⚡ Quick Access]   [+ Tạo Lệnh Mới]   |
+----------------------------------------------------------------------------------------------------------+
| TÊN LỆNH           | KIỂU THỰC THI | CHUỖI LỆNH                        | NHÓM         | ⚡ QUICK | HÀNH ĐỘNG     |
|--------------------+---------------+-----------------------------------+--------------+----------+---------------|
| Git Status         | [Shell: zsh]  | git status -sb                    | (Chưa gán)   |  [●━]    | [▷] [✎] [⧉] [🗑] |
| Vite Frontend Dev  | [Shell: bash] | pnpm --filter web dev --port 3000 | Web Platform |  [━○]    | [▷] [✎] [⧉] [🗑] |
| Docker PS          | [Direct Argv] | docker ps --format "{{.Names}}"   | Infra        |  [●━]    | [▷] [✎] [⧉] [🗑] |
+----------------------------------------------------------------------------------------------------------+
| Tổng cộng: 14 lệnh | 8 Shell | 6 Argv | ⚡ 5 Quick Access                                                   |
+----------------------------------------------------------------------------------------------------------+
```

#### 3. Bảng Dữ liệu & Quy cách Thao tác
* **Các cột hiển thị:**
  1. **Tên Lệnh (`name`):** Nhãn gợi nhớ thân thiện (ví dụ: `Vite Frontend Dev`).
  2. **Kiểu Thực thi (`is_shell`):** Badge trực quan:
     * 🟩 `Direct Argv`: Phân tách argv tường minh, an toàn, không chạy qua trung gian shell.
     * 🟨 `Shell (bash/sh)`: Chạy qua shell hệ thống để hỗ trợ piping (`|`), redirect (`>`), biến môi trường (`$VAR`).
  3. **Chuỗi Lệnh (`execution_string`):** Hiển thị dạng code mono block có truncate kèm nút "Copy".
  4. **Nhóm thuộc về:** Liệt kê các nhóm lệnh đang chứa command này (từ bảng `group_membership`).
  5. **Quick Access (`quick_access`):**
     * Switch toggle bật/tắt trực tiếp trên từng dòng table row, có hiệu lực ngay lập tức qua IPC `commands_set_quick_access(id, enabled, confirmed: true)`.
     * Switch hiển thị trạng thái loading spinner / disabled trong lúc chờ backend xác nhận; nếu gặp sự cố sẽ tự động hoàn nguyên (rollback) về trạng thái cũ và kích hoạt Toast cảnh báo lỗi.
     * Tooltip switch: *"Hiện lệnh này trong mục Quick Access ở màn Terminal"*.
  6. **Hành động (Action Buttons):**
     * `[▷]` Chạy ngay lập tức (mở sang Workspace).
     * `[✎]` Chỉnh sửa (mở Modal MOD-02).
     * `[⧉]` Nhân bản lệnh (Duplicate).
     * `[🗑]` Xóa lệnh (Yêu cầu xác nhận an toàn MOD-03).
* **Bộ Lọc Nhanh & Thống Kê Thanh Điều Hướng:**
  * **Bộ lọc `⚡ Quick Access`:** Bổ sung tab filter nhanh `⚡ Quick Access` vào cụm tab `All` / `Shell` / `Argv`, cho phép lọc ngay lập tức các lệnh đang được gắn cờ.
  * **Chỉ số Telemetry Footer:** Hiển thị số lượng lệnh Quick Access (ví dụ `⚡ 5 Quick Access`) bên cạnh tổng số lệnh và phân loại thực thi.

---

### Màn hình 3: SCR-03 — Quản lý Nhóm Lệnh & Điều phối Thứ tự Chạy (Command Groups & Sequencer)

#### 1. Mục đích & Vai trò
Thiết lập danh mục các nhóm lệnh (`command_group`), gắn cờ tự khởi động cùng app (`autostart`), và sắp xếp thứ tự thực thi tuần tự (`execution_order`) giữa các lệnh trong nhóm.

#### 2. Wireframe Chi tiết

```
+----------------------------------------------------------------------------------------------------+
| [=] Command Groups       [🔍 Search groups...]                             [+ Tạo Nhóm Mới]        |
+----------------------------------------------------------------------------------------------------+
| [CARD 1: Web Platform Development]                                             [Autostart: [ON ] ] |
| Mô tả: Khởi động toàn bộ stack phát triển cục bộ cho web app                   Trạng thái: 2/3 run |
| Lệnh thực thi tuần tự theo execution_order:                                                        |
|   1. ⠿ [Argv] docker-postgres-start    (Timeout: 10s)                          [Active: Code 0]    |
|   2. ⠿ [Shell] nestjs-backend-api      (Wait: until ready)                     [Running: PID 2845] |
|   3. ⠿ [Shell] vite-frontend-dev       (Wait: parallel)                        [Running: PID 2841] |
| Hành động: [▷ Chạy toàn nhóm]  [⏹ Dừng toàn nhóm]  [✎ Sắp xếp & Chỉnh sửa]  [🗑 Xóa nhóm]          |
|----------------------------------------------------------------------------------------------------|
| [CARD 2: Cloudflare Remote Tunnel]                                             [Autostart: [OFF] ] |
| Mô tả: Mở đường hầm bảo mật cho staging preview                                Trạng thái: Idle    |
| Lệnh thực thi:                                                                                     |
|   1. ⠿ [Argv] cloudflared tunnel run dev-tunnel                                [Idle]              |
| Hành động: [▷ Chạy toàn nhóm]  [⏹ Dừng toàn nhóm]  [✎ Sắp xếp & Chỉnh sửa]  [🗑 Xóa nhóm]          |
|----------------------------------------------------------------------------------------------------|
| [CARD 3: Database Migration & Seed]                                            [Autostart: [OFF] ] |
| Mô tả: Chạy migrate DB rồi tự thoát                                            Trạng thái: Idle    |
| Lệnh thực thi:                                                                                     |
|   1. ⠿ [Shell] pnpm prisma migrate dev                                         [Idle]              |
|   2. ⠿ [Shell] pnpm prisma db seed                                             [Idle]              |
| Hành động: [▷ Chạy toàn nhóm]  [⏹ Dừng toàn nhóm]  [✎ Sắp xếp & Chỉnh sửa]  [🗑 Xóa nhóm]          |
+----------------------------------------------------------------------------------------------------+
```

#### 3. Các Tính năng Đặc thù
* **Autostart Switch:** Toggle trực tiếp cờ `autostart` trên card. Khi bật ON, hệ thống kích hoạt nhóm này tự động ngay khi ứng dụng khởi chạy và đã nắm giữ Single-Instance Lock.
* **Kéo thả Sắp xếp Thứ tự (`execution_order`):** Icon tay cầm kéo thả `⠿` cho phép hoán đổi vị trí nhanh các lệnh, tự động cập nhật lại trường `execution_order` (1, 2, 3...) trong SQLite `group_membership`.
* **Play All Sequence:** Khi nhấn "Chạy toàn nhóm", Process Manager Rust sẽ duyệt theo thứ tự `execution_order`.

---

### Màn hình 4: SCR-04 — Lịch sử Phiên Chạy & Nhật ký Sự kiện (Session History & Logs)

#### 1. Mục đích & Vai trò
Theo dõi các phiên chạy trong quá khứ được lưu tĩnh trong SQLite (`run_session` và `run_event`). Cung cấp thông tin chẩn đoán lỗi (exit code, thời lượng chạy, PID lúc chạy) mà không làm phình cơ sở dữ liệu do không ghi stream log PTY vào đĩa.

#### 2. Wireframe Chi tiết

```
+----------------------------------------------------------------------------------------------------+
| [=] Session History & Event Logs     [Filter: All Groups / Web Platform]  [Date: Today / Last 7d]  |
+----------------------------------------------------------------------------------------------------+
| DANH SÁCH PHIÊN (run_session)       | CHI TIẾT SỰ KIỆN THỰC THI (run_event của session #108)        |
|-------------------------------------+--------------------------------------------------------------|
| [SESSION #108] Web Platform Dev     | Nhóm: Web Platform Dev | Bắt đầu: 2026-09-25 14:20:05        |
| Started: 14:20:05 | Status: RUNNING | Trạng thái tổng: Đang chạy (Active)                          |
| 3 commands | 2 active, 1 ended      |--------------------------------------------------------------|
|                                     | COMMAND          | STARTED  | ENDED    | EXIT | PID  | LOG MEMORY|
| [SESSION #107] DB Migration & Seed  |------------------+----------+----------+------+------+-----------|
| Started: 11:05:12 | Status: SUCCESS | docker-postgres  | 14:20:05 | 14:20:10 | 0    | 2830 | [Xem Log] |
| Duration: 45s | Exit Code: 0        | nestjs-backend   | 14:20:10 | -        | -    | 2845 | [Xem PTY] |
|                                     | vite-frontend    | 14:20:12 | -        | -    | 2841 | [Xem PTY] |
| [SESSION #106] Cloudflare Tunnel    |--------------------------------------------------------------|
| Started: 09:15:00 | Status: FAILED  | Chẩn đoán nhanh:                                             |
| Exit code 1 (Tunnel credentials err)| Lệnh 'docker-postgres' kết thúc thành công (exit 0) sau 5s.  |
|                                     | Tiến trình backend & frontend đang stream PTY in-memory.     |
+-------------------------------------+--------------------------------------------------------------+
```

#### 3. Quy định Hiển thị & Lưu trữ Dữ liệu
* **Tách biệt Triệt để DB vs In-Memory:**
  * DB SQLite chỉ lưu: Thời gian bắt đầu (`started_at`), Thời gian kết thúc (`ended_at`), Trạng thái (`status`), Mã thoát (`exit_code`), và Mã tiến trình (`pid` phục vụ chẩn đoán tạm thời).
  * Cột **LOG MEMORY:**
    * Nếu phiên đang chạy hoặc vừa chạy trong phiên app hiện tại: Cho phép nhấn `[Xem PTY]` để xem nội dung từ Ring Buffer in-memory.
    * Nếu app đã từng khởi động lại hoặc session cũ: Nút chuyển sang trạng thái xám `[Đã giải phóng]` kèm tooltip: *"Tuân thủ kiến trúc bảo mật & hiệu năng: Log luồng PTY không lưu trữ ra đĩa cứng"*.

---

### Màn hình 5: SCR-05 — Cài đặt Hệ thống, Sao lưu & Khôi phục (Settings, Backup & Restore)

#### 1. Mục đích & Vai trò
Quản lý cấu hình cấp ứng dụng, cơ chế tự khởi động cùng OS, quản lý bộ nhớ đệm PTY, và thực hiện quy trình sao lưu / phục hồi SQLite an toàn với cơ chế kiểm tra tính toàn vẹn (Integrity Check) và Rollback tự động.

#### 2. Wireframe Chi tiết

```
+----------------------------------------------------------------------------------------------------+
| [=] Settings & Data Management                                                                     |
+----------------------------------------------------------------------------------------------------+
| [TAB 1: Tự Khởi Động & Hệ Thống]  [TAB 2: Terminal & Buffer]  [TAB 3: Sao Lưu & Khôi Phục Dữ Liệu] |
+----------------------------------------------------------------------------------------------------+
| 📦 SAO LƯU DỮ LIỆU CỤC BỘ (BACKUP / EXPORT)                                                        |
| Tạo bản sao snapshot toàn vẹn của cơ sở dữ liệu SQLite (WAL mode) bằng lệnh VACUUM INTO an toàn.    |
| Bản sao lưu bao gồm: Danh mục lệnh, Nhóm lệnh, Cấu hình thứ tự và Lịch sử phiên chạy.               |
|                                                                                                    |
| Vị trí lưu mặc định: ~/backups/command-manager/                                                    |
| Tên file tự sinh:    cm_backup_2026-09-25_160821.sqlite                                            |
|                                                                                                    |
| [ 📥 Xuất Bản Sao Lưu Ngay (Export Backup) ]  -- Kiểm tra PRAGMA integrity_check trước khi báo OK  |
|----------------------------------------------------------------------------------------------------|
| ⚠️ KHÔI PHỤC DỮ LIỆU (RESTORE / IMPORT)                                                             |
| Nạp dữ liệu từ file backup .sqlite bên ngoài vào ứng dụng.                                         |
| Quy trình an toàn 7 bước:                                                                          |
|   1. Kiểm tra tính toàn vẹn (integrity_check) & Schema version                                     |
|   2. Tự động sao lưu dự phòng file app.db hiện tại                                                 |
|   3. Dừng an toàn mọi tiến trình & phiên chạy đang active                                          |
|   4. Đóng toàn bộ connection SQLite, xóa sạch app.db-wal & app.db-shm                             |
|   5. Ghi đè file mới & Khởi tạo lại kết nối                                                        |
|   6. Tự động Rollback hoàn trả nếu việc mở DB mới gặp lỗi                                          |
|                                                                                                    |
| [ 📤 Chọn Tệp Sao Lưu Để Khôi Phục... ]                                                            |
|----------------------------------------------------------------------------------------------------|
| ⚙️ THIẾT LẬP VÒNG ĐỜI TIẾN TRÌNH & BỘ NHỚ                                                          |
| Kích thước Ring Buffer in-memory mỗi PTY:  [ 2 MB       ▼ ] (Giới hạn byte để chống tràn RAM)      |
| Thời gian chờ dừng tiến trình (Graceful):  [ 8 giây     ▼ ] (SIGTERM -> chờ timeout -> SIGKILL)    |
| Tự khởi động cùng Hệ điều hành (OS Boot):  [ BẬT (ON)   ▼ ] (tauri-plugin-autostart)               |
+----------------------------------------------------------------------------------------------------+
```

#### 3. Tab 4: Phím Tắt (Keyboard Shortcuts Editor)

##### 3.1. Mục đích & Phạm vi
Cho phép người dùng xem, tìm, đổi, thêm, gỡ và khôi phục phím tắt của mọi tác vụ trong ứng dụng, theo mô hình của VS Code (`Ctrl+K Ctrl+S`). Tab này thay cho bảng tĩnh ở mục 7.2: bảng đó chỉ còn là **danh sách mặc định**.

Hiện phím tắt nằm rải rác ở `AppShell.vue`, `DockHost.vue`, `GroupTree.vue` và `XtermPane.vue`, và nhãn gợi ý (`title="... (Ctrl+N)"`) được viết cứng. Để tính năng này hoạt động, mọi phím tắt phải đi qua **một registry và một bộ điều phối duy nhất** (mục 3.7).

**Ngoài phạm vi v1:**
* Chord hai bước kiểu `Ctrl+K Ctrl+S`. Định dạng dữ liệu vẫn để chỗ cho chord (mục 3.5).
* Biểu thức `when` tự do. Chỉ dùng danh sách phạm vi cố định ở mục 3.4.
* Phím tắt toàn hệ điều hành khi app không có focus.

* **Stitch Screen Reference:** Screen ID `1e3f3759e99e4a67a1fb95b6304db408` tại Project `14914224436748087443`.

##### 3.2. Wireframe Tab Phím Tắt

```
+----------------------------------------------------------------------------------------------------+
| [=] Settings & Data Management                                                                     |
+----------------------------------------------------------------------------------------------------+
| [Sao Lưu & Phục Hồi]  [Hệ Thống & Khởi Động]  [Terminal & Hiệu Năng]  [⌨ Phím Tắt]                |
+----------------------------------------------------------------------------------------------------+
| [🔍 Tìm theo tên tác vụ, ID hoặc phím...                                     ] [⌨ Ghi phím] [⋯]   |
| Lọc: (● Tất cả 28) ( Đã tùy chỉnh 3 ) ( ⚠ Xung đột 1 )      Phạm vi: [ Tất cả phạm vi      ▼ ]   |
|----------------------------------------------------------------------------------------------------|
| TÁC VỤ                              | PHÍM TẮT              | PHẠM VI            | NGUỒN              |
|----------------------------------------------------------------------------------------------------|
| ▾ ỨNG DỤNG & ĐIỀU HƯỚNG                                                                            |
|   Mở Command Palette                | [Ctrl] + [K]          | Toàn ứng dụng      | Mặc định           |
|   Thoát ứng dụng                    | [Ctrl] + [Q]          | Toàn ứng dụng      | Mặc định           |
|   Đi đến Cài đặt                    | [Ctrl] + [5]          | Toàn ứng dụng      | Mặc định           |
|                                     | [Ctrl]+[Shift]+[S]    | Toàn ứng dụng      | Mặc định           |
| ▾ WORKSPACE TERMINAL                                                                               |
| ● Mở terminal mới (cùng cwd)        | [Ctrl] + [T]          | Workspace          | Người dùng   ✎ ↺ ✕ |  <- hàng đã sửa: chấm xanh bên trái
| ⚠ Chia đôi panel sang phải          | [Ctrl]+[Alt]+[R]      | Workspace          | Mặc định           |  <- xung đột: icon vàng, hover xem chi tiết
|   Phóng to / khôi phục panel        | [Ctrl]+[Shift]+[Z]    | Workspace          | Mặc định           |
|   Đóng panel đang chọn              | —                     | Workspace          | Đã gỡ        ↺     |  <- binding mặc định bị gỡ: chữ mờ, gạch ngang
| ▾ TERMINAL                                                                                         |
|   Bật / tắt gợi ý lệnh              | [Ctrl] + [Space]      | Terminal đang gõ   | Mặc định           |
|   Nhận toàn bộ gợi ý mờ             | [End]  ·  [→]         | Terminal có gợi ý  | Cố định 🔒          |  <- phím cố định: chỉ xem
|----------------------------------------------------------------------------------------------------|
| ⓘ Phím tắt áp dụng ngay khi thay đổi, không cần bấm "Lưu Cài Đặt".                                 |
+----------------------------------------------------------------------------------------------------+

Menu [⋯]:  Xuất ra JSON...  |  Nhập từ JSON...  |  Khôi phục toàn bộ về mặc định
```

* **Ô tìm kiếm:** Lọc theo tên tác vụ, ID (`workspace.splitRight`) hoặc chuỗi phím (`ctrl+alt`). Kết quả khớp cập nhật ngay khi gõ, có debounce 100ms.
* **Nút `⌨ Ghi phím`:** Bật chế độ ghi. Khi bật, ô tìm kiếm không nhận ký tự mà ghi lại tổ hợp phím vừa nhấn, rồi lọc các tác vụ đang dùng đúng tổ hợp đó. Dùng để trả lời câu hỏi "phím này đang làm gì?". Bấm lại hoặc nhấn `Esc` để tắt.
* **Bộ lọc:** *Tất cả* · *Đã tùy chỉnh* (gồm hàng nguồn Người dùng và Đã gỡ) · *Xung đột*. Dropdown Phạm vi lọc theo mục 3.4.
* **Nhóm:** Tác vụ được nhóm theo danh mục của registry. Mỗi nhóm thu gọn được.
* **Một tác vụ có nhiều phím:** Hiển thị mỗi phím một dòng con, tên tác vụ chỉ ghi ở dòng đầu (ví dụ "Đi đến Cài đặt").
* **Thao tác trên hàng** (hiện khi hover hoặc focus):
  * `✎` hoặc nháy đúp: mở MOD-16 để đổi phím.
  * `+` (trong menu chuột phải): thêm một phím nữa cho tác vụ.
  * `↺`: khôi phục hàng đó về mặc định.
  * `✕`: gỡ phím. Tác vụ vẫn chạy được từ Command Palette.
* **Hàng có phạm vi Cố định 🔒:** Chỉ hiển thị để người dùng biết, không có thao tác. Đây là các phím của popup gợi ý (`↑ ↓ Enter Esc`) và `Esc` đóng modal.
* **Bàn phím:** `↑ ↓` di chuyển giữa các hàng, `Enter` mở MOD-16, `Delete` gỡ phím, `Ctrl+F` focus ô tìm kiếm.

##### 3.3. Dữ liệu Hiển thị (Data Binding)

| Cột | Nguồn | Ghi chú |
| :--- | :--- | :--- |
| Tác vụ | `KeyCommand.title` | Tooltip hiển thị `KeyCommand.id` để người dùng copy vào JSON. |
| Phím tắt | Binding hiệu lực | Render từng phím thành `<kbd>`. Hiển thị `Cmd` thay cho `Ctrl` trên macOS khi binding dùng `mod`. |
| Phạm vi | `binding.when` | Nhãn tiếng Việt lấy theo bảng 3.4. |
| Nguồn | So với mặc định | *Mặc định* / *Người dùng* / *Đã gỡ* / *Cố định*. |

##### 3.4. Phạm vi Áp dụng (`when`)

Danh sách phạm vi cố định. Phạm vi hẹp hơn được ưu tiên khi trùng phím.

| ID | Nhãn | Hoạt động khi | Độ ưu tiên |
| :--- | :--- | :--- | :--- |
| `global` | Toàn ứng dụng | Luôn luôn, trừ khi đang có modal | 1 |
| `workspace` | Workspace | Route hiện tại là `/workspace` | 2 |
| `explorer` | Cây nhóm lệnh | Focus nằm trong `GroupTree` | 3 |
| `terminal` | Terminal đang gõ | Focus nằm trong một `XtermPane` | 3 |
| `files` | Trình quản lý file | Focus nằm trong một Files pane (SCR-08), không tính ô đổi tên / ô đường dẫn đang sửa | 3 |
| `suggest` | Terminal có gợi ý | Focus ở terminal **và** đang có popup hoặc ghost text | 4 (Cố định) |
| `modal` | Modal | Đang có modal hoặc dialog mở | 5 (Cố định) |

* Khi đang có modal, mọi binding ngoài phạm vi `modal` đều bị bỏ qua, tức là không thể tạo terminal mới khi dialog đang mở.
* Phạm vi `terminal` và `files` cũng thỏa `workspace`, vì cả hai chỉ nằm trong workspace. Vì vậy `Ctrl+N` khai báo ở `workspace` vẫn chạy khi đang gõ trong terminal, giống hành vi hiện tại.

##### 3.5. Lưu trữ & Định dạng

* **Không cần migration.** Ghi vào bảng `app_setting` có sẵn với key `keybindings`. Giá trị là JSON và **chỉ lưu phần khác mặc định**, như `keybindings.json` của VS Code. Nhờ vậy, khi phiên bản sau đổi phím mặc định, người dùng chưa tùy chỉnh phím đó sẽ tự nhận phím mới.
* **Định dạng** (cũng là định dạng Xuất/Nhập JSON):
  ```json
  [
    { "key": "ctrl+t",       "command": "workspace.newTerminal", "when": "workspace" },
    { "key": "ctrl+n",       "command": "-workspace.newTerminal" },
    { "key": "ctrl+shift+w", "command": "-workspace.closePanel" }
  ]
  ```
  * `command` có tiền tố `-` nghĩa là gỡ binding mặc định trùng `key` (và trùng `when` nếu có ghi).
  * `key` là chuỗi chữ thường, các modifier theo thứ tự cố định `mod+ctrl+shift+alt+meta+<phím>`. Alias `mod` nghĩa là `ctrl` trên Linux/Windows và `meta` trên macOS, nên không được dùng chung với `ctrl` hoặc `meta`. Mục thêm phím bắt buộc có `when` (`global`/`workspace`/`explorer`/`terminal`/`files`/`suggest`); mục gỡ (`-id`) có thể bỏ `when`. Chord sau này sẽ ghi dạng `"ctrl+k ctrl+s"`, cách nhau bằng dấu cách.
* **Nhập JSON:**
  * Kiểm tra schema. Bỏ qua mục có `command` không tồn tại và báo số mục bị bỏ qua.
  * Không ghi đè ngay: hiện bảng xem trước "thêm X / gỡ Y / xung đột Z", người dùng xác nhận rồi mới lưu.
* **Lưu:** Mỗi thao tác sửa, gỡ hay khôi phục được lưu ngay qua `keybindings_set` (không đi qua nút "Lưu Cài Đặt" ở header). Khi đang ở tab này, nút header ẩn đi để tránh hiểu lầm. Lưu lỗi thì hoàn tác thay đổi trên UI và hiện toast lỗi.

* **IPC backend (`src-tauri/src/keybindings.rs`):**

  | Lệnh | Vai trò |
  | :--- | :--- |
  | `keybindings_get()` | Trả danh sách override đã chuẩn hóa. Giá trị lưu bị hỏng (backup cũ, sửa DB tay) thì chỉ trả các mục còn hợp lệ, không báo lỗi. |
  | `keybindings_set(entries)` | Ghi đè toàn bộ danh sách. Kiểm tra chặt: một mục sai thì từ chối cả lần lưu và báo `Mục #n: ...`. Trả về danh sách đã chuẩn hóa. |
  | `keybindings_reset(confirmed)` | Xóa toàn bộ tùy chỉnh. |
  | `keybindings_parse(json)` | Kiểm tra file nhập mà không lưu. Trả `{ entries, skipped: [{ index, reason }] }` để UI dựng bảng xem trước. |
  | `keybindings_export(dest?)` | Ghi file JSON và trả đường dẫn. Mặc định ghi vào thư mục dữ liệu app, giống `backup_export`. |

  * `settings_set` từ chối key `keybindings`, để mọi lần ghi phải qua bước kiểm tra.
  * Backend kiểm tra cú pháp (phím, modifier, phạm vi, định dạng ID) và các quy tắc ở mục 3.6. ID có tồn tại hay không là việc của registry ở frontend.
  * Tối đa 500 mục. Mục trùng hoàn toàn chỉ giữ lần xuất hiện cuối.

##### 3.6. Quy tắc Nhận & Chuẩn hóa Phím

1. **Không bao giờ bắt phím khi đang gõ IME** (ibus-unikey, xem các lỗi IME đã gặp):
   * Bỏ qua nếu `event.isComposing`, `event.key === 'Process'` hoặc `keyCode === 229`.
   * Không đổi giá trị textarea của xterm.
2. **Dùng `event.code`, không dùng `event.key`, để xác định phím chính:**
   * Không phụ thuộc layout bàn phím và Shift. Ví dụ code hiện tại phải viết `e.key === '0' || e.key === ')'`.
   * `KeyN` → `n`, `Digit0` → `0`, `Backslash` → `\`, `Space` → `space`.
   * Nhãn hiển thị cũng sinh từ `code`.
3. **Ràng buộc khi gán phím:**
   * Mọi binding (ở mọi phạm vi gán được) phải có ít nhất một trong `Ctrl/Alt/Meta`, trừ `F1`–`F24`. Shift đơn lẻ không đủ, để không cướp ký tự khi gõ trong terminal hay ô lọc.
   * Không cho gán phím chỉ gồm modifier.
   * Phím bị khóa, không gán được vì app không nhận được hoặc gán sẽ làm hỏng UI: `Tab`, `Shift+Tab`, `Esc` đơn, `Ctrl+Alt+Del`, `Ctrl+Alt+F1..F12` (đổi TTY trên Linux).
4. **Cảnh báo phím dành cho shell** (vẫn cho gán nhưng hiện cảnh báo vàng trong MOD-16):
   * Phạm vi bao trùm terminal (`global`, `workspace`, `terminal`) và phím là `Ctrl+C/D/Z/L/R/A/E/U/W/K` hoặc `Ctrl+Left/Right`.
   * Nội dung: "Shell và các chương trình như vim, htop đang dùng phím này (ví dụ `^C` = dừng lệnh). Gán sẽ khiến terminal không nhận được phím đó nữa."
5. **Xung đột:**
   * Hai binding hiệu lực cùng `key` và phạm vi chồng nhau (cùng phạm vi, hoặc một bên là `global`) thì cả hai hàng hiện icon ⚠.
   * Khi chạy: phạm vi ưu tiên cao hơn thắng. Nếu cùng phạm vi thì binding Người dùng thắng Mặc định, và binding Người dùng sau thắng binding trước.

##### 3.7. Kiến trúc Triển khai (Ràng buộc Kỹ thuật)

* **Registry `src/keybindings/registry.ts`:**
  * Khai báo `KeyCommand { id, title, category, defaults: { key, when }[], locked?: boolean }`.
  * Đây là nguồn sự thật duy nhất. Bảng 7.2 sinh ra từ đây, hoặc ít nhất phải khớp với đây.
* **Đăng ký handler:** Mỗi component đăng ký handler cho tác vụ của mình khi mount và hủy khi unmount, qua `useKeyCommand('workspace.splitRight', handler)`. Tác vụ không có handler đang sống (ví dụ đang không ở workspace) thì không khớp, phím được trả về luồng bình thường.
* **Một listener duy nhất:**
  * Gắn `window.addEventListener('keydown', ..., { capture: true })`.
  * Thay cho các `handleGlobalKeydown` trong `AppShell.vue` và `DockHost.vue`, và phần `Ctrl+F` trong `GroupTree.vue`.
* **Terminal:** `attachCustomKeyEventHandler` của `XtermPane` hỏi dispatcher "phím này có khớp binding nào không".
  * Có khớp: `return false` để xterm không gửi phím vào PTY. Thay cho đoạn chặn `Ctrl+N` viết cứng hiện tại.
  * Đồng thời sửa lỗi hiện có: `Ctrl+Alt+R` vừa chia panel vừa gửi `ESC ^R` vào shell.
* **Nhãn gợi ý động:**
  * Thay mọi chuỗi cứng `(Ctrl+Alt+R)` trong `title=` và `<kbd>` bằng `keyLabel('workspace.splitRight')`.
  * Áp dụng cho TilingPanel, DockHost, SessionsDrawer, CommandPalette, Titlebar và GroupTree.
  * Tác vụ không có phím thì không hiện phần ngoặc.
* **Command Palette:** Liệt kê mọi `KeyCommand` có handler đang sống, hiển thị phím bên phải. Nhờ vậy, tác vụ bị gỡ phím vẫn chạy được.
* **Tạm dừng dispatcher khi đang ghi phím** (MOD-16 và nút `⌨ Ghi phím`), để `Ctrl+N` không mở terminal trong lúc người dùng đang gán phím.

##### 3.8. Danh sách Tác vụ Khởi đầu (Lấy từ Code Hiện Tại)

| ID | Tác vụ | Mặc định | Phạm vi |
| :--- | :--- | :--- | :--- |
| `app.commandPalette` | Mở / đóng Command Palette | `Ctrl+K` | global |
| `app.quit` | Thoát ứng dụng (Graceful Shutdown) | `Ctrl+Q` | global |
| `app.toggleDevtools` | Mở / đóng Inspect. Chỉ có trong bản dev: không xuất hiện trong registry của bản build, và lệnh `app_toggle_devtools` báo lỗi ở bản release | `Ctrl+Shift+I` | global |
| `nav.workspace` | Đi đến Workspace | `Ctrl+1` | global |
| `nav.commands` | Đi đến Thư viện lệnh | `Ctrl+2` | global |
| `nav.groups` | Đi đến Nhóm lệnh | `Ctrl+3` | global |
| `nav.history` | Đi đến Lịch sử | `Ctrl+4` | global |
| `nav.settings` | Đi đến Cài đặt | `Ctrl+5`, `Ctrl+Shift+S` | global |
| `nav.templates` | Đi đến Mẫu lệnh | `Ctrl+6` | global |
| `nav.keybindings` | Mở trang Phím tắt | `Ctrl+Shift+K` | global |
| `nav.servers` | Đi đến Máy chủ SSH (SCR-07) — *mới* | `Ctrl+7` | global |
| `ssh.connect` | Mở Command Palette lọc sẵn danh sách host SSH — *mới* | `Ctrl+Shift+H` | global |
| `transfers.toggle` | Mở / đóng ngăn Transfers — *mới* | `Ctrl+Shift+J` | global |
| `files.open` | Mở Files pane theo ngữ cảnh: terminal SSH → host đó tại cwd; terminal local → local tại cwd; còn lại → home local — *mới* | `Ctrl+Shift+E` | workspace |
| `explorer.focusFilter` | Focus ô lọc nhóm & lệnh | `Ctrl+F` | explorer |
| `workspace.newTerminal` | Mở terminal mới (cùng cwd) | `Ctrl+N` | workspace |
| `workspace.newWindow` | Mở Cửa Sổ workspace mới | `Ctrl+Shift+T` | workspace |
| `workspace.splitRight` | Chia đôi panel sang phải | `Ctrl+Alt+R` | workspace |
| `workspace.splitDown` | Chia đôi panel xuống dưới | `Ctrl+Alt+D` | workspace |
| `workspace.balanceSplits` | Cân bằng tỉ lệ chia 50:50 | `Ctrl+Alt+0` | workspace |
| `workspace.toggleZoom` | Phóng to / khôi phục panel | `Ctrl+Shift+Z` | workspace |
| `workspace.toggleSync` | Bật / tắt phát phím đồng bộ | `Ctrl+Alt+S` | workspace |
| `workspace.closePanel` | Đóng panel đang chọn | `Ctrl+Shift+W` | workspace |
| `terminal.toggleSuggest` | Bật / tắt popup gợi ý | `Ctrl+Space` | terminal |
| `terminal.copy` | Sao chép vùng chọn (không có vùng chọn thì không làm gì, phím không lọt vào shell) | `Ctrl+Shift+C` | terminal |
| `terminal.paste` | Dán clipboard vào terminal (đọc qua `clipboard_read_text` ở Rust, có bracketed paste) | `Ctrl+Shift+V` | terminal |
| `terminal.find` | Mở thanh tìm kiếm của terminal (`@xterm/addon-search`): Enter / Shift+Enter đi tới kết quả sau / trước, Esc đóng, có phân biệt hoa thường và regex | `Ctrl+Shift+F` | terminal |
| `terminal.kill` | Mở MOD-01 để chọn Dừng mềm (SIGTERM) hoặc Cưỡng bức (SIGKILL) cho terminal đang focus | `Ctrl+Shift+X` | terminal |
| `files.rename` | Đổi tên mục đang chọn — *mới* | `F2` | files |
| `files.refresh` | Làm mới danh sách — *mới* | `F5` | files |
| `files.newFolder` | Tạo thư mục — *mới* | `Ctrl+Shift+N` | files |
| `files.toggleHidden` | Hiện / ẩn file ẩn — *mới* | `Ctrl+H` | files |
| `files.selectAll` | Chọn tất cả — *mới* | `Ctrl+A` | files |
| `files.copy` / `files.paste` | Sao chép mục đang chọn / dán vào thư mục đang mở (bộ nhớ tạm nội bộ, tạo job truyền) — *mới* | `Ctrl+C` / `Ctrl+V` | files |
| `files.open` / `files.up` / `files.delete` | Mở mục / lên thư mục cha / xóa (qua MOD-13) — *mới* | `Enter` / `Backspace` / `Delete` | files (🔒) |
| `terminal.acceptSuggestWord` | Nhận một từ của gợi ý mờ | `Ctrl+→` | suggest |
| `terminal.acceptSuggest` | Nhận toàn bộ gợi ý mờ | `End`, `→` | suggest (🔒) |
| `suggest.navigate` / `suggest.accept` / `suggest.close` | Di chuyển, chọn, đóng popup gợi ý | `↑ ↓` / `Enter` / `Esc` | suggest (🔒) |

* `nav.keybindings` là phím mới, mở thẳng `/settings?tab=keybindings`.
* Các dòng *mới* thuộc SCR-07/SCR-08, chỉ thêm vào registry khi có handler. `Enter`/`Backspace`/`Delete` trong Files pane là phím cố định (🔒) vì không có modifier (mục 3.6). Backend phải thêm `files` vào `SCOPES` trong `src-tauri/src/keybindings.rs`.
* Menu chuột phải mặc định của webview bị chặn trên toàn ứng dụng, ở cả bản dev. Sao chép, dán và Inspect dùng các phím tắt ở trên.
* Bảng 7.2 hiện ghi một số phím không có trong code: `Ctrl+\`, `Ctrl+Alt+←/→`, `Ctrl+Alt+M`, `Ctrl+W`, `Ctrl+R`, `Ctrl+Shift+C/V`. Bảng này cần được viết lại theo registry khi triển khai. Phím nào chưa có handler thì không đưa vào registry.

##### 3.9. Trạng thái Giao diện (UI States)
* **Không có kết quả tìm:** "Không có tác vụ nào khớp `<từ khóa>`". Kèm nút *Xóa bộ lọc*.
* **Có xung đột:** Badge `⚠ n` trên chip lọc Xung đột. Tab "⌨ Phím Tắt" ở thanh tab Settings cũng có chấm vàng.
* **Đang ghi phím:** Ô tìm kiếm viền màu accent, placeholder "Nhấn tổ hợp phím... (Esc để dừng)".
* **Lưu thất bại:** Hàng quay về giá trị cũ, kèm toast lỗi theo `.settings-toast.error`.
* **Khôi phục toàn bộ:** Mở `ConfirmDialog` (MOD-13) với nội dung "Xóa n tùy chỉnh phím tắt và quay về mặc định?".

---

### Màn hình 6: SCR-06 — Quản lý Thư viện Mẫu Lệnh (Command Templates)

#### 1. Mục đích & Vai trò
Quản lý các mẫu lệnh tái sử dụng chứa các tham số linh hoạt theo dạng placeholder (ví dụ: `ffmpeg -i {{input}} -crf {{crf}} {{output}}`). Cho phép xem danh sách template, quản lý các định nghĩa tham số, lưu lại các bộ giá trị thường dùng (Presets), xem trước câu lệnh đã render và kích hoạt thực thi nhanh trong tab PTY Terminal mà không cần nhập lại lệnh thủ công hay tiếp xúc trực tiếp với chuỗi dòng lệnh phức tạp.

#### 2. Wireframe Chi tiết

```
+----------------------------------------------------------------------------------------------------+
| [=] Command Templates    [🔍 Tìm kiếm template theo tên, placeholder...]  [Filter: All / Shell / Argv]|
|                          [+ Tạo Template Mới]                                                      |
+----------------------------------------------------------------------------------------------------+
| TÊN TEMPLATE      | KIỂU | CẤU TRÚC MẪU (TEMPLATE STRING)          | THAM SỐ (PARAMS)| LẦN CHẠY CUỐI | HÀNH ĐỘNG   |
|-------------------+------+-----------------------------------------+-----------------+---------------+-------------|
| FFmpeg Video Conv | Shell| ffmpeg -i {{input}} -crf {{crf}} {{out}} | 3 (1 secret)    | 10 phút trước | [▷] [✎] [⧉] [🗑] |
| Curl API Request  | Argv | curl -X {{method}} {{url}} -H {{auth}}  | 3 (1 secret)    | Hôm qua       | [▷] [✎] [⧉] [🗑] |
| Docker Run        | Shell| docker run -d -p {{host_port}}:{{port}} | 3 params        | 3 ngày trước  | [▷] [✎] [⧉] [🗑] |
| Postgres DB Dump  | Argv | pg_dump -h {{host}} -U {{user}} {{db}}  | 3 (1 secret)    | Chưa chạy     | [▷] [✎] [⧉] [🗑] |
+----------------------------------------------------------------------------------------------------+
| Tổng cộng: 4 templates | 2 Shell templates | 2 Direct Argv templates            | Trang: [<] 1 [>]    |
+----------------------------------------------------------------------------------------------------+
```

#### 3. Các Thành phần Giao diện & Data Binding
* **Header & Telemetry Ribbon:**
  * Huy hiệu `v2.2 Templates Engine`.
  * Nút `+ Tạo Template Mới`: Mở modal `TemplateEditorModal` (MOD-11) với mẫu trống.
  * 4 Thẻ Quick Stats Telemetry Ribbon:
    * `TỔNG TEMPLATE`: Đếm tổng số bản ghi trong `command_template`.
    * `SHELL TEMPLATES`: Số lượng template dùng môi trường Shell wrapper.
    * `DIRECT ARGV TEMPLATES`: Số lượng template dùng phân tách mảng `argv` trực tiếp.
    * `ĐÃ LƯU PRESETS`: Tổng số bộ tham số preset sẵn có từ `template_preset`.
* **Thanh Tìm Kiếm & Lọc:**
  * Khung nhập từ khóa tìm kiếm theo tên template hoặc cú pháp placeholder.
  * Filter Dropdown: `Tất cả` / `Direct Argv` / `Shell Execution`.
* **Bảng Danh Mục Mẫu Lệnh (Templates Table):**
  * **Tên Template (`name`):** Tên gợi nhớ và mô tả vắn tắt.
  * **Kiểu thực thi (`is_shell`):** Badge `Direct Argv` (Xanh lá) hoặc `Shell` (Vàng).
  * **Cấu trúc Mẫu (`template_string`):** Chuỗi mẫu monospaced highlight các biến dạng `{{param_name}}`.
  * **Danh sách Tham số (`params`):** Badge hiển thị số lượng tham số khai báo kèm số lượng secret (ví dụ: `3 params (1 secret)`).
  * **Lần chạy gần nhất:** Thời gian tương đối tính từ `run_session` có `template_id` tương ứng.
  * **Hành động (Action Buttons):**
    * `[▷]` **Chạy Template:** Mở modal `TemplateRunModal` (MOD-10) để điền params, chọn preset và thực thi.
    * `[✎]` **Chỉnh sửa Template:** Mở modal `TemplateEditorModal` (MOD-11) để sửa mẫu lệnh và danh sách `template_param`.
    * `[⧉]` **Nhân bản (Duplicate):** Tạo bản sao template kèm toàn bộ khai báo tham số.
    * `[🗑]` **Xóa:** Xóa template (CASCADE xóa các param và preset liên quan sau khi người dùng xác nhận).

#### 4. Các Trạng thái Giao diện (UI States)
* **Empty State:** Chưa tạo template nào: Hiển thị hình minh họa mẫu lệnh, thông điệp *"Chưa có Template nào. Tạo mẫu lệnh đầu tiên để tự động hóa công việc lặp đi lặp lại với tham số động"*.
* **Validation Warning State:** Khi template string chứa placeholder chưa được khai báo loại param trong bảng: Hiển thị badge cảnh báo màu vàng `⚠️ 1 placeholder chưa khai báo` cạnh tên template.


---

### Màn hình 7: SCR-07 — Quản lý Máy chủ SSH (SSH Hosts)

#### 1. Mục đích & Vai trò
Lưu danh sách máy chủ SSH (host profile) để mở terminal từ xa và duyệt file trên server chỉ bằng một cú bấm. Route `/servers`, biểu tượng 🌐 trên Activity Bar, nằm sau 🧩 Templates. Terminal SSH chạy trong Workspace (SCR-01) như mọi terminal khác, nên hưởng sẵn tiling, đổi tên, tìm kiếm, copy/paste và reattach buffer.
* **Stitch Screen Reference:** Screen ID `aa57f380bdf34671ad2bc10f3036aede` tại Project `14914224436748087443`.

#### 2. Quyết định Kỹ thuật (chốt trước khi triển khai)

| Vấn đề | Lựa chọn | Lý do |
| :--- | :--- | :--- |
| Thư viện SSH | **`russh` + `russh-sftp`** (Rust thuần, chạy trên tokio sẵn có của Tauri) | Terminal và SFTP dùng **chung một kết nối** nên chỉ xác thực một lần; chạy giống nhau trên Linux/Windows/macOS; không phụ thuộc OpenSSH cài trên máy hay libssh2/OpenSSL. |
| Thoát hiểm cho cấu hình lạ | Cờ profile **"Dùng OpenSSH hệ thống"**: chạy `ssh <alias>` trong PTY local như một terminal thường | Bao được `ProxyCommand`, `Match`, thẻ bảo mật FIDO… mà parser riêng không đọc. Đổi lại profile này **không có Files pane** (ẩn nút 📁). |
| Mật khẩu & passphrase | Người dùng chọn qua checkbox **"Lưu mật khẩu"** và nơi lưu: **OS keyring** (crate `keyring`: Secret Service / Windows Credential Manager / macOS Keychain, khuyến nghị) hoặc **SQLite của app** (bảng `ssh_secret`, mã hóa AES-256-GCM). Không chọn lưu thì hỏi lại mỗi lần kết nối. | Keyring an toàn hơn nhưng không phải máy nào cũng có (vd. Linux không chạy Secret Service); SQLite luôn dùng được. Không bao giờ ghi mật khẩu vào log hay trả về frontend. |
| Host key | TOFU: đọc `~/.ssh/known_hosts` (chỉ đọc) + `known_hosts` riêng trong `app_data_dir` (đọc/ghi) | Khóa lạ → MOD-18 hỏi. Khóa **đã đổi** → chặn cứng, không có nút "kết nối tiếp". |
| Phạm vi đọc `~/.ssh/config` | Chỉ dùng khi **Nhập**: `Host`, `HostName`, `Port`, `User`, `IdentityFile`, `ProxyJump` (1 cấp) | Profile sau khi nhập là bản sao độc lập; sửa config sau đó không tự đồng bộ. |
| Thứ tự xác thực (chế độ `Tự động`) | agent → khóa trong profile → `~/.ssh/id_ed25519`, `id_ecdsa`, `id_rsa` → keyboard-interactive / mật khẩu | Giống OpenSSH. Agent: Unix socket `SSH_AUTH_SOCK`; Windows named pipe `\\.\pipe\openssh-ssh-agent` (cần kiểm chứng hỗ trợ của russh khi triển khai). |

**Mô hình kết nối:**
* Một `SshConnection` cho mỗi host, đếm tham chiếu theo số channel đang mở (mỗi terminal là 1 channel `session` + `pty-req`; Files pane dùng 1 channel `sftp` chung cho cả host).
* Hết channel → giữ kết nối thêm 60 giây rồi đóng, để mở lại tab không phải xác thực lại.
* Keepalive mỗi 30 giây; 3 lần không phản hồi → trạng thái `Mất kết nối`.
* Vòng đời trạng thái: `Đang kết nối → Kiểm tra host key → Xác thực → Sẵn sàng → Mất kết nối`.

**Ghép vào hạ tầng PTY hiện có:**
* Tách trait `TerminalIo { write, resize, clone_reader }`. `PtySession` (local) và `SshChannelIo` (remote) cùng triển khai trait này.
* Nhờ đó ring buffer, backpressure, `pty_reattach`, `pty_write`, `pty_resize` dùng lại nguyên vẹn. `run_event_id` của terminal SSH có tiền tố `ssh:<uuid>`.
* Không tiêm shell integration lên server. Nếu shell phía server tự phát OSC 7 hoặc OSC 633 `P;Cwd` thì app vẫn đọc để biết cwd; không có thì cwd = `null`.
* Không ghi lịch sử lệnh của terminal SSH và tắt gợi ý (đúng nguyên tắc plan.md: không thấy marker thì không đoán).

#### 3. Wireframe Chi tiết

```
+----------------------------------------------------------------------------------------------------+
| 🌐 Máy chủ SSH                    [🔍 Lọc tên / host / tag...]   [⇩ Nhập ~/.ssh/config] [+ Thêm]   |
+----------------------------------------------------------------------------------------------------+
| ▌ TÊN               | ĐỊA CHỈ                       | XÁC THỰC   | TAG          | LẦN CUỐI   |        |
|----------------------------------------------------------------------------------------------------|
| ▌ prod-api          | deploy@10.0.4.12:22           | 🔑 Khóa    | prod         | 2 giờ trước| [⌨][📁][⋯] |
| ▌ prod-db  ● 2 tab  | postgres@10.0.4.20:22  ⤳ bastion | 🔐 Agent | prod, db     | Đang mở    | [⌨][📁][⋯] |
| ▌ bastion           | emil@bastion.example.com:2222 | 🔐 Agent   | prod         | Hôm qua    | [⌨][📁][⋯] |
| ▌ raspberry         | pi@192.168.1.40:22            | •••• Mật khẩu | home     | 5 ngày     | [⌨][📁][⋯] |
| ▌ legacy-box  ⚙     | legacy (OpenSSH hệ thống)     | —          | old          | Chưa       | [⌨]    [⋯] |
+----------------------------------------------------------------------------------------------------+
| 5 máy chủ · 1 đang kết nối (2 terminal, 1 SFTP) · known_hosts: 14 khóa                              |
+----------------------------------------------------------------------------------------------------+
```

* `▌` đầu hàng là **màu nhận diện** của host. Màu này lặp lại ở viền trái tab và header pane, để không gõ nhầm vào máy prod.
* `● 2 tab`: số terminal SSH đang mở tới host. `⤳ bastion`: đi qua jump host. `⚙`: profile dùng OpenSSH hệ thống.
* `[⌨]` mở terminal SSH vào khung đang focus của Workspace (chuyển sang `/workspace`). `[📁]` mở Files pane của host ở thư mục mặc định.
* `[⋯]` (menu của app, không phải menu chuột phải của webview): Sửa · Nhân bản · Kiểm tra kết nối · Sao chép lệnh `ssh …` · Quên mật khẩu đã lưu · Xóa.

#### 4. Quy tắc Tương tác
1. **Mở nhanh khi không ở `/servers`:**
   * Command Palette (`Ctrl+K`) tìm được host theo tên/tag, có tiền tố hiển thị `🌐`. `Enter` mở terminal, `Ctrl+Enter` mở Files pane.
   * Phím `ssh.connect` (`Ctrl+Shift+H`) mở Palette đã lọc sẵn chỉ còn host.
   * Menu `+` trên tab strip của panel có thêm mục "Terminal SSH…" và "Trình quản lý file".
2. **Nháy đúp hàng** = `[⌨]`. `Enter` trên hàng đang chọn cũng vậy.
3. **Xóa host:**
   * Dùng MOD-13 (ConfirmDialog). Nội dung nêu số tab đang mở; xóa sẽ ngắt các tab đó.
   * Xóa luôn mật khẩu đã lưu (keyring hoặc `ssh_secret`), **giữ** khóa trong `known_hosts` (khóa thuộc về máy, không thuộc profile).
4. **Nhập `~/.ssh/config`:**
   * Hiện danh sách `Host` đọc được, kèm ô chọn. Alias có ký tự đại diện (`*`, `?`) bị bỏ qua.
   * Alias đã có trong danh sách bị đánh dấu "Đã có" và mặc định không chọn.
   * Chỉ thị không hỗ trợ (`ProxyCommand`, `Match`, `LocalForward`…) hiện cảnh báo vàng ở từng hàng, kèm gợi ý bật "Dùng OpenSSH hệ thống".
5. **Kiểm tra kết nối:** chạy đủ các bước (host key → xác thực → mở SFTP) và hiện kết quả từng bước ✓/✗ ngay dưới hàng. Không mở terminal.
6. **Sao lưu (SCR-05):**
   * Bảng `ssh_host` và `ssh_secret` đi theo bản sao lưu, nhưng `ssh_secret` chỉ là bản mã. Khóa giải mã `secret.key` **không** nằm trong bản sao lưu.
   * Khôi phục trên cùng máy: mật khẩu dùng được ngay. Khôi phục sang máy khác (hoặc đã mất `secret.key`): không giải mã được → coi như chưa lưu, MOD-19 hỏi lại kèm ghi chú "Mật khẩu đã lưu không đọc được trên máy này".
   * Mật khẩu trong keyring không đi theo bản sao lưu.

#### 5. Terminal SSH trong Workspace (bổ sung SCR-01)

```
| ▌[●] prod-api · deploy@10.0.4.12 · ~/app  [📁] [◫][⬒][🗖][✕] |   <- header pane: màu host + user@host + cwd (nếu server báo)
|--------------------------------------------------------------|
| deploy@prod-api:~/app$ systemctl status api                  |
| ● api.service - API                                          |
|                                                              |
| ┌──────────────────────────────────────────────────────────┐ |
| │ ⚠ Mất kết nối tới prod-api (14:22:08) · Thử lại sau 4s   │ |   <- banner phủ đáy terminal, không xóa output cũ
| │                      [Kết nối lại ngay]  [Đóng tab]      │ |
| └──────────────────────────────────────────────────────────┘ |
```

* **Tab:** biểu tượng 🌐 thay cho ⚡, viền trái mang màu host, tooltip `deploy@10.0.4.12:22 · ed25519 SHA256:…`.
* **Trong lúc kết nối:** terminal in dòng mờ `Đang kết nối deploy@10.0.4.12…`, `Xác thực bằng khóa ~/.ssh/id_ed25519…`. Hỏi mật khẩu/OTP thì dùng MOD-19, không gõ vào terminal.
* **Mất kết nối:**
  * Tự thử lại 3 lần (2s, 4s, 8s), sau đó dừng và chờ người dùng bấm.
  * Kết nối lại mở **shell mới**: chương trình chạy dở trên server không quay lại được.
  * Output cũ được giữ, chèn dòng ngăn cách `──── kết nối lại 14:23:10 ────`.
* **Dừng (`terminal.kill`, MOD-01)** đổi nhãn cho phù hợp:
  * **Đóng phiên:** gửi EOF rồi đóng channel; shell ở xa thoát bình thường.
  * **Ngắt ngay:** đóng channel lập tức.
  * Cả hai **không** ảnh hưởng các tab khác dùng chung kết nối.
* **Đóng tab ≠ đóng phiên:** giữ đúng nguyên tắc 1.1.3. Channel vẫn sống và buffer vẫn nhận dữ liệu; mở lại từ Palette/SCR-07 thì reattach.
* **Thoát app:** MOD-09 đóng mọi channel và kết nối SSH trong cùng `shutdown_timeout_secs`.
* **Nút `[📁]` trên header:** mở Files pane của cùng host tại cwd hiện tại. Không biết cwd thì mở thư mục mặc định của profile, không có nữa thì mở home.

#### 5b. Keychain (kiểu Termius)

Tab **Keychain** trong `/servers` (`/servers?tab=keychain`) giữ các thông tin đăng nhập dùng lại cho nhiều máy chủ:

* **Khóa:** dán khóa OpenSSH / PEM / PuTTY `.ppk`, chọn tệp, hoặc sinh khóa ed25519 mới.
  * Nội dung khóa (và passphrase nếu nhập) lưu trong Keyring hoặc SQLite mã hóa, bắt buộc phải lưu.
  * Khóa `.ppk` được chuyển sang OpenSSH khi nhập.
  * Danh sách hiện thuật toán, fingerprint và số máy chủ / identity đang dùng. Có nút chép public key, đổi tên, xóa.
  * Sinh khóa xong, modal hiện public key để chép vào `~/.ssh/authorized_keys`.
* **Identity:** tên đăng nhập + mật khẩu (tùy chọn lưu) + khóa (tùy chọn). Đổi mật khẩu ở identity là mọi máy chủ dùng nó cùng đổi.
* **Trong MOD-17:**
  * Chọn **Identity** thì ô tên đăng nhập và mật khẩu của host bị ẩn / khóa.
  * Mục khóa có ba nguồn: **Từ Keychain** | **Tệp khóa** | **Dán nội dung khóa**.
  * Nút `+` cạnh hai ô chọn mở modal tạo khóa / identity và tự chọn mục vừa tạo.
* **Thứ tự gộp khi kết nối:**
  * Tên đăng nhập và mật khẩu lấy từ identity nếu có, không thì từ host.
  * Khóa lấy theo thứ tự: khóa Keychain của host → khóa của identity → nội dung khóa dán trên host → `key_path` → `~/.ssh/id_*`.
  * Passphrase đi theo khóa Keychain; với các nguồn khác thì đi theo host.
  * Mật khẩu / passphrase nhập ở MOD-19 được lưu về đúng chủ sở hữu (host, identity hoặc khóa).
* **Thứ tự thử khi đăng nhập:** khóa đã chọn được thử trước SSH agent. Mỗi khóa bị từ chối đều tính vào `MaxAuthTries`, nên một agent nhiều khóa có thể làm server ngắt kết nối trước khi tới khóa đúng.
* Migration `013_ssh_keychain.sql` tạo `ssh_key`, `ssh_identity`, `ssh_keychain_secret` và thêm `ssh_host.identity_id`, `ssh_host.key_id` (`ON DELETE SET NULL`).

| Lệnh IPC | Đầu vào | Kết quả / Ghi chú |
| :--- | :--- | :--- |
| `ssh_keychain_list` | — | `{ keys: [SshKey + used_by_hosts, used_by_identities], identities: [SshIdentity + used_by_hosts] }` |
| `ssh_key_import` | `name`, `private_key`, `passphrase?`, `store: 'keyring' \| 'sqlite'` | Kiểm tra khóa (và passphrase nếu có) trước khi lưu |
| `ssh_key_generate` | `name`, `store` | Khóa ed25519 mới, trả kèm `public_key` |
| `ssh_key_rename` / `ssh_key_delete` | `id`, `name` / `id`, `confirmed` | Xóa khóa thì host / identity dùng nó mất liên kết |
| `ssh_identity_create` / `ssh_identity_update` | `identity: { name, username, key_id? }`, `password?: SecretChange`, `store` | `store = 'none'` không lưu mật khẩu |
| `ssh_identity_delete` | `id`, `confirmed` | — |

#### 6. Dữ liệu & IPC

**Migration `011_ssh_hosts.sql`:**

```sql
CREATE TABLE ssh_host (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  host TEXT NOT NULL,
  port INTEGER NOT NULL DEFAULT 22 CHECK (port BETWEEN 1 AND 65535),
  username TEXT NOT NULL,
  auth_method TEXT NOT NULL DEFAULT 'auto' CHECK (auth_method IN ('auto', 'agent', 'key', 'password')),
  key_path TEXT,
  jump_host_id TEXT REFERENCES ssh_host(id) ON DELETE SET NULL,
  use_system_ssh INTEGER NOT NULL DEFAULT 0 CHECK (use_system_ssh IN (0, 1)),
  agent_forwarding INTEGER NOT NULL DEFAULT 0 CHECK (agent_forwarding IN (0, 1)),
  default_dir TEXT,
  secret_store TEXT NOT NULL DEFAULT 'none' CHECK (secret_store IN ('none', 'keyring', 'sqlite')),
  color TEXT,
  tags TEXT NOT NULL DEFAULT '',          -- phân tách bằng dấu phẩy
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  last_connected_at TEXT
);
```

**Lưu mật khẩu trong SQLite** (cùng migration 011):

```sql
CREATE TABLE ssh_secret (
  host_id TEXT NOT NULL REFERENCES ssh_host(id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK (kind IN ('password', 'passphrase')),
  nonce BLOB NOT NULL,                    -- 12 byte ngẫu nhiên, mới cho mỗi lần ghi
  ciphertext BLOB NOT NULL,               -- AES-256-GCM, AAD = host_id || kind
  updated_at TEXT NOT NULL,
  PRIMARY KEY (host_id, kind)
);
```

* Khóa mã hóa: 32 byte ngẫu nhiên trong `<app_data_dir>/secret.key`, tạo ở lần lưu đầu tiên, quyền `0600` (Windows: ACL chỉ user hiện tại). File này không nằm trong DB nên không theo bản sao lưu.
* Mức bảo vệ cần nói rõ: chống lộ khi **chỉ** file DB hoặc bản sao lưu bị lấy đi. Ai đọc được cả thư mục dữ liệu app của user trên máy này vẫn giải mã được. Đó là lý do keyring vẫn là lựa chọn khuyến nghị.
* AAD gắn `host_id` để không chép bản mã của host này sang host khác được.

**Lưu trong keyring:** service `command-manager`, account `ssh:<host_id>:password` hoặc `ssh:<host_id>:passphrase`.

Cột `ssh_host.secret_store TEXT NOT NULL DEFAULT 'none' CHECK (secret_store IN ('none', 'keyring', 'sqlite'))` ghi nơi đang lưu. Đổi nơi lưu thì chuyển mật khẩu sang nơi mới rồi xóa ở nơi cũ, trong cùng một lệnh.

| Lệnh IPC | Đầu vào | Kết quả / Ghi chú |
| :--- | :--- | :--- |
| `ssh_hosts_list` | — | `SshHost[]` kèm `open_channels` (số channel đang mở trên kết nối dùng chung) |
| `ssh_hosts_create` | `host: SshHostInput`, `secrets?: SecretsInput` | Trả `SshHost`. Lưu mật khẩu lỗi thì không tạo host. |
| `ssh_hosts_update` | `id`, `host`, `secrets?` | Trả `SshHost`. Bỏ `secrets` = không đụng tới mật khẩu đã lưu. |
| `ssh_hosts_delete` | `id`, `confirmed` | Dừng các terminal SSH của host, ngắt kết nối, xóa mật khẩu đã lưu (keyring và `ssh_secret`) |
| `ssh_secret_status` | — | `{ keyring_available }` để UI bật/tắt lựa chọn keyring |
| `ssh_config_scan` | — | `{ path, hosts: ConfigHost[] }`; mỗi host có `unsupported[]` để hiện cảnh báo |
| `ssh_test_connection` | `host: SshHostInput`, `host_id?`, `password?`, `passphrase?`, `private_key?` | Chạy với giá trị đang nhập (chưa lưu), xong thì ngắt. Trả `{ ok, failed_stage: 'network' \| 'host_key' \| 'auth' \| null, error }` |
| `ssh_terminal_open` | `host_id`, `cols?`, `rows?` | `TerminalInfo` (giống `terminal_open`, `pid = 0`, `shell_kind = 'ssh'`). Host "OpenSSH hệ thống" chạy `ssh` trong PTY local. |
| `ssh_terminal_reconnect` | `run_event_id` | Mở shell mới cho tab đang `disconnected`; output cũ giữ nguyên, có dòng ngăn cách |
| `ssh_disconnect` | `host_id` | Đóng kết nối ngay |
| `ssh_forget_secret` | `host_id` | Xóa mật khẩu/passphrase đã lưu |
| `ssh_forget_host_key` | `host_id` | Xóa khóa của host trong `known_hosts` của app, trả số dòng đã xóa |
| `ssh_hostkey_respond` | `request_id`, `trust`, `remember?` (mặc định `true`) | Trả lời MOD-18 |
| `ssh_auth_respond` | `request_id`, `answers[]`, `store?: 'none' \| 'keyring' \| 'sqlite'` | Trả lời MOD-19; `answers` rỗng = hủy |

`SecretsInput = { store, password?: { action: 'keep' } \| { action: 'set', value } \| { action: 'clear' }, passphrase?: (như password), private_key?: (như password) }`. Migration `012_ssh_private_key.sql` thêm `ssh_host.key_source` (`file` \| `content`) và cho phép `ssh_secret.kind = 'private_key'`. `store = 'none'` xóa mọi mật khẩu đã lưu của host.

Sự kiện:
* `ssh://status { host_id, state, error }`, `state` ∈ `connecting`, `verifying`, `authenticating`, `ready`, `lost`, `closed`.
* `ssh://hostkey-request { request_id, prompt: { host_id, label, algorithm, fingerprint, changed } }`. `changed` khác `null` nghĩa là kết nối đã bị chặn, không cần trả lời.
* `ssh://auth-request { request_id, prompt: { host_id, label, kind, name, instructions, prompts: [{ text, echo }], storable, store, error, attempt } }`.
* Hai sự kiện request chờ tối đa 5 phút, hết giờ coi như hủy.
* Terminal SSH dùng chung `process://status`; có thêm trạng thái `disconnected` khi mất kết nối (tab và output được giữ, gọi `ssh_terminal_reconnect` để mở lại).

---

### Màn hình 8: SCR-08 — Trình Quản lý File & Kéo Thả Truyền File (Files Pane + Transfers)

#### 1. Mục đích & Vai trò
Duyệt file trên máy local và trên các server SSH, rồi chuyển file giữa chúng bằng kéo thả. Files pane **không phải route riêng**: nó là một **loại tab mới** trong tiling của Workspace (SCR-01), nên đặt được cạnh terminal SSH, chia khung, kéo tab sang panel khác như terminal.

* `OpenTabItem` có thêm `kind: 'terminal' | 'files'`. Tab `files` mang `location: { kind: 'local' } | { kind: 'ssh', hostId }` và `path`.
* Layout gợi ý khi mở Files pane lần đầu cho một host: chia phải panel hiện tại thành **Local | Server**, kiểu WinSCP/FileZilla nhưng mỗi bên là một tab độc lập.
* **Stitch Screen Reference:** Screen ID `7813737d88334282b99d4ed4d24d7cd0` tại Project `14914224436748087443`.

#### 2. Wireframe Chi tiết

##### 2.1. Hai Files pane cạnh nhau, đang kéo 3 file từ local lên server

```
+-----------------------------------------------+------------------------------------------------+
| [📁 Local · ~/build] [x] [+]                  | ▌[📁 prod-api · /var/www/app] [x] [+]          |
|-----------------------------------------------+------------------------------------------------|
| [↑] [⟳] [＋📁]  ~ / build /            [👁 ẩn] | [↑] [⟳] [＋📁]  / var / www / app /    [👁 ẩn] |  <- breadcrumb, bấm để sửa đường dẫn
|-----------------------------------------------+------------------------------------------------|
| TÊN ▲               KÍCH THƯỚC   SỬA ĐỔI       | TÊN ▲           KÍCH THƯỚC  SỬA ĐỔI    QUYỀN   |
| 📁 assets                  —     14:02         | 📁 releases          —     Hôm qua   drwxr-x |
| ▓ 📄 app.js           1.2 MB     14:05 ▓       | ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓|
| ▓ 📄 app.js.map       3.4 MB     14:05 ▓       | ┃ 📁 public       —     3 ngày   drwxr-x     ┃|  <- thư mục đích được tô sáng
| ▓ 📄 index.html       2.1 KB     14:05 ▓       | ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛|
| 📄 README.md          4.0 KB     Hôm qua        | ↪ current → releases/42   —   Hôm qua  lrwxrwx |
|                                               |  ┌───────────────────────────────────────┐     |
|                                               |  │ 📄×3  Tải lên → prod-api:/var/www/app/ │     |  <- nhãn đi theo con trỏ
|                                               |  │       public/   (4.7 MB) · Shift = Di chuyển│ |
|                                               |  └───────────────────────────────────────┘     |
|-----------------------------------------------+------------------------------------------------|
| 4 mục · 3 đã chọn (4.7 MB)                    | 3 mục · sftp deploy@10.0.4.12 · ed25519        |
+-----------------------------------------------+------------------------------------------------+
| ● 2 running | ⇅ 1 đang chuyển · 38% · 3.1 MB/s   | Ring Buffer: 680KB/2MB | ...                  |  <- status bar
+-----------------------------------------------------------------------------------------------+
```

##### 2.2. Ngăn Transfers (mở từ status bar `⇅` hoặc `Ctrl+Shift+J`)

```
+-----------------------------------------------------------------------------------------------+
| ⇅ TRUYỀN FILE                                   [Xóa mục đã xong] [⏸ Tạm dừng hàng đợi] [✕]   |
|-----------------------------------------------------------------------------------------------|
| ▾ Local ~/build → prod-api:/var/www/app/public     3 tệp · 4.7 MB      ██████░░░░ 62%  [✕]   |
|     ✓ index.html                 2.1 KB                                  Xong                  |
|     ⇡ app.js.map                 3.4 MB   ███████░░░ 71%  2.8 MB/s · còn 1s          [✕]       |
|     ⇡ app.js                     1.2 MB   ██░░░░░░░░ 18%                             [✕]       |
| ▸ prod-db:/var/backups/db.sql.gz → Local ~/Downloads     812 MB  ████████████ ✓ Xong 14:01      |
| ▸ prod-api:/etc/nginx → staging:/etc/nginx  ⇄ qua máy này   12 tệp   ✗ 1 lỗi   [↻ Thử lại]     |
|     ✗ nginx.conf   Permission denied (/etc/nginx/nginx.conf)                                   |
+-----------------------------------------------------------------------------------------------+
```

* Ngăn trượt lên từ status bar, cao 240px, kéo được mép trên. `Esc` đóng.
* Mỗi lần thả là **một job**; mở job (`▾`) để thấy từng tệp. Job xong được giữ tới khi bấm "Xóa mục đã xong" hoặc tắt app (không lưu DB).
* Thông báo toast khi job xong lúc ngăn đang đóng: "Đã tải lên 3 tệp tới prod-api" hoặc "1 tệp lỗi, xem ⇅".

#### 3. Nguồn kéo & Đích thả

| Kéo từ | Thả vào | Kết quả |
| :--- | :--- | :--- |
| Files pane (local) | Files pane (server) | **Tải lên** |
| Files pane (server) | Files pane (local) | **Tải xuống** |
| Files pane (server A) | Files pane (server B) | **Chuyển server → server**, stream qua máy này (không ghi tạm ra đĩa). Nhãn kéo ghi rõ `⇄ qua máy này`. |
| Files pane (server A) | Files pane (cùng server A, thư mục khác) | Copy/di chuyển phía server bằng SFTP (`rename` khi di chuyển, copy thì stream lại qua máy này). |
| Files pane (local) | Files pane (local) | Copy/di chuyển local. |
| Trình quản lý file của OS (Nautilus, Explorer, Finder) | Files pane server / local | Tải lên / copy local. |
| OS hoặc Files pane local | **Terminal SSH** | Tải lên vào **cwd của terminal** (chỉ khi server báo cwd), xong thì chèn đường dẫn ở xa đã quote vào dòng lệnh. Không biết cwd → hỏi bằng MOD-15 (PromptDialog), điền sẵn thư mục mặc định của host. |
| Files pane server A | Terminal SSH cùng server A | **Không truyền**, chỉ chèn đường dẫn ở xa vào dòng lệnh. |
| OS hoặc Files pane local | Terminal local | Giữ hành vi hiện tại: chèn đường dẫn (`formatDroppedPath`). |
| Files pane server | Terminal local, hoặc kéo ra ngoài OS | **Chưa hỗ trợ** (con trỏ 🚫, nhãn giải thích). Kéo ra OS để Giai đoạn 3. |

**Đích thả cụ thể trong Files pane:** thả lên **hàng thư mục** = vào thư mục đó; thả lên vùng trống = thư mục đang mở. Giữ chuột trên một hàng thư mục 800ms thì mở thư mục đó (spring-loaded) để thả sâu hơn.

#### 4. Quy tắc Tương tác
1. **Cơ chế kéo:**
   * **Kéo trong app** (Files pane → pane/terminal) dùng pointer events như kéo tab (`useTabDrag`), **không** dùng HTML5 DnD. Lý do: trên Windows, khi `dragDropEnabled` của Tauri bật (cần để nhận file từ OS), HTML5 DnD trong webview không chạy.
   * **Kéo từ OS vào** dùng `getCurrentWebview().onDragDropEvent()`:
     * `over` cho vị trí con trỏ (đơn vị **physical pixel**, chia `devicePixelRatio` trước khi `document.elementFromPoint`) để tô sáng đích.
     * `drop` cho danh sách đường dẫn tuyệt đối.
     * Một listener duy nhất ở `AppShell`, phát cho pane nằm dưới con trỏ.
   * Ngưỡng bắt đầu kéo: 4px. `Esc` trong lúc kéo = hủy.
2. **Copy hay di chuyển:**
   * Mặc định **copy**. Giữ `Shift` khi thả = **di chuyển**; nhãn kéo đổi ngay "Di chuyển".
   * Di chuyển giữa hai nơi khác nhau = copy toàn bộ thành công rồi mới xóa nguồn. Có tệp lỗi thì không xóa nguồn của tệp đó.
   * Kéo từ OS luôn là copy (không xóa file của người dùng ngoài app).
3. **Thả nhiều/thư mục lớn:** job bắt đầu bằng pha "Đang quét… 1.204 tệp · 380 MB" rồi mới truyền, để thanh tiến trình có tổng dung lượng thật.
4. **Xung đột tên:** tệp đích đã tồn tại thì mở MOD-20 cho tệp đầu tiên, có lựa chọn áp dụng cho các xung đột còn lại. Các tệp không xung đột vẫn chạy tiếp trong lúc chờ.
5. **Thao tác trong Files pane:**
   * Chọn: click, `Ctrl+click`, `Shift+click`, `Ctrl+A`, kéo khung chọn từ vùng trống.
   * Mở: nháy đúp/`Enter` vào thư mục; `Backspace` hoặc `[↑]` lên thư mục cha.
   * Nháy đúp vào **tệp** local thì mở bằng ứng dụng mặc định của OS. Tệp trên server thì không làm gì ở Giai đoạn 2 (tải về & mở để sau).
   * `F2` đổi tên tại chỗ, `Ctrl+Shift+N` tạo thư mục, `F5` làm mới, `Ctrl+H` bật/tắt file ẩn.
   * `Delete` mở MOD-13. Xóa **thư mục trên server** là xóa đệ quy, không có thùng rác, nên dùng kiểu nguy hiểm (nút đỏ, phải gõ tên thư mục khi xóa > 1 thư mục).
   * `Ctrl+C` / `Ctrl+V` giữa hai Files pane = kéo thả bằng bàn phím (bộ nhớ tạm nội bộ của app, không dùng clipboard OS).
6. **Menu chuột phải:** menu gốc của webview vẫn bị chặn toàn app. Files pane vẽ **menu riêng** (cùng mục với nút `⋯` của hàng): Mở · Tải xuống tới… · Tải lên vào đây… · Đổi tên · Xóa · Sao chép đường dẫn · Mở terminal tại đây.
7. **Hiển thị:**
   * Thư mục lên trước, sắp theo tên tự nhiên (`file2` < `file10`). Bấm tiêu đề cột để đổi kiểu sắp.
   * Danh sách > 500 mục dùng cuộn ảo.
   * Symlink hiện `↪ tên → đích`. Nháy đúp vào symlink thư mục thì đi theo. Khi **truyền**, symlink không được đi theo: tệp được bỏ qua, có cảnh báo trong job.
   * Tên không phải UTF-8 trên server hiện bằng ký tự thay thế `�` và khóa đổi tên.
   * Lỗi quyền (`Permission denied`) hiện ngay trong vùng danh sách với nút "Thử lại", không mở modal.
8. **Mất kết nối giữa chừng:** Files pane hiện banner giống terminal SSH. Job đang chạy chuyển "Tạm dừng – mất kết nối", tự tiếp tục khi kết nối lại; tệp đang ghi dở thì làm lại từ đầu.

#### 5. Động cơ Truyền File (Rust)
* **Hàng đợi:** tối đa 3 tệp song song mỗi kết nối, 6 tệp toàn app. Đọc/ghi theo khối 256 KB, pipeline nhiều request SFTP để không bị giới hạn bởi độ trễ mạng.
* **Ghi an toàn:** ghi vào `<tên>.cm-part` cạnh đích, xong mới `rename` đè. Hủy hoặc lỗi → xóa `.cm-part`. Tệp đích cũ không bao giờ bị hỏng dở.
* **Giữ thuộc tính:** giữ thời gian sửa đổi. Quyền (`chmod`) chỉ giữ khi truyền giữa hai server, hoặc lên server từ local Unix. Từ Windows lên thì dùng mặc định của server.
* **Sự kiện tiến trình:** gộp và phát tối đa 4 lần/giây mỗi job, để không đè lên kênh `pty-data`.
* **Tắt app khi đang truyền:** MOD-09 liệt kê job đang chạy. Chờ thì chờ tới hết `shutdown_timeout_secs`; thoát ngay thì hủy và dọn `.cm-part`.

| Lệnh IPC | Đầu vào | Kết quả / Ghi chú |
| :--- | :--- | :--- |
| `fs_list` | `location`, `path`, `show_hidden` | `FsEntry[] { name, kind, size, mtime, mode?, link_target? }` |
| `fs_home` | `location` | Thư mục home (local hoặc trên server) |
| `fs_mkdir` / `fs_rename` | `location`, đường dẫn | — |
| `fs_delete` | `location`, `paths[]`, `confirmed` | Đệ quy với thư mục |
| `fs_open_local` | `path` | Mở bằng ứng dụng mặc định của OS |
| `transfer_start` | `sources: {location, path}[]`, `dest: {location, path}`, `mode: 'copy' \| 'move'` | `job_id` |
| `transfer_resolve` | `job_id`, `item_id`, `action`, `apply_to_rest` | Trả lời MOD-20 |
| `transfer_cancel` / `transfer_retry` | `job_id`, `item_id?` | Cả job hoặc từng tệp |
| `transfer_list` / `transfer_clear_finished` | — | — |

Sự kiện: `transfer-progress`, `transfer-conflict`, `transfer-done`.

**Bảo vệ đường dẫn:**
* Mọi đường dẫn tới IPC đều là tuyệt đối và đã chuẩn hóa ở Rust.
* Tên tệp chứa `/`, `\0` hay đúng bằng `..` bị từ chối.
* Không có lệnh nào ghép đường dẫn vào chuỗi shell; mọi thao tác đi qua SFTP hoặc `std::fs`.

#### 6. Lộ trình Triển khai

| Giai đoạn | Phạm vi | Ghi chú |
| :--- | :--- | :--- |
| **1. SSH terminal** | SCR-07, MOD-17/18/19, migration 011, trait `TerminalIo`, `ssh_terminal_open`, lưu mật khẩu (keyring / SQLite mã hóa), banner mất kết nối | Dùng được ngay, chưa có file. |
| **2. Files & kéo thả** | Tab `kind: 'files'`, SCR-08, MOD-20, ngăn Transfers, `fs_*`, `transfer_*`, kéo từ OS vào, thả vào terminal SSH | Phần chính của yêu cầu kéo thả. |
| **3. Mở rộng** | Kéo **ra** OS (`tauri-plugin-drag`, tải về thư mục tạm trước), tải về & mở tệp server, ProxyJump nhiều cấp, chạy Command/Group trên server (`command_definition.ssh_host_id`), resume tệp lớn | Đánh giá lại sau giai đoạn 2. |

---

## 4. Thiết kế Chi tiết Các Hộp Thoại & Hộp Cảnh Báo (Modals & Dialogs)

---

### MOD-02: Modal Thêm / Chỉnh Sửa Lệnh (Command Definition Editor)

* **Tiêu đề:** *"Tạo Câu Lệnh Mới"* hoặc *"Chỉnh Sửa Câu Lệnh: [Tên]"*
* **Kích thước:** 640px x 580px (Modal trung tâm có backdrop làm mờ).
* **Wireframe:**

```
+-----------------------------------------------------------------------------+
|  ⚡ Chỉnh Sửa Câu Lệnh                                                 [X]  |
+-----------------------------------------------------------------------------+
|  Tên gợi nhớ của lệnh (*)                                                   |
|  [ Vite Frontend Dev Server                                            ]    |
|                                                                             |
|  Phương thức thực thi (*)                                                   |
|  ( ) Chạy trực tiếp qua mảng đối số (Direct Argv) - Khuyến nghị an toàn     |
|      Chạy thẳng binary, không qua shell trung gian, tránh escape lỗi ký tự. |
|  (o) Chạy qua vỏ lệnh (Shell Execution: bash / sh / zsh / cmd)              |
|      Dùng khi lệnh cần nối ống (pipe |), xuất file (>), hoặc biến môi trường|
|                                                                             |
|  Chuỗi dòng lệnh (Execution String) (*)                                     |
|  +-----------------------------------------------------------------------+  |
|  | pnpm --filter web run dev --host 0.0.0.0 --port 3000                  |  |
|  |                                                                       |  |
|  +-----------------------------------------------------------------------+  |
|  [⚡ Kiểm tra cú pháp nhanh]               Môi trường giả lập: Shell Default |
|                                                                             |
|  Thư mục làm việc (Working Directory - Tùy chọn)                            |
|  [ /data/Second/htdocs/projects/command-manager                      ] [Duyệt]|
|                                                                             |
|  Gán vào Nhóm lệnh                                                          |
|  [x] Web Platform Development    [ ] Infrastructure    [ ] Microservices     |
|                                                                             |
|  ⚡ Quick Access                                                   [●━]      |
|  Hiện trong mục Quick Access ở màn Terminal để dán hoặc chạy nhanh           |
|  vào terminal đang focus.                                                   |
+-----------------------------------------------------------------------------+
|  [Hủy Bỏ]                                          [💾 Lưu Định Nghĩa Lệnh]  |
+-----------------------------------------------------------------------------+
```

* **Ràng buộc & Validate:**
  * `name`: Bắt buộc, độ dài từ 2 đến 100 ký tự, không chứa ký tự điều khiển.
  * `execution_string`: Bắt buộc, không được để trống.
  * `is_shell`: Radio button rõ ràng. Nếu người dùng chọn Shell, hiển thị cảnh báo nhỏ: *"Lưu ý: Lệnh chạy trong Shell có thể chịu ảnh hưởng bởi biến môi trường của hệ thống host"*.
  * `quick_access`: Switch toggle "⚡ Quick Access". Mặc định tắt (`false`) khi tạo mới lệnh. Khi nhân bản lệnh (Duplicate `[⧉]`), giữ nguyên giá trị của lệnh gốc. Giá trị được lưu đồng bộ cùng form qua IPC `commands_create` / `commands_update`.

---

### MOD-04: Modal Soạn Nhóm Lệnh & Thứ Tự Thực Thi (Group Composer)

* **Tiêu đề:** *"Cấu Hình Nhóm Lệnh & Trình Tự Thực Thi (Workflow Sequencer)"*
* **Wireframe:**

```
+-----------------------------------------------------------------------------+
|  📁 Cấu Hình Nhóm Lệnh: Web Platform Development                       [X]  |
+-----------------------------------------------------------------------------+
|  Tên nhóm: [ Web Platform Development                                  ]    |
|                                                                             |
|  [x] Tự động khởi động nhóm này khi mở ứng dụng (Autostart)                 |
|      Nhóm sẽ được Process Manager khởi chạy ngay sau khi giữ Single-Instance|
|                                                                             |
|  Danh sách lệnh thực thi tuần tự (Kéo thả ⠿ để đổi execution_order):       |
|  +-----------------------------------------------------------------------+  |
|  | ⠿ 1. [Argv] docker-postgres-init       (Thứ tự: 1)             [X Gỡ] |  |
|  | ⠿ 2. [Shell] pnpm prisma db push       (Thứ tự: 2)             [X Gỡ] |  |
|  | ⠿ 3. [Shell] nestjs-backend-dev        (Thứ tự: 3)             [X Gỡ] |  |
|  | ⠿ 4. [Shell] vite-frontend-dev         (Thứ tự: 4)             [X Gỡ] |  |
|  +-----------------------------------------------------------------------+  |
|  [+ Thêm lệnh từ Thư Viện vào nhóm này]                                      |
|                                                                             |
|  Chính sách khi có lệnh gặp lỗi (Exit code != 0):                           |
|  (o) Dừng toàn bộ các lệnh tiếp theo trong nhóm                             |
|  ( ) Vẫn tiếp tục thực thi các lệnh phía sau                                |
+-----------------------------------------------------------------------------+
|  [Hủy]                                                 [💾 Lưu Cấu Hình Nhóm]|
+-----------------------------------------------------------------------------+
```

---

### MOD-08: Hộp Thoại Wizard Khôi Phục Dữ Liệu & Bảo Vệ Rollback (Restore Wizard)

* **Tiêu đề:** *"Quy Trình Khôi Phục Cơ Sở Dữ Liệu SQLite"*
* **Wireframe:**

```
+-----------------------------------------------------------------------------+
|  ⚠️ CẢNH BÁO TÁC VỤ ĐẶC QUYỀN: KHÔI PHỤC CƠ SỞ DỮ LIỆU                 [X]  |
+-----------------------------------------------------------------------------+
|  Tệp nguồn: /home/emil/Downloads/backup_remote_2026.sqlite                   |
|                                                                             |
|  KẾT QUẢ KIỂM TRA TÍNH TOÀN VẸN (PRE-FLIGHT CHECK):                         |
|  [✔] Cấu trúc SQLite: Hợp lệ (PRAGMA integrity_check: ok)                   |
|  [✔] Phiên bản lược đồ (Schema Version): Tương thích v1.0.0                 |
|  [✔] Tổng số lượng: 12 commands, 3 groups, 42 history records               |
|                                                                             |
|  QUY TRÌNH AN TOÀN ĐƯỢC THỰC HIỆN TỰ ĐỘNG:                                  |
|  1. Hệ thống sẽ tạo bản backup dự phòng: app.db.bak_{timestamp}             |
|  2. Toàn bộ 2 tiến trình đang chạy ngầm sẽ được dừng an toàn (SIGTERM).     |
|  3. Kết nối SQLite hiện tại sẽ đóng lại; xóa file WAL/SHM tạm thời.         |
|  4. Thay thế file DB và mở lại kết nối.                                     |
|  5. NẾU CÓ LỖI: Tự động khôi phục hoàn nguyên file backup dự phòng.         |
|                                                                             |
|  [ ] TÔI XÁC NHẬN RẰNG TỆP BACKUP NÀY ĐẾN TỪ NGUỒN TIN CẬY VÀ ĐỒNG Ý GHI ĐÈ |
+-----------------------------------------------------------------------------+
|  [Hủy Bỏ Thao Tác]                     [🔒 BẮT ĐẦU KHÔI PHỤC VÀ TẢI LẠI APP] |
+-----------------------------------------------------------------------------+
```

---

### MOD-09: Màn Hình Chờ Dừng Duyên Dáng Khi Thoát Ứng Dụng (Graceful Shutdown Overlay)

* **Mục đích:** Khi người dùng bấm nút [X] của cửa sổ hoặc chọn Thoát ứng dụng, hiển thị overlay toàn màn hình ngăn chặn tương tác mới và báo hiệu tiến độ dừng tiến trình.
* **Wireframe:**

```
+-----------------------------------------------------------------------------+
|                                                                             |
|                           [ Command Manager ]                               |
|                                                                             |
|                     Đang dừng an toàn các tiến trình...                     |
|                                                                             |
|            Đang gửi tín hiệu SIGTERM tới 3 tiến trình con đang chạy:        |
|            - PID 2841 (vite-frontend-dev): Đã thoát (Exit 0)                |
|            - PID 2845 (nestjs-backend-api): Đang dọn dẹp kết nối...         |
|            - PID 2830 (docker-postgres): Chờ tắt...                         |
|                                                                             |
|            [============>                    ]  Thời gian chờ: 4s / 8s      |
|                                                                             |
|     (Nếu tiến trình không dừng sau 8 giây, hệ thống sẽ gửi SIGKILL)         |
|                                                                             |
|                     [ Cưỡng Chế Thoát Ngay (Force Kill) ]                   |
|                                                                             |
+-----------------------------------------------------------------------------+
```

---

### MOD-10: Modal Chạy Mẫu Lệnh & Quản lý Preset (TemplateRunModal)

* **Tiêu đề:** *"Thực Thi Mẫu Lệnh: [Tên Template]"*
* **Kích thước:** 680px x 620px (Modal trung tâm có backdrop mờ).
* **Wireframe:**

```
+-----------------------------------------------------------------------------+
|  🧩 Thực Thi Mẫu Lệnh: FFmpeg Video Converter                          [X]  |
+-----------------------------------------------------------------------------+
|  Bộ giá trị đã lưu (Preset):                                                |
|  [ 📌 Chọn Preset... ▼ ]  [💾 Lưu thành Preset mới...]  [🗑 Xóa Preset]     |
|-----------------------------------------------------------------------------|
|  ĐIỀN THAM SỐ THỰC THI (PARAMETERS FORM):                                  |
|                                                                             |
|  1. File Video Đầu Vào (input) (*) [Path]                                   |
|     [ /home/user/Videos/input_sample.mp4                           ] [Duyệt]|
|                                                                             |
|  2. Mức Nén CRF (crf) [Number: 0 - 51]                                      |
|     [ 23                                                           ]        |
|     (Default: 23 | Mức CRF tiêu chuẩn cho H.264: 18 - 28)                      |
|                                                                             |
|  3. Mật Khẩu Giải Mã (secret_key) 🔒 [Secret - Không lưu Preset/History]     |
|     [ ••••••••••••••••••                                           ] [👁️]   |
|                                                                             |
|  4. File Đầu Ra (output) (*) [String]                                       |
|     [ /home/user/Videos/output_compressed.mp4                     ]        |
|-----------------------------------------------------------------------------|
|  ⚡ XEM TRƯỚC LỆNH SẼ CHẠY (RENDERED PREVIEW - Rust Generated):               |
|  +-----------------------------------------------------------------------+  |
|  | ffmpeg -i "/home/user/Videos/input_sample.mp4" -crf 23 \              |  |
|  |        -key "******" "/home/user/Videos/output_compressed.mp4"        |  |
|  +-----------------------------------------------------------------------+  |
|  [✔] An toàn chống Shell Injection (Values are shell-quoted / argv isolated)|
+-----------------------------------------------------------------------------+
|  [Hủy Bỏ]                                  [▷ Thực Thi Lệnh (Enter)]        |
+-----------------------------------------------------------------------------+
```

* **Quy tắc Nghiệp vụ & Ràng buộc UI:**
  * **Form Sinh Động:** Đơn giản hóa từ `template_param` (kiểu `string`, `number`, `enum`, `path`, `bool`). Ô kiểu `path` có nút `[Duyệt]` mở Native File Dialog.
  * **Trường Mật Khẩu (Secret Param):** Nhập qua ô password (ẩn ký tự, có toggle `[👁️]`). **Tuyệt đối không lưu vào `template_preset`** và không lưu vào DB lịch sử `run_session`/`run_event`. Khi xem trước lệnh (`template_preview`), hiển thị dạng che mờ `******`.
  * **Live Preview:** Mọi thay đổi giá trị kích hoạt IPC `template_preview` xuống Rust backend để nhận câu lệnh đã render an toàn (`shell_words::quote` cho Shell hoặc tách mảng token cho Direct Argv).
  * **Quản lý Preset:** Cho phép chọn bộ giá trị đã lưu, hoặc nhập tên để lưu preset mới qua IPC `template_presets_create`.
  * **Kích Hoạch Chạy:** Bấm `[▷ Thực Thi Lệnh]` hoặc nhấn `Enter` gọi IPC `template_run`. Backend Rust validate, tạo `run_session` + `run_event`, spawn tiến trình PTY và trả về `run_event_id` để frontend mở tab terminal tại SCR-01 Workspace.

---

### MOD-11: Modal Soạn Mẫu Lệnh & Định Nghĩa Tham Số (TemplateEditorModal)

* **Tiêu đề:** *"Tạo Mẫu Lệnh Mới"* hoặc *"Chỉnh Sửa Mẫu Lệnh: [Tên]"*
* **Kích thước:** 720px x 650px (Modal trung tâm có backdrop mờ).
* **Wireframe:**

```
+-----------------------------------------------------------------------------+
|  📝 Soạn Mẫu Lệnh & Định Nghĩa Tham Số                                 [X]  |
+-----------------------------------------------------------------------------+
|  Tên Mẫu Lệnh (*)                                                           |
|  [ FFmpeg Video Converter                                              ]    |
|                                                                             |
|  Mô Tả Vắn Tắt                                                              |
|  [ Mẫu chuyển đổi mã hóa video với các tham số chất lượng và đường dẫn ]    |
|                                                                             |
|  Phương Thức Thực Thi (*)                                                   |
|  ( ) Direct Argv (Tách token trước)   (o) Shell Execution (Quoting an toàn)|
|                                                                             |
|  Chuỗi Cấu Trúc Mẫu (Template String) (*) - Dùng {{name}} cho tham số        |
|  +-----------------------------------------------------------------------+  |
|  | ffmpeg -i {{input}} -crf {{crf}} -key {{secret_key}} {{output}}       |  |
|  +-----------------------------------------------------------------------+  |
|  [⚡ Tự Động Trích Xuất Placeholders từ Chuỗi Lệnh]                           |
|                                                                             |
|  DANH SÁCH THAM SỐ KHAI BÁO (TEMPLATE PARAMETERS):                          |
|  +-----------------------------------------------------------------------+  |
|  | THAM SỐ    | NHÃN (LABEL) | KIỂU DỮ LIỆU | MẶC ĐỊNH | REQ | SECRET | XÓA |  |
|  |------------+--------------+--------------+----------+-----+--------+-----|  |
|  | input      | File Đầu Vào | [Path   ▼]   |          | [x] | [ ]    | [X] |  |
|  | crf        | Mức Nén CRF  | [Number ▼]   | 23       | [ ] | [ ]    | [X] |  |
|  | secret_key | Mã Giải Mã   | [String ▼]   |          | [x] | [x]    | [X] |  |
|  | output     | File Đầu Ra  | [String ▼]   |          | [x] | [ ]    | [X] |  |
|  +-----------------------------------------------------------------------+  |
|  [+ Thêm Tham Số Khai Báo Mới]                                              |
+-----------------------------------------------------------------------------+
|  [Hủy Bỏ]                                          [💾 Lưu Mẫu Lệnh]        |
+-----------------------------------------------------------------------------+
```

* **Quy tắc Nghiệp vụ:**
  * **Auto Extract Placeholders:** Nút `[⚡ Tự Động Trích Xuất Placeholders]` tự động phân tích `template_string` tìm các biểu thức `{{name}}` và thêm vào danh sách tham số khai báo nếu chưa có.
  * **Quản lý Param:** Cấu hình Nhãn (Label), Kiểu dữ liệu (`String`, `Number`, `Enum`, `Path`, `Bool`), Giá trị mặc định, Cờ bắt buộc (`Required`), Cờ ẩn bí mật (`Secret`), và thứ tự `param_order`.


---

### MOD-13: Hộp Thoại Xác Nhận Thao Tác Đặc Quyền & Xóa Dữ Liệu (ConfirmDialog / Privileged Confirm Modal)

* **Tiêu đề:** *"Xác Nhận Thao Tác Đặc Quyền"* hoặc *"Xác Nhận Xóa Dữ Liệu"*
* **Kích thước:** 520px x 280px (Modal trung tâm compact có backdrop mờ).
* **Wireframe (Biến thể Hazard / Destructive):**

```
+-----------------------------------------------------------------------------+
|  ⚠️ CẢNH BÁO: XÁC NHẬN THAO TÁC NGUY HIỂM                              [X]  |
+-----------------------------------------------------------------------------+
|  [ 🛑 Icon Warning ]  Xác nhận xóa Mẫu Lệnh "FFmpeg Video Converter"?       |
|                                                                             |
|  Hành động này sẽ xóa vĩnh viễn Mẫu Lệnh cùng 2 bộ Presets tương ứng        |
|  khỏi cơ sở dữ liệu SQLite cục bộ. Thao tác này KHÔNG THỂ HỒI PHỤC.         |
|                                                                             |
|  [x] Tôi đã đọc kỹ cảnh báo và xác nhận thực hiện thao tác này.             |
+-----------------------------------------------------------------------------+
|  [ Hủy Bỏ (Esc) ]                         [ 🗑 XÁC NHẬN XÓA VĨNH VIỄN ]     |
+-----------------------------------------------------------------------------+
```

* **Wireframe (Biến thể Privileged Warning / System Action):**

```
+-----------------------------------------------------------------------------+
|  ⚡ XÁC NHẬN TÁC VỤ ĐẶC QUYỀN HỆ THỐNG                                 [X]  |
+-----------------------------------------------------------------------------+
|  [ ⚠️ Icon Alert ]  Kích hoạt Tự Khởi Động Nhóm: "Web Platform Dev"?       |
|                                                                             |
|  Nhóm lệnh sẽ tự động khởi chạy ngay khi ứng dụng mở cùng hệ điều hành.     |
|  Đảm bảo chuỗi lệnh trong nhóm an toàn và không gây xung đột tài nguyên.    |
|                                                                             |
|  [x] Xác nhận tin cậy chuỗi lệnh này (Trusted Zone).                        |
+-----------------------------------------------------------------------------+
|  [ Hủy Bỏ ]                               [ ⚡ KÍCH HOẠT AUTOSTART ]         |
+-----------------------------------------------------------------------------+
```

* **Quy tắc Nghiệp vụ & Thiết kế UI:**
  * **Hai biến thể trực quan:**
    1. **Biến thể Destructive (Đỏ / Hazard):** Áp dụng cho xóa Lệnh (`SCR-02`), xóa Template (`SCR-06`), xóa Nhóm (`SCR-03`), xóa Preset (`MOD-10`), và xóa sạch Lịch sử chạy (`SCR-04`). Nút bấm màu đỏ hazard (`--status-failed: #ef4444`). Có vạch cảnh báo sọc chéo hazard, icon `ShieldAlert` hoặc `AlertTriangle`, và hộp cảnh báo *"HÀNH ĐỘNG NÀY KHÔNG THỂ HỒI PHỤC!"*.
    2. **Biến thể Privileged Warning (Vàng / System):** Áp dụng cho Autostart, Import đè DB, Chạy lệnh Shell không sandbox. Nút bấm màu cam/vàng (`--status-warning: #f59e0b` hoặc `--primary: #744791`).
  * **Cờ Xác Nhận Bắt Buộc (`confirmed: true`):** Truyền cờ xác nhận an toàn xuống Backend Rust qua IPC để bảo vệ ranh giới vùng tin cậy (Trusted Zone).
  * **Tiêu chuẩn Hóa Toàn Hệ Thống:** Thay thế triệt để 100% mọi lệnh `window.confirm` mặc định của hệ thống/trình duyệt, hỗ trợ đóng/hủy bằng phím `Esc` và xác nhận bằng phím `Enter`.

---

### MOD-14: Hộp Thoại Đổi Tên Terminal Thủ Công (Rename Manual Terminal Modal)

#### 1. Mục đích & Vai trò
Cho phép người dùng tùy biến nhãn tên hiển thị của các tab terminal được mở thủ công (empty shell terminal PTY) trực tiếp từ giao diện Tab Strip thông qua thao tác **nháy đúp chuột (`double-click`) vào tên tab**. Tính năng này giúp người dùng dễ dàng phân biệt giữa các phiên làm việc terminal độc lập (ví dụ: `Worker Debug`, `DB Migration`, `Scratchpad`, `Vite Test`) mà không làm gián đoạn tiến trình PTY đang chạy ngầm hoặc Ring Buffer.

#### 2. Kích thước & Vị trí
* **Kích thước:** Compact modal 480px x 260px.
* **Vị trí:** Canh giữa màn hình (Center-aligned), phủ lên trên backdrop mờ (`background: rgba(11, 13, 19, 0.75); backdrop-filter: blur(8px)`).
* **Độ ưu tiên z-index:** Thuộc tầng modal (`z-index: 1050`), cao hơn Tab Strip và xterm viewport.
* **Stitch Screen Reference:** Screen ID `beabcffb357343e08f5c528b095cf2ac` tại Project `14914224436748087443`.

#### 3. Wireframe Chi tiết

```
+-----------------------------------------------------------------------------+
|  ✏️ Đổi Tên Terminal (Rename Terminal)                                 [X]  |
|  Shell PTY • PID: 3418 • PowerShell                                         |
+-----------------------------------------------------------------------------+
|  💡 Nháy đúp vào tab thủ công để đổi tên hiển thị. Tên tùy chỉnh giúp       |
|  phân biệt nhanh các phiên PTY độc lập trong không gian làm việc.            |
|                                                                             |
|  Tên Terminal Mới (*)                                               19 / 32 |
|  +-----------------------------------------------------------------------+  |
|  | Worker Debug & Test                                               (x) |  |
|  +-----------------------------------------------------------------------+  |
|                                                                             |
|  Gợi Ý Nhanh (Presets):                                                     |
|  [Terminal] [Dev Server] [Worker Debug] [Build & Watch] [Logs] [Scratchpad] |
+-----------------------------------------------------------------------------+
|  ⌨️ Nhấn Esc để hủy, Enter để lưu                  [ Hủy Bỏ ]  [💾 Lưu Tên] |
+-----------------------------------------------------------------------------+
```

#### 4. Các Thành phần Dữ liệu & Quy tắc Kiểm Thực (Validation Rules)
1. **Thông tin Ngữ cảnh (Header Context):**
   * Tiêu đề modal kèm icon chỉnh sửa/terminal (`✏️`).
   * Meta badge hiển thị loại shell (`PowerShell / Cmd / Bash`) và `PID` hiện hành của tiến trình terminal con.
   * Nút đóng nhanh `[X]` góc trên bên phải.
2. **Trường Nhập Tên (Terminal Name Input):**
   * **Giá trị khởi tạo:** Tự động điền (pre-filled) tên hiện tại của tab. Khi modal mở ra, hệ thống tự động `autofocus` và bôi đen toàn bộ chuỗi text (`select()`) để người dùng có thể gõ đè tên mới ngay lập tức.
   * **Giới hạn ký tự:** Độ dài từ 1 đến 32 ký tự, hiển thị bộ đếm ký tự thời gian thực (`counter: current / 32`).
   * **Quy tắc Kiểm thực (Validation):**
     * Không được để trống (sau khi `trim()`).
     * Không chứa ký tự xuống dòng (`\n`, `\r`) hoặc chuỗi điều khiển ANSI.
     * Nếu chuỗi rỗng: vô hiệu hóa nút "Lưu Tên" và hiển thị cảnh báo đỏ *"Tên terminal không được để trống"*.
   * Nút xóa nhanh nội dung `(x)` (Clear icon) bên trong ô input khi có văn bản.
3. **Thẻ Gợi Ý Nhanh (Preset Chips / Quick Tags):**
   * Cung cấp các nhãn thường dùng: `[Terminal]`, `[Dev Server]`, `[Worker Debug]`, `[Build & Watch]`, `[API Test]`, `[Logs]`, `[Scratchpad]`.
   * Click vào một chip sẽ điền ngay tên đó vào ô input và focus lại để người dùng điều chỉnh thêm nếu muốn.
4. **Hành động & Phím Tắt (Actions & Shortcuts):**
   * **Nút "Lưu Tên" (`Enter`):** Lưu tên mới vào thuộc tính `tab.name` của tab hiện hành trong bộ nhớ Vue State (`openTabs`). Đóng modal và hiển thị Toast nhẹ: *"Đã đổi tên tab thành: [Tên mới]"*.
   * **Nút "Hủy Bỏ" (`Esc`):** Hủy thao tác, giữ nguyên tên cũ và đóng modal.
   * Click ra ngoài vùng backdrop mờ tương đương với lệnh Hủy (`Esc`).

#### 5. Phạm vi Hiệu lực & Lưu trữ Trạng thái (Lifecycle & State Scope)
* **In-Memory Tab Scope:** Tên tùy chỉnh được lưu trữ trực tiếp trong mảng trạng thái `openTabs: OpenTabItem[]` ở component [DockHost.vue](../src/components/terminal/DockHost.vue).
* **Không làm gián đoạn PTY:** Thao tác đổi tên chỉ tác động đến lớp hiển thị của Tab Strip (DOM label và title attribute), hoàn toàn **không khởi động lại tiến trình, không gián đoạn stream dữ liệu PTY hay can thiệp vào Ring Buffer**.
* **Phân định rõ ràng:** Tab của `command_definition` lấy tên theo DB SQLite; chỉ tab mở thủ công từ nút `[+ Terminal mới]` mới có cờ `isManual: true` để cho phép đổi tên.

---

### MOD-15: Hộp Thoại Nhập Liệu Nhanh & Xác Nhận Tương Tác (PromptDialog / Quick Prompt Modal)

#### 1. Mục đích & Vai trò
Thay thế triệt để các hộp thoại mặc định của trình duyệt (`window.prompt`, `window.alert`) bằng modal tùy chỉnh mang phong cách **Technical Precision Dark**. Hộp thoại này phục vụ các luồng nhập liệu chuỗi ngắn, tức thì có kiểm thực (single-input inline prompt) như:
* Lưu một dòng lịch sử thực thi thành **Lệnh mới** vào Thư Viện Lệnh (`HistoryView -> saveAsCommand`).
* Lưu một dòng lịch sử shell thành **Mẫu Lệnh Shell mới** (`HistoryView -> saveAsTemplate`).
* Lưu bộ giá trị tham số cấu hình hiện tại thành **Preset mới** (`TemplateRunModal -> promptSavePreset`).

#### 2. Kích thước & Vị trí
* **Kích thước:** Compact modal 480px x 240px (co giãn linh hoạt theo nội dung gợi ý/lỗi).
* **Vị trí:** Canh giữa màn hình (Center-aligned), phủ lên trên backdrop mờ (`background: rgba(11, 13, 19, 0.8); backdrop-filter: blur(6px)`).
* **Độ ưu tiên z-index:** Thuộc tầng modal (`z-index: 120`), ngang hàng với ConfirmDialog và các modal cấu hình hệ thống.
* **Stitch Screen Reference:** Screen ID `223e53ac59e44c9381610af4eba20ac1` tại Project `14914224436748087443`.

#### 3. Wireframe Chi tiết

##### Biến thể 1: Lưu Câu Lệnh Mới / Mẫu Lệnh từ Lịch Sử (Save As Command / Template)
```
+-----------------------------------------------------------------------------+
|  ✏️ LƯU LỆNH MỚI TỪ LỊCH SỬ                                            [X]  |
|  MOD-15 • Technical Input Gate • Shell History Replay                       |
+-----------------------------------------------------------------------------+
|  💡 Nhập tên định danh gợi nhớ để lưu câu lệnh này vào Thư Viện Lệnh.      |
|                                                                             |
|  Tên Câu Lệnh Mới (*)                                               18 / 64 |
|  +-----------------------------------------------------------------------+  |
|  | cargo test --lib --features desktop                               (x) |  |
|  +-----------------------------------------------------------------------+  |
|  ⚠️ Tên câu lệnh không được để trống hoặc chỉ chứa khoảng trắng.            |
+-----------------------------------------------------------------------------+
|  ⌨️ Nhấn Esc để hủy, Enter để lưu                  [ Hủy Bỏ ]   [💾 Lưu Lại] |
+-----------------------------------------------------------------------------+
```

##### Biến thể 2: Lưu Bộ Tham Số Mẫu Lệnh (Save Template Preset)
```
+-----------------------------------------------------------------------------+
|  🔖 LƯU BỘ THAM SỐ PRESET                                              [X]  |
|  MOD-15 • Parameter Snapshot Gate • FFmpeg Transcoder                       |
+-----------------------------------------------------------------------------+
|  💡 Lưu lại các giá trị tham số hiện thời thành một Preset tái sử dụng.     |
|                                                                             |
|  Tên Preset Mới (*)                                                 12 / 32 |
|  +-----------------------------------------------------------------------+  |
|  | Fast 1080p Web                                                    (x) |  |
|  +-----------------------------------------------------------------------+  |
+-----------------------------------------------------------------------------+
|  ⌨️ Nhấn Esc để hủy, Enter để lưu                  [ Hủy Bỏ ]  [💾 Lưu Preset]|
+-----------------------------------------------------------------------------+
```

#### 4. Các Thành phần Dữ liệu & Quy tắc Kiểm Thực (Validation Rules)
1. **Thông tin Ngữ cảnh (Header Context):**
   * Tiêu đề modal kèm icon biểu trưng (`✏️ Edit3`, `🔖 Bookmark`, `Terminal`).
   * Subtitle kỹ thuật định danh nguồn gọi (`MOD-15 • Interactive Input Gate`).
   * Nút đóng nhanh `[X]` góc trên bên phải.
2. **Trường Nhập Liệu (Input Field):**
   * **Tự động kích hoạt (Autofocus & Select):** Ngay khi modal hiển thị, con trỏ tự động focus vào ô input và bôi đen toàn bộ giá trị ban đầu (`select()`) để người dùng có thể gõ đè ngay mà không cần dùng chuột.
   * **Nút xóa nhanh `(x)` (Clear Button):** Xuất hiện khi có văn bản trong ô input, click sẽ xóa sạch nội dung và giữ focus.
   * **Bộ đếm ký tự (Character Counter):** Hiển thị thời gian thực theo định dạng `current / maxLength`.
   * **Kiểm thực dữ liệu (Live Validation):**
     * Không được để trống hoặc chỉ toàn khoảng trắng (`value.trim().length > 0`).
     * Độ dài tối đa khống chế theo prop `maxLength` (mặc định 64 ký tự).
     * Loại bỏ các ký tự xuống dòng (`\r`, `\n`).
     * Khi không hợp lệ: Hiển thị cảnh báo lỗi màu đỏ (`--status-failed: #ef4444`) và vô hiệu hóa nút Submit (`disabled: true`).
3. **Phím Tắt & Tương Tác Bàn Phím (Keyboard UX):**
   * **Phím `Enter`:** Tương đương bấm nút Xác Nhận / Lưu (nếu giá trị hợp lệ).
   * **Phím `Esc`:** Hủy bỏ thao tác và đóng modal ngay lập tức.
   * **Backdrop Click:** Click ra ngoài vùng modal tương đương lệnh Hủy (`Esc`).
4. **Xử lý Thông báo & Lỗi (Error Handling):**
   * Thay thế hoàn toàn `window.alert(...)` bằng thông báo lỗi trực tiếp trên giao diện hoặc inline error message, đảm bảo trải nghiệm người dùng liền mạch không bị chặn bởi pop-up hệ điều hành.

---

### MOD-16: Hộp Thoại Ghi Phím Tắt (Keybinding Capture Modal)

#### 1. Mục đích & Vai trò
Ghi một tổ hợp phím mới cho một tác vụ ở tab Phím Tắt (SCR-05 mục 3), kiểm tra hợp lệ và báo xung đột trước khi lưu. Modal mở khi bấm `✎`, nháy đúp vào hàng, nhấn `Enter` trên hàng, hoặc chọn "Thêm phím" (lúc đó modal thêm binding mới thay vì thay binding cũ).

#### 2. Kích thước & Vị trí
Rộng 460px, căn giữa màn hình, nền mờ. Dùng chung khung modal với MOD-14 và MOD-15.
* **Stitch Screen Reference:** Screen ID `aa0bb46697ef4c088513deb9076db865` tại Project `14914224436748087443`.

#### 3. Wireframe Chi tiết

```
+------------------------------------------------------------+
| Đổi phím tắt: Chia đôi panel sang phải                 [X] |
| workspace.splitRight                                       |
+------------------------------------------------------------+
|  Nhấn tổ hợp phím mong muốn, rồi nhấn Enter để lưu.        |
|                                                            |
|        +------------------------------------------+        |
|        |      [Ctrl]  +  [Shift]  +  [R]          |        |  <- vùng ghi, autofocus
|        +------------------------------------------+        |
|                                                            |
|  Phạm vi:  [ Workspace                    ▼ ]              |
|                                                            |
|  ⚠ Phím này đang được dùng bởi:                            |
|     • Mở terminal mới — Workspace (Người dùng)             |
|     Lưu sẽ khiến tác vụ trên không chạy bằng phím này.     |
|                                                            |
+------------------------------------------------------------+
|                        [ Hủy (Esc) ]   [ Lưu (Enter) ]     |
+------------------------------------------------------------+
```

#### 4. Quy tắc Tương tác
1. **Ghi phím:**
   * Vùng ghi nhận mọi `keydown` và hiển thị tổ hợp **mới nhất** đủ modifier + phím chính.
   * Khi người dùng mới giữ modifier, vùng ghi hiện tạm `[Ctrl] + …`.
   * Mọi phím bị `preventDefault`, kể cả `Ctrl+W` và `Ctrl+Q`. Dispatcher toàn cục tạm dừng trong lúc modal mở.
   * Bỏ qua sự kiện IME (`isComposing`, `Process`, `229`).
2. **Phím điều khiển modal:**
   * `Enter` và `Esc` **đơn** (không kèm modifier) là Lưu và Hủy, không bị ghi thành phím tắt.
   * Muốn gán `Ctrl+Enter` thì vẫn ghi được, vì có modifier.
3. **Kiểm tra hợp lệ** theo SCR-05 mục 3.6, chạy lại mỗi khi phím hoặc phạm vi thay đổi:
   * Phím bị khóa, hoặc thiếu modifier ở phạm vi `global`/`workspace`: hiện thông báo đỏ và vô hiệu nút Lưu.
   * Phím dành cho shell: hiện cảnh báo vàng nhưng vẫn cho lưu.
   * Có xung đột: hiện cảnh báo vàng liệt kê các tác vụ đang dùng phím đó. Vẫn cho lưu, sau đó hai hàng hiện ⚠ trong bảng.
   * Phím trùng đúng binding hiện tại: nút Lưu đổi thành "Không đổi", và bấm sẽ chỉ đóng modal.
4. **Phạm vi:**
   * Mặc định lấy phạm vi của binding đang sửa.
   * Dropdown chỉ cho các phạm vi mà tác vụ có handler, ví dụ `workspace.*` không thể chọn `explorer`.
5. **Lưu:**
   * Ghi override (mục 3.5) và áp dụng ngay.
   * Đóng modal, cuộn bảng tới hàng vừa sửa và highlight hàng đó 1.5 giây.

---

### MOD-17: Modal Thêm / Sửa Máy Chủ SSH (SSH Host Editor)

#### 1. Mục đích & Vai trò
Tạo hoặc sửa một host profile ở SCR-07. Mở từ `[+ Thêm]`, `⋯ → Sửa`, `⋯ → Nhân bản` (điền sẵn, tên thêm hậu tố "(bản sao)"), hoặc từ hàng đã chọn trong màn Nhập `~/.ssh/config`.

#### 2. Kích thước & Vị trí
Rộng 560px, căn giữa, nền mờ, dùng chung khung modal với MOD-02.
* **Stitch Screen Reference:** Screen ID `1cc2e20a0651434d8304fc6f64c6755d` tại Project `14914224436748087443`.

#### 3. Wireframe Chi tiết

```
+------------------------------------------------------------------+
| Thêm máy chủ SSH                                             [X] |
+------------------------------------------------------------------+
|  Tên hiển thị *   [ prod-api                                   ] |
|  Máy chủ *        [ 10.0.4.12                ]  Cổng [ 22    ]   |
|  Người dùng *     [ deploy                                     ] |
|                                                                  |
|  Xác thực         (•) Tự động  ( ) Agent  ( ) Khóa  ( ) Mật khẩu |
|  Khóa riêng       [ Tệp khóa | Dán nội dung khóa ]               |  <- hiện khi Tự động / Khóa
|  Tệp khóa         [ ~/.ssh/id_ed25519                 ] [Chọn…]  |  <- nguồn = Tệp khóa
|  Nội dung khóa    +--------------------------------------------+ |  <- nguồn = Dán nội dung khóa
|                   | -----BEGIN OPENSSH PRIVATE KEY-----        | |
|                   +--------------------------------------------+ |
|  Passphrase       [ ••••••••                              ] [👁] |  <- hiện khi Tự động / Khóa
|  Mật khẩu         [ ••••••••                              ] [👁] |  <- hiện khi Tự động / Mật khẩu
|                   [x] Lưu mật khẩu                               |
|                       Nơi lưu: (•) Keyring hệ điều hành          |
|                                ( ) Cơ sở dữ liệu của app (SQLite)|
|                       ⓘ Mã hóa bằng khóa riêng của máy này, không |  <- hiện khi chọn SQLite
|                         theo bản sao lưu sang máy khác.          |
|                                                                  |
|  Đi qua (jump)    [ — Không —                               ▼ ]  |
|  Thư mục mặc định [ /var/www/app                               ] |
|  Màu nhận diện    (●)(●)(●)(●)(●)(●)( ) Không                    |
|  Tag              [ prod ×] [ api ×] [ thêm tag…   ]             |
|                                                                  |
|  ▸ Nâng cao                                                      |
|     [ ] Chuyển tiếp agent (agent forwarding)                     |
|         ⚠ Người có root trên máy này dùng được agent của bạn.    |
|     [ ] Dùng OpenSSH hệ thống (không có trình quản lý file)      |
|         Lệnh chạy: ssh -p 22 deploy@10.0.4.12                    |
|     [Quên host key]  (chỉ xóa trong known_hosts của app)         |
+------------------------------------------------------------------+
| [Kiểm tra kết nối]  ✓ Host key · ✓ Xác thực · ✓ SFTP             |
|                                         [ Hủy ]   [ Lưu (Ctrl+Enter) ] |
+------------------------------------------------------------------+
```

#### 4. Quy tắc Tương tác
1. **Ô bắt buộc:** Tên, Máy chủ, Người dùng.
   * `Máy chủ` nhận hostname, IPv4 hoặc IPv6 (không cần ngoặc vuông). Dán `user@host:port` vào ô này thì tự tách sang 3 ô.
   * Cổng 1–65535.
   * Tên trùng host khác chỉ là cảnh báo vàng, vẫn cho lưu.
2. **Mật khẩu:**
   * Checkbox **"Lưu mật khẩu"** mặc định **tắt** (trừ khi dùng nội dung khóa dán vào). Tắt = không lưu ở đâu cả, mỗi lần kết nối app hỏi bằng MOD-19.
   * Bật thì chọn nơi lưu. Mặc định là Keyring nếu khả dụng (`ssh_secret_status`); nếu không, lựa chọn Keyring bị mờ kèm chú thích "Không tìm thấy keyring trên máy này" và chọn sẵn SQLite.
   * Ô mật khẩu chỉ gửi xuống Rust khi bấm Lưu; Rust ghi vào nơi đã chọn và **không** trả về frontend.
   * Khi sửa host đã có mật khẩu, ô hiện `••••••••` giả. Để nguyên = giữ mật khẩu cũ; xóa trắng rồi lưu hoặc bỏ checkbox = quên mật khẩu.
   * Đổi nơi lưu (Keyring ⇄ SQLite) với ô để nguyên: Rust chuyển mật khẩu cũ sang nơi mới, không cần nhập lại.
   * Áp dụng tương tự cho **passphrase** của khóa.
   * Ô mật khẩu và passphrase luôn hiện theo phương thức xác thực. Khi không lưu, giá trị chỉ dùng cho "Kiểm tra kết nối"; lúc kết nối thật, app hỏi bằng MOD-19.
3. **Nguồn khóa riêng** (`key_source`):
   * **Tệp khóa** (`file`): dùng `key_path`, để trống thì thử `~/.ssh/id_ed25519`, `id_ecdsa`, `id_rsa`.
   * **Dán nội dung khóa** (`content`): nội dung khóa OpenSSH/PEM được lưu như mật khẩu (`ssh_secret.kind = 'private_key'`), nên checkbox "Lưu" bị khóa ở trạng thái bật. Khóa được kiểm tra trước khi lưu; khóa có passphrase vẫn nhận, passphrase hỏi khi đăng nhập. Khóa PuTTY `.ppk` bị từ chối kèm hướng dẫn xuất sang OpenSSH.
   * Windows Credential Manager chỉ chứa được ~2,5 KB, nên khóa RSA 4096 phải lưu trong SQLite.
   * "Dùng OpenSSH hệ thống" chỉ đi với Tệp khóa.
   * Chuyển về Tệp khóa thì nội dung khóa đã lưu bị xóa.
4. **Jump host:** dropdown chỉ liệt kê host khác không dùng OpenSSH hệ thống, và loại các host sẽ tạo vòng lặp.
5. **Kiểm tra kết nối:**
   * Chạy với giá trị **đang nhập** (chưa lưu), kể cả mật khẩu, passphrase và nội dung khóa. Có thể mở MOD-18/MOD-19 trong lúc kiểm tra.
   * Đang chạy thì nút đổi thành `[Hủy kiểm tra]`.
6. **Lưu:** `Ctrl+Enter` hoặc nút Lưu. `Esc` hủy; nếu form đã sửa thì hỏi "Bỏ thay đổi?".

---

### MOD-18: Hộp Thoại Xác Minh Host Key (Host Key Verification)

#### 1. Mục đích & Vai trò
Chặn kết nối khi khóa của server chưa từng thấy hoặc đã thay đổi, đúng tinh thần "Vùng tin cậy" (1.1.5). Do sự kiện `ssh-hostkey-request` mở ra, ở bất kỳ màn hình nào.
* **Stitch Screen Reference:** Screen ID `508a57936ad841b49bac016c9094a9ca` tại Project `14914224436748087443`.

#### 2. Wireframe Chi tiết

**a) Khóa lạ (lần đầu kết nối):**

```
+------------------------------------------------------------+
| Xác minh máy chủ mới                                   [X] |
+------------------------------------------------------------+
|  Lần đầu kết nối tới prod-api (10.0.4.12:22).              |
|  Hãy đối chiếu dấu vân tay với quản trị viên máy chủ:      |
|                                                            |
|    Loại khóa:  ED25519                                     |
|    SHA256:     nThbg6kXUpJWGl7E1IGOCspRomTxdCARLviKw6E5SY8 |  <- mono, chọn & copy được
|                                                            |
|  [x] Ghi nhớ khóa này                                      |
+------------------------------------------------------------+
|                         [ Hủy kết nối ]   [ Tin cậy & kết nối ] |
+------------------------------------------------------------+
```

**b) Khóa đã thay đổi:**

```
+------------------------------------------------------------+
| ⛔ Khóa máy chủ đã thay đổi                            [X] |
+------------------------------------------------------------+
|  Khóa của 10.0.4.12:22 KHÁC với khóa đã lưu.               |
|  Có thể server vừa cài lại, hoặc có kẻ đang chặn kết nối.  |
|                                                            |
|    Đã lưu:  ED25519  SHA256:nThbg6kX…5SY8  (~/.ssh/known_hosts:14) |
|    Hiện tại: ED25519 SHA256:Qm9yZWQg…bG9s                  |
|                                                            |
|  Kết nối đã bị hủy. Nếu chắc chắn server đã đổi khóa, xóa  |
|  khóa cũ trong Sửa máy chủ → Nâng cao → Quên host key.     |
+------------------------------------------------------------+
|                                               [ Đóng ]     |
+------------------------------------------------------------+
```

#### 3. Quy tắc Tương tác
1. **Khóa lạ:**
   * Nút mặc định (focus khi mở) là **Hủy kết nối**, không phải "Tin cậy", để `Enter` vô tình không chấp nhận khóa.
   * Bỏ chọn "Ghi nhớ" = tin cậy cho lần này, không ghi file.
2. **Khóa đổi:**
   * Không có nút kết nối. "Quên host key" chỉ xóa dòng trong `known_hosts` **của app**.
   * Nếu khóa cũ nằm trong `~/.ssh/known_hosts` thì hiện đường dẫn và số dòng để người dùng tự sửa; app không ghi vào file đó.
3. **Nhiều yêu cầu cùng lúc** (mở nhiều tab tới cùng host): chỉ hiện một hộp; câu trả lời áp dụng cho mọi yêu cầu đang chờ của host đó.

---

### MOD-19: Hộp Thoại Xác Thực SSH (Password / Passphrase / OTP)

#### 1. Mục đích & Vai trò
Hỏi thông tin bí mật khi kết nối: mật khẩu, passphrase của khóa, hoặc các câu hỏi keyboard-interactive (mã OTP, 2FA). Do sự kiện `ssh-auth-request` mở ra. Mở rộng từ khung PromptDialog (MOD-15), có hỗ trợ nhiều ô.
* **Stitch Screen Reference:** Screen ID `1a27bb7ff77b47f4815808cc4c034f15` tại Project `14914224436748087443`.

#### 2. Wireframe Chi tiết

```
+------------------------------------------------------------+
| Đăng nhập deploy@prod-api                              [X] |
+------------------------------------------------------------+
|  Máy chủ yêu cầu:                                          |
|    Password:              [ ••••••••••              ] [👁] |
|    Verification code:     [ 482913                  ]      |  <- echo = true thì hiện chữ
|                                                            |
|  [x] Lưu mật khẩu   vào: [ Keyring hệ điều hành     ▼ ]   |  <- chỉ với ô mật khẩu/passphrase
|  ✗ Sai mật khẩu (lần 1/3)                                  |
+------------------------------------------------------------+
|                                    [ Hủy ]   [ Đăng nhập ] |
+------------------------------------------------------------+
```

#### 3. Quy tắc Tương tác
1. **Hiển thị nhãn:** nhãn mỗi ô lấy nguyên văn từ server (keyboard-interactive). Server gửi `echo=false` thì ô ẩn ký tự.
2. **Lưu mật khẩu:**
   * Checkbox và dropdown nơi lưu (Keyring / SQLite) giống MOD-17; mặc định lấy theo `secret_store` hiện tại của host, host chưa lưu thì checkbox tắt.
   * Chỉ áp dụng cho **mật khẩu đăng nhập** và **passphrase của khóa**, không bao giờ cho OTP.
   * Chỉ lưu sau khi xác thực **thành công**.
3. **Sai:** hiện lỗi đỏ dưới ô, xóa ô mật khẩu và focus lại. Sau 3 lần (hoặc khi server đóng kết nối) thì đóng hộp; tab hiện banner "Xác thực thất bại" kèm `[Thử lại]`.
4. **Hủy:** gửi `answers: []`. Kết nối dừng, tab hiện "Đã hủy đăng nhập".
5. **Xử lý giá trị nhập:**
   * Không dùng `v-model` lưu lâu: ô được xóa ngay khi hộp đóng.
   * Giá trị không đi qua store/log nào của frontend.
   * Dispatcher phím tắt đã bỏ qua phạm vi modal, nên `Ctrl+Shift+V` không dán nhầm vào terminal phía sau.

---

### MOD-20: Hộp Thoại Xung Đột Tệp Khi Truyền (Transfer Conflict)

#### 1. Mục đích & Vai trò
Hỏi cách xử lý khi tệp đích đã tồn tại. Do sự kiện `transfer-conflict` mở ra. Các tệp không xung đột trong cùng job vẫn chạy tiếp.
* **Stitch Screen Reference:** Screen ID `d97ef28f285549d78f858799be271d14` tại Project `14914224436748087443`.

#### 2. Wireframe Chi tiết

```
+------------------------------------------------------------+
| Tệp đã tồn tại                                         [X] |
+------------------------------------------------------------+
|  prod-api:/var/www/app/public/app.js                       |
|                                                            |
|               KÍCH THƯỚC     SỬA ĐỔI                       |
|   Mới (local)   1.2 MB       Hôm nay 14:05   ← mới hơn     |
|   Đang có       1.1 MB       Hôm qua 09:12                 |
|                                                            |
|   (•) Ghi đè                                               |
|   ( ) Ghi đè nếu tệp mới hơn                               |
|   ( ) Bỏ qua                                               |
|   ( ) Giữ cả hai → app (1).js                              |
|                                                            |
|   [x] Áp dụng cho 6 xung đột còn lại trong job này         |
+------------------------------------------------------------+
|                          [ Hủy cả job ]   [ Tiếp tục ]     |
+------------------------------------------------------------+
```

#### 3. Quy tắc Tương tác
1. **Lựa chọn mặc định:**
   * Mặc định là **Ghi đè nếu tệp mới hơn** khi tệp mới có mtime lớn hơn; ngược lại là **Bỏ qua**.
   * Lựa chọn cuối cùng của người dùng được nhớ trong phiên làm việc (không lưu DB).
2. **Áp dụng cho các xung đột còn lại:** ô này chỉ hiện khi còn ≥ 1 xung đột khác. Bật lên thì các xung đột sau không hỏi nữa.
3. **Xung đột kiểu khác loại** (thư mục gặp tệp cùng tên hoặc ngược lại):
   * Chỉ cho **Bỏ qua** hoặc **Giữ cả hai**; không cho ghi đè.
   * Thư mục gặp thư mục thì gộp, không hỏi.
4. **Đóng hộp:** `Esc` / `[X]` = Bỏ qua tệp này (không hủy cả job).

---

## 5. Quy cách Vòng đời Tab Terminal Dockview & PTY Reattach

Bảng ma trận hành vi giữa tương tác giao diện và tiến trình thực thi tại Rust Backend:

| Thao tác Người dùng trên UI | Trạng thái Tab Dockview | Trạng thái Tiến trình (Rust Backend) | Trạng thái Ring Buffer (RAM) | Ghi chú & Trách nhiệm Kỹ thuật |
| :--- | :--- | :--- | :--- | :--- |
| **Bấm nút [Play] Nhóm** | Tạo mới panel tab qua `dockview.api.addPanel()` | Fork process, gắn PTY, khởi tạo Ring Buffer | Bắt đầu nhận byte stream | Tự động focus vào tab đầu tiên của nhóm. |
| **Chuyển qua lại giữa các tab** | Tab cũ ẩn (hidden), tab mới active | Các tiến trình vẫn chạy độc lập bình thường | Buffer vẫn tiếp tục thu thập dữ liệu ngầm | Kích hoạt `fitAddon.fit()` để căn lại cols/rows của tab đang active. |
| **Nháy đúp vào tên tab thủ công** | Giữ nguyên viewport tab, mở Modal MOD-14 đổi tên | Tiến trình PTY vẫn chạy bình thường | Ring buffer tiếp tục nhận dữ liệu ngầm | Sau khi Lưu, tên tab đổi ngay trên Tab Strip; không ảnh hưởng PID hay lifecycle. |
| **Bấm [◫ Split Dọc] / [⬒ Split Ngang]** | Chuyển layout sang Dual-Pane; chia mảng tabs thành Pane A và Pane B | Giữ nguyên tiến trình; FitAddon trên cả 2 pane tự tính lại cols/rows | Không xáo trộn buffer; tiếp tục nhận stream bình thường | Gửi debounce IPC `pty_resize` cho cả 2 terminal đang active ở 2 pane. |
| **Kéo thanh Resizable Sash** | Cập nhật `splitRatio` realtime (20%–80%); hiển thị pointer overlay | Không can thiệp tiến trình backend | Buffer in-memory tiếp tục lưu bình thường | Khi nhả chuột: FitAddon resize lại cols/rows và gửi IPC `pty_resize`. |
| **Kéo thả Tab sang Pane đối diện** | Di chuyển đối tượng `OpenTabItem` sang mảng tab của Pane đích | Tiến trình PTY vẫn tiếp tục chạy không ngắt quãng | Không mất mát buffer log | Pane nguồn focus tab kế tiếp; Pane đích focus tab vừa nhận. |
| **Đóng tab cuối cùng của Pane B** | Tự động thu gọn (auto-collapse) về Single View; Pane A mở rộng 100% | Tiến trình của tab vừa đóng chuyển sang ngầm | Buffer vẫn tiếp tục lưu in-memory | Không để lại pane rỗng; FitAddon của Pane A tự bung đầy màn hình. |
| **Bấm nút [◻ Single View]** | Toàn bộ tab của Pane B gộp nối tiếp vào cuối Tab Strip của Pane A | Mọi tiến trình PTY vẫn tiếp tục chạy bình thường | Không làm mất dữ liệu log | Giao diện thu về 1 pane đơn giản; tab active của Pane A được focus. |
| **Bấm [x] Đóng thẻ Tab** | Panel tab bị destroy / unmount khỏi DOM | **TIẾN TRÌNH VẪN TIẾP TỤC CHẠY** | **BUFFER VẪN TIẾP TỤC LƯU** | Hiển thị thông báo Toast nhẹ: *"Đã ẩn tab. Lệnh vẫn chạy ngầm"*. |
| **Mở lại lệnh từ Sidebar** | Tạo lại panel tab dockview mới | Tiến trình đang chạy được gắn lại (Reattach) | **Xả ngược (flush) 1-2MB gần nhất** lên xterm | Người dùng xem lại được toàn bộ ngữ cảnh output gần nhất. |
| **Bấm nút [Stop Process] đỏ** | Tab giữ nguyên, hiển thị badge "Stopped" | Gửi SIGTERM / CTRL_C_EVENT; sau 5s gửi SIGKILL | Ngừng nhận stream; giữ nguyên log đã xả | Cập nhật `run_event.ended_at` và `exit_code` vào SQLite. |
| **Mở terminal SSH (SCR-07 / Palette)** | Tạo tab 🌐 mang màu host | Mở (hoặc dùng lại) kết nối SSH của host, mở channel `session` + `pty-req` | Bắt đầu nhận stream như PTY local | `run_event_id = ssh:<uuid>`. Xác thực / host key qua MOD-18, MOD-19. |
| **Đóng tab terminal SSH** | Tab bị unmount | **Channel vẫn mở**, shell ở xa vẫn chạy | Buffer vẫn nhận dữ liệu | Giống terminal local: mở lại thì reattach. |
| **Mạng rớt / server reboot** | Tab giữ nguyên, hiện banner "Mất kết nối" | Kết nối đóng; tự thử lại 3 lần | Giữ log cũ; khi nối lại chèn dòng ngăn cách | Kết nối lại = shell **mới**; chương trình chạy dở ở xa không quay lại. |
| **Dừng terminal SSH (MOD-01)** | Tab hiện "Stopped" | Đóng phiên (EOF) hoặc Ngắt ngay (đóng channel); kết nối dùng chung vẫn sống | Ngừng nhận stream | Hết channel thì kết nối tự đóng sau 60 giây. |
| **Bấm [X] Thoát Ứng dụng** | Toàn bộ UI chuyển sang Overlay MOD-09 | Duyệt toàn bộ process list; Graceful Shutdown | Giải phóng toàn bộ bộ nhớ khi app quit | Hợp đồng vòng đời: Thoát app là dừng triệt để mọi tiến trình con. |

---

## 6. Sơ đồ Luồng Thao tác Trọng yếu (User Journey Flows)

### 6.1. Luồng Khởi chạy Nhóm Lệnh & Quản lý Terminal Tab

```mermaid
sequenceDiagram
    autonumber
    actor User as Người dùng
    participant UI as Frontend (Vue/React + Dockview)
    participant IPC as Tauri v2 IPC Bridge
    participant PM as Process Manager (Rust Tokio)
    participant PTY as Kernel PTY & Child Process
    participant DB as SQLite WAL

    User->>UI: Bấm [▷ Play Group] tại Web Platform Dev
    UI->>DB: Truy vấn danh sách lệnh theo execution_order
    DB-->>UI: [1. db-init, 2. backend-api, 3. frontend-dev]
    UI->>IPC: invoke('start_run_session', { groupId })
    IPC->>PM: Tạo run_session mới & lưu DB
    loop Cho mỗi lệnh theo execution_order
        PM->>PTY: Fork child process (argv hoặc shell)
        PTY-->>PM: Gán PID & File Descriptor PTY
        PM->>PM: Khởi tạo Ring Buffer in-memory (2MB)
        PM-->>IPC: emit('process_started', { cmdId, pid })
        IPC-->>UI: Event process_started
        UI->>UI: dockview.api.addPanel({ id: cmdId, title: name })
        UI->>UI: Gắn xterm.js instance vào panel
        PTY-->>PM: Stream stdout/stderr (ANSI raw bytes)
        PM->>PM: Ghi vào Ring Buffer
        PM-->>IPC: emit('pty_data_chunk', { cmdId, bytes })
        IPC-->>UI: xterm.write(bytes)
    end
```

---

### 6.2. Luồng Khôi phục Dữ liệu An toàn (Restore with Rollback)

```mermaid
sequenceDiagram
    autonumber
    actor User as Người dùng
    participant UI as Restore Wizard UI (MOD-08)
    participant Backend as Tauri Rust Backend
    participant SQLite as SQLite Storage

    User->>UI: Chọn tệp sao lưu .sqlite & xác nhận nguồn tin cậy
    UI->>Backend: invoke('verify_backup_file', { filePath })
    Backend->>Backend: Copy vào thư mục temp
    Backend->>Backend: PRAGMA integrity_check & check schema_version
    Backend-->>UI: Kết quả: Hợp lệ (Verified)
    User->>UI: Bấm [Xác nhận ghi đè & Khôi phục]
    UI->>Backend: invoke('execute_restore_pipeline')
    Backend->>Backend: 1. Tạo bản snapshot dự phòng app.db.bak
    Backend->>Backend: 2. Dừng mềm toàn bộ run_session & child processes
    Backend->>SQLite: 3. Đóng toàn bộ connection pool
    Backend->>Backend: 4. Xóa app.db, app.db-wal, app.db-shm
    Backend->>Backend: 5. Copy tệp sao lưu mới vào vị trí app.db
    Backend->>SQLite: 6. Mở lại kết nối & chạy test query
    alt Mở DB mới thành công
        Backend-->>UI: Thông báo Khôi phục thành công -> Reload State
    else Mở DB mới thất bại / File lỗi
        Backend->>Backend: ROLLBACK: Khôi phục lại app.db.bak
        Backend->>SQLite: Mở lại kết nối cũ
        Backend-->>UI: Báo lỗi & xác nhận đã Rollback an toàn
    end
```

### 6.3. Luồng Mở Terminal SSH

```mermaid
sequenceDiagram
    actor U as Người dùng
    participant UI as Workspace / SCR-07
    participant R as Rust (SshManager)
    participant S as Server
    U->>UI: Bấm [⌨] hoặc Ctrl+Shift+H → chọn host
    UI->>R: ssh_terminal_open(host_id, cols, rows)
    R->>S: TCP + trao đổi khóa
    alt Host key lạ hoặc đã đổi
        R-->>UI: ssh-hostkey-request
        UI->>U: MOD-18
        U->>UI: Tin cậy / Hủy
        UI->>R: ssh_hostkey_respond
    end
    R->>S: agent → khóa → mật khẩu đã lưu (keyring / SQLite)
    alt Cần nhập mật khẩu / OTP
        R-->>UI: ssh-auth-request
        UI->>U: MOD-19
        UI->>R: ssh_auth_respond(answers, remember)
    end
    R->>S: channel session + pty-req + shell
    R-->>UI: TerminalInfo { run_event_id: ssh:… }
    S-->>R: output
    R-->>UI: pty-data (ring buffer + backpressure dùng chung)
```

### 6.4. Luồng Kéo Thả Truyền File

```mermaid
sequenceDiagram
    actor U as Người dùng
    participant A as Files pane nguồn / OS
    participant B as Files pane đích / Terminal SSH
    participant R as Rust (TransferQueue)
    U->>A: Chọn tệp, kéo > 4px
    A->>B: Tô sáng đích, nhãn "Tải lên → host:/path (n tệp)"
    U->>B: Thả (Shift = di chuyển)
    B->>R: transfer_start(sources, dest, mode)
    R-->>B: job_id → ngăn Transfers / status bar ⇅
    R->>R: Quét → tổng số tệp & dung lượng
    loop Từng tệp (3 song song / kết nối)
        alt Đích đã tồn tại
            R-->>B: transfer-conflict → MOD-20
            B->>R: transfer_resolve(action, apply_to_rest)
        end
        R->>R: Ghi .cm-part → rename
        R-->>B: transfer-progress (≤ 4 lần/giây)
    end
    R-->>B: transfer-done → làm mới danh sách đích, toast
    opt Đích là terminal SSH
        B->>B: Chèn đường dẫn ở xa (đã quote) vào dòng lệnh
    end
```

---

## 7. Hệ thống Design Tokens & Hướng dẫn Triển khai Giao diện

### 7.1. Bảng Màu Tiêu Chuẩn (Color Palette & Tokens)

Hệ màu tối ưu cho Dark Theme với phong cách kỹ thuật cao cấp:

```css
:root {
  /* Màu Thương hiệu & Điểm nhấn Chính (Primary Brand & Accents) */
  --primary: #744791;              /* Màu chủ đạo chính (Royal Purple/Violet) */
  --primary-hover: #8956aa;        /* Hover sáng hơn cho button/link */
  --primary-active: #5f3977;       /* Trạng thái pressed / active */
  --primary-subtle: rgba(116, 71, 145, 0.15); /* Nền mờ cho item được chọn, active tab */
  --primary-glow: rgba(116, 71, 145, 0.35);   /* Hiệu ứng focus ring / glow */

  /* Nền và Bề mặt (Dark Theme Canvas) */
  --bg-app-base: #0f1117;          /* Nền ngoài cùng của app shell */
  --bg-sidebar: #141721;           /* Nền thanh điều hướng phụ */
  --bg-surface: #1a1e2b;           /* Nền card, dockview tab container */
  --bg-surface-hover: #23283a;     /* Trạng thái hover của item */
  --bg-terminal: #0b0d13;          /* Nền đen sâu chuẩn cho xterm viewport */

  /* Đường viền & Phân tách (Borders & Dividers) */
  --border-subtle: #242a3e;        /* Đường viền nhẹ ngăn cách panel */
  --border-focus: #744791;         /* Viền khi focus / active tab (đồng bộ theo Primary) */

  /* Chữ & Biểu tượng (Typography) */
  --text-primary: #f1f5f9;         /* Chữ sáng chính (Slate 100) */
  --text-secondary: #94a3b8;       /* Chữ chú thích, nhãn phụ (Slate 400) */
  --text-muted: #64748b;           /* Chữ mờ, phím tắt (Slate 500) */

  /* Trạng thái Tiến trình (Semantic Process States) */
  --status-idle: #64748b;          /* Chưa chạy: Slate */
  --status-running: #10b981;       /* Đang chạy: Emerald 500 */
  --status-starting: #f59e0b;      /* Đang khởi tạo: Amber 500 */
  --status-success: #06b6d4;       /* Thành công: Cyan 500 */
  --status-failed: #ef4444;        /* Lỗi / Crash: Rose 500 */
  --status-warning: #f97316;       /* Cảnh báo / Untrusted: Orange 500 */

  /* Phông chữ Kỹ thuật */
  --font-sans: 'Inter', -apple-system, BlinkMacSystemFont, sans-serif;
  --font-mono: 'JetBrains Mono', 'Fira Code', Menlo, Consolas, monospace;
}
```

### 7.2. Danh mục Phím Tắt Toàn Cục (Global Keyboard Shortcuts)

> **Lưu ý:** Đây là phím **mặc định**. Người dùng có thể đổi tại SCR-05 → tab Phím Tắt (mục 3). Khi triển khai, bảng này phải khớp với registry `src/keybindings/registry.ts` (xem SCR-05 mục 3.8 về các dòng hiện không có trong code).

| Phím tắt | Tác vụ (Action) | Phạm vi áp dụng |
| :--- | :--- | :--- |
| `Ctrl + K` / `Cmd + K` | Mở Command Palette tìm kiếm nhanh lệnh / nhóm / template | Toàn bộ ứng dụng |
| `Ctrl + 1` .. `Ctrl + 6` | Chuyển nhanh giữa 6 màn hình chính (`/workspace`, `/commands`, `/templates`, `/groups`, `/history`, `/settings`) | Toàn bộ ứng dụng |
| `Ctrl + \` / `Cmd + \` | Bật / Tắt nhanh Split View (Split Right) | Màn hình Workspace (SCR-01) |
| `Ctrl + Shift + \` | Đổi hướng chia đôi Dọc ⇄ Ngang (Toggle Horizontal / Vertical Split) | Màn hình Workspace (SCR-01) |
| `Ctrl + Alt + Left / Right` | Chuyển focus giữa Khung Trái (Pane A) và Khung Phải (Pane B) | Màn hình Workspace (SCR-01) |
| `Ctrl + Alt + Up / Down` | Chuyển focus giữa Khung Trên (Pane A) và Khung Dưới (Pane B) | Màn hình Workspace (SCR-01) |
| `Ctrl + Alt + M` | Chuyển tab đang active sang Pane đối diện (Move to Opposite Pane) | Màn hình Workspace (SCR-01) |
| `Ctrl + Alt + R` | Đặt lại tỉ lệ chia đôi về trạng thái cân bằng chuẩn 50:50 | Màn hình Workspace (SCR-01) |
| `Ctrl + N` | Mở terminal mới vào khung đang focus, cùng thư mục làm việc (cwd do shell báo qua OSC 633 `P;Cwd`) với terminal đang active; thư mục không còn tồn tại thì dùng thư mục mặc định | Màn hình Workspace (kể cả khi đang gõ trong terminal) |
| `Ctrl + W` | Ẩn tab Terminal hiện tại (không dừng process) | Màn hình Workspace |
| `Ctrl + Shift + W` | Dừng tiến trình hiện tại và đóng tab | Màn hình Workspace |
| `Ctrl + Shift + C` | Sao chép vùng chọn trong xterm.js | Tab Terminal |
| `Ctrl + Shift + V` | Dán văn bản vào xterm.js (gửi stdin tới PTY) | Tab Terminal |
| `Ctrl + R` | Khởi động lại (Restart) lệnh của tab hiện hành | Tab Terminal |
| `Ctrl + Shift + S` | Mở màn hình Cài đặt & Sao lưu | Toàn bộ ứng dụng |
| `Ctrl + 7` | Đi đến Máy chủ SSH (`/servers`) | Toàn bộ ứng dụng |
| `Ctrl + Shift + H` | Kết nối SSH nhanh (Palette lọc host) | Toàn bộ ứng dụng |
| `Ctrl + Shift + J` | Mở / đóng ngăn Transfers | Toàn bộ ứng dụng |
| `Ctrl + Shift + E` | Mở Files pane theo ngữ cảnh (host + cwd của terminal đang focus) | Màn hình Workspace |
| `F2` / `F5` / `Delete` / `Ctrl+Shift+N` / `Ctrl+H` | Đổi tên / Làm mới / Xóa / Tạo thư mục / Hiện file ẩn | Files pane |
| `Esc` | Đóng bất kỳ Modal hoặc Drawer đang mở | Modal / Overlay |
| `Nháy đúp tab (Double Click)` | Mở modal MOD-14 đổi tên terminal (chỉ áp dụng cho tab mở thủ công) | Header Tab Strip |

---

## 8. Đối chiếu Tiêu chí Nghiệm thu (UI Definition of Done)

- [x] Đầy đủ 6 màn hình chức năng chính khớp 100% với kiến trúc `plan.md` và tiêu chí `checklist.md` (bao gồm SCR-06 Command Templates).
- [x] Thể hiện trực quan nguyên tắc cốt lõi: Đóng tab ≠ Dừng tiến trình; chỉ dừng khi bấm nút Stop hoặc thoát app.
- [x] Có giao diện chi tiết phân biệt `is_shell` (Direct Argv vs Shell).
- [x] Thể hiện quản lý `execution_order` bằng danh sách có thể sắp xếp trực quan.
- [x] Đặc tả giao diện SOẠN và CHẠY Command Template (SCR-06, MOD-10, MOD-11) với Form động, xem trước live preview, preset và bảo vệ tham số Secret.
- [x] Thiết kế tương tác nháy đúp đổi tên tab terminal thủ công (MOD-14: Rename Manual Terminal Modal) cho phép tùy biến nhãn tab trong phiên làm việc mà không ảnh hưởng tiến trình PTY ngầm.
- [x] Đặc tả hoàn chỉnh **Chế độ Split View Terminal Mode trong SCR-01 (chia đôi màn hình n tab PTY)**: hỗ trợ chia dọc (Left-Right) và chia ngang (Top-Bottom), thanh phân cách điều chỉnh kích thước Resizable Sash với giới hạn an toàn min-width/height, Dual Tab Strips độc lập, kéo thả tab liên khung (Cross-Pane Drag & Drop), tự động thu gọn (Auto-Collapse on Empty) và cơ chế ResizeObserver + debounce IPC `pty_resize` độc lập cho từng terminal.
- [x] Mô hình hóa quy trình Sao lưu (VACUUM INTO) và Khôi phục 7 bước kèm Rollback an toàn.
- [x] Thiết kế cảnh báo an toàn cho vùng tin cậy (Trusted Zone) và kiểm soát quyền đặc quyền.
- [x] Tích hợp chuẩn thiết kế xterm.js và Dockview cho trải nghiệm PTY chân thực.
- [ ] SSH (SCR-07, MOD-17/18/19): host profile trong SQLite, lưu mật khẩu tùy chọn (không lưu / OS keyring / SQLite mã hóa AES-GCM với khóa không theo bản sao lưu), xác minh host key TOFU chặn cứng khi khóa đổi, terminal SSH dùng chung ring buffer / reattach / tiling với terminal local, banner mất kết nối và kết nối lại.
- [ ] Kéo thả truyền file (SCR-08, MOD-20, ngăn Transfers): Files pane là loại tab trong tiling; kéo giữa local ↔ server ↔ server, từ OS vào, thả vào terminal SSH; ghi qua `.cm-part`; xử lý xung đột; tiến trình và hủy từng tệp.
- [ ] Trang chỉnh sửa phím tắt kiểu VS Code (SCR-05 tab Phím Tắt + MOD-16): registry + dispatcher duy nhất, chỉ lưu override vào `app_setting.keybindings`, phát hiện xung đột, bỏ qua sự kiện IME, nhãn gợi ý phím động, Xuất/Nhập JSON.

