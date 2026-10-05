# Đặc Tả Kiến Trúc & Thiết Kế Chia Màn Hình & Panel Theo Mô Hình Tilix (Tilix-Style Screen & Panel Tiling Specification)

> **Dự án:** Ứng dụng Desktop Quản lý, Thực thi Lệnh và Terminal Đa nhiệm (Command Manager Desktop)  
> **Tài liệu tham chiếu:** [docs/plan.md](./plan.md) | [docs/screens.md](./screens.md) | [docs/checklist.md](./checklist.md)  
> **Nguồn cảm hứng thiết kế:** [Tilix GTK3 Tiling Terminal Emulator](https://github.com/gnunn1/tilix) (Gerald Nunn)  
> **Nền tảng công nghệ:** Tauri v2 (Rust Backend + Tokio PTY) + Vue 3 + TypeScript + Tailwind CSS + xterm.js + SQLite WAL

---

## 1. Nghiên cứu & Phân tích Kiến trúc Tilix (Comparative Study & Core Principles)

Tilix (tiền thân là Terminix) là một trong những ứng dụng Terminal Emulator dạng chia lát gạch (tiling) tối ưu và được đánh giá cao nhất trên hệ điều hành Linux. Thay vì giới hạn người dùng trong một cấu hình chia đôi cứng nhắc (Dual-Pane A/B thông thường), Tilix xây dựng một hệ thống phân chia không gian làm việc cực kỳ linh hoạt và giàu tính năng.

### 1.1. Bảng So sánh Kiến trúc: Cũ vs Mô hình Tilix Mới

| Tiêu chí | Command Manager Hiện tại (Dual-Pane v1) | Kiến trúc Tilix Tiling Đề xuất (v2) | Lợi ích Mang lại |
| :--- | :--- | :--- | :--- |
| **Cấu trúc Không gian (Spatial Model)** | Cứng nhắc 2 khung: `paneA` & `paneB` (chỉ chia 1 lần dọc hoặc ngang) | Cây nhị phân đệ quy (**Recursive Binary Tree Tiling**) không giới hạn cấp độ | Chia 1, 2, 3, 4, N panel tùy ý; tạo layout 2x2, 1 trái + 2 phải, 3 cột, v.v. |
| **Tổ chức Đa Không gian (Multi-Workspace)** | Chỉ 1 Workspace duy nhất, các lệnh dồn vào tab strip của Pane A hoặc B | Khái niệm **Sessions** (Phiên làm việc độc lập) kèm thanh Session Drawer / Tabs | Mỗi dự án/stack có 1 Session riêng, chuyển đổi mượt mà không làm lẫn lộn terminal |
| **Thanh Tiêu đề Panel (Per-Pane Chrome)** | Chỉ có tab strip ở đỉnh mỗi pane; terminal không có header riêng | Mỗi panel sở hữu một **Mini-Headerbar (28px)** tích hợp controls ngữ cảnh | Nút chia nhanh (Split Right/Down), phóng to (Zoom), đồng bộ (Sync) ngay trên từng ô |
| **Thu phóng Tạm thời (Pane Zoom / Maximize)** | Không hỗ trợ; muốn xem to phải kéo thanh sash hoặc tắt pane kia | Hỗ trợ **Zoom / Unzoom (Ctrl+Shift+Z)**: 1 click phóng to 100%, click lại trả về layout gốc | Tập trung debug log mà không làm hỏng layout chia lưới phức tạp đã dựng |
| **Đồng bộ Bàn phím (Synchronized Input)** | Không có; phải copy-paste hoặc click từng terminal để gõ | **Broadcast Input**: Gõ phím đồng thời vào tất cả terminal trong Session hoặc Nhóm gán cờ | Cập nhật đồng loạt N server, SSH cluster, tail log song song chỉ với 1 lần gõ |
| **Kéo thả Tái cấu trúc (Drag & Drop)** | Kéo tab giữa 2 thanh tab strip Pane A và B | Kéo panel thả vào 4 cạnh (Top/Bottom/Left/Right/Center) để tái cấu trúc cây nhị phân | Tổ chức lại cửa sổ trực quan như VSCode / Tilix / Dockview |
| **Bố cục Mẫu (Layout Presets)** | Chỉ có nút chuyển Single / Dọc / Ngang | Thư viện **Layout Presets** (Single, 2 Cols, 2 Rows, 2x2 Grid, 1+2 Main/Sub, 3 Cols, Custom) | 1 click dựng ngay môi trường Fullstack (1 API lớn + 1 DB nhỏ + 1 Web nhỏ) |
| **Điều hướng Không gian (Spatial Navigation)** | Click chuột hoặc phím tắt tuần tự | Phím tắt điều hướng không gian: `Alt + Mũi tên` (Trái/Phải/Lên/Xuống) | Điều hướng bàn phím cực nhanh không cần rời tay khỏi phím |

---

## 2. Kiến trúc Hệ thống Phân Cấp 4 Tầng & Mô hình Dữ liệu (4-Level Hierarchy Architecture)

Kiến trúc không gian làm việc của ứng dụng được xây dựng theo mô hình phân cấp 4 tầng chặt chẽ:
```
[Windows] (Nhiều cửa sổ / Không gian Workspace)
   └── [Panels] (Cây nhị phân phân chia đệ quy Tiling Tree trong từng Window)
         └── [Tabs] (Mỗi Panel sở hữu thanh Tab Bar riêng chứa nhiều tab)
               └── [Terminal] (Mỗi Tab gắn với một instance XtermPane độc lập)
```

### 2.1. Cấu trúc Cây Nhị phân Phân chia (Binary Tree Tiling Engine) & Kế Thừa CWD

Không gian làm việc trong mỗi Window được biểu diễn bằng một cây nhị phân (Binary Tree):
- **`PanelNode` (Nút Lá - TilingPanelNode):** Đại diện cho một Panel chứa danh sách các Tab terminal (`tabIds: string[]`) và tab đang hiển thị (`activeTabId: string`).
- **`SplitNode` (Nút Phân Nhánh - TilingSplitNode):** Đại diện cho một container chia đôi, xác định hướng chia (`horizontal` hoặc `vertical`), tỉ lệ chia (`ratio: 0.15 - 0.85`), và 2 node con (`firstChild`, `secondChild`).

**Quy tắc Kế Thừa Thư Mục Làm Việc (Working Path / CWD Inheritance):**
Khi người dùng bấm **Split Right**, **Split Down** hoặc nhấn **[+] Mở tab mới trong panel**:
1. Hệ thống truy vấn trực tiếp thuộc tính `cwd` của tab đang active tại Panel mục tiêu (được cập nhật thời gian thực từ shell kernel thông qua mã điều khiển OSC 633 `P;Cwd=<path>`).
2. Giá trị `cwd` này được truyền trực tiếp vào lời gọi IPC `ipcClient.openTerminal(cwd)`.
3. Rust backend (`terminal_open_sync`) khởi tạo shell PTY tương tác ngay tại thư mục làm việc đó, bảo đảm ngữ cảnh làm việc không bị gián đoạn.

```
              [Root: SplitNode (Horizontal, 50%)]
                        /            \
                       /              \
       [Panel 1: Dev API]            [SplitNode (Vertical, 60%)]
       - Tab 1: nest start --watch       /            \
       - Tab 2: prisma studio   [Panel 2: Web]      [Panel 3: DB & Worker]
                                - Tab 1: vite       - Tab 1: redis-cli
                                                    - Tab 2: worker-debug
```

### 2.2. TypeScript Interfaces

```typescript
// Định nghĩa kiểu hướng chia (Theo quy ước chuẩn của Tilix & CSS Flex/Grid)
// 'horizontal': Chia đôi trái - phải (tạo đường phân cách dọc)
// 'vertical': Chia đôi trên - dưới (tạo đường phân cách ngang)
export type SplitOrientation = 'horizontal' | 'vertical';

// Chế độ phát sóng gõ phím đồng bộ (Synchronized Input)
export type SyncInputMode = 'off' | 'session' | 'group';

// Dữ liệu của một Terminal Panel cụ thể (Leaf Node)
export interface TerminalPanel {
  id: string;                    // UUID duy nhất của panel (vd: 'pane-8f3a')
  commandId: number;             // ID lệnh trong SQLite hoặc 0 (manual shell)
  name: string;                  // Tên hiển thị (tùy biến hoặc tên lệnh)
  title?: string;                // Tiêu đề động do ứng dụng đặt qua OSC 0/2
  runEventId?: string;           // UUID phiên thực thi
  shellKind?: string;            // 'bash' | 'zsh' | 'powershell' | 'fish'
  cwd?: string;                  // Thư mục làm việc hiện tại
  pid?: number;                  // PID tiến trình thực tế từ Rust backend
  status: 'idle' | 'starting' | 'running' | 'success' | 'failed';
  syncGroup?: string;            // Màu hoặc nhóm đồng bộ phím (vd: 'purple', 'blue')
  isZoomed?: boolean;            // Đang được phóng to chiếm toàn bộ layout
  createdAt: number;
}

// Cấu trúc Nút trong Cây Nhị phân Tiling
export type TilingNode = TilingSplitNode | TilingLeafNode;

export interface TilingLeafNode {
  type: 'leaf';
  id: string;                    // Trùng với TerminalPanel.id
}

export interface TilingSplitNode {
  type: 'split';
  id: string;                    // UUID của split container
  orientation: SplitOrientation; // 'horizontal' | 'vertical'
  ratio: number;                 // Tỉ lệ phần trăm vị trí phân cách (mặc định 0.5 = 50:50, min 0.15, max 0.85)
  firstChild: TilingNode;        // Panel bên trái (nếu horizontal) hoặc bên trên (nếu vertical)
  secondChild: TilingNode;       // Panel bên phải (nếu horizontal) hoặc bên dưới (nếu vertical)
}

// Session (Không gian làm việc chứa 1 layout cây tiling hoàn chỉnh)
export interface WorkspaceSession {
  id: string;                    // UUID session (vd: 'session-dev-stack')
  name: string;                  // Tên Session (vd: 'Web Platform Dev')
  icon?: string;                 // Icon đại diện
  rootNode: TilingNode | null;   // Cây phân chia giao diện của session
  panels: Record<string, TerminalPanel>; // Bảng tra cứu panel theo ID
  activePanelId: string | null;  // Panel đang nhận tiêu điểm (focus)
  zoomedPanelId: string | null;  // Panel đang được zoom full screen (nếu có)
  syncMode: SyncInputMode;       // Chế độ broadcast bàn phím của session này
}
```

---

## 3. Đặc tả Thiết kế Trải nghiệm Người dùng (UI/UX Specification)

### 3.1. Bố cục Tổng thể Màn hình Terminal Workspace (Tilix Model)

```
+-------------------------------------------------------------------------------------------------------------------------+
| [Icon] Command Manager v1.1.0        [Search Commands / Groups... Ctrl+K]                         4.2MB RAM | WAL  [_][□][X]|
+-------------------------------------------------------------------------------------------------------------------------+
| NAV | SESSIONS DRAWER | WORKSPACE TOOLBAR (Layout Presets, Sync Broadcast, Batch Actions)                               |
| BAR | (Collapsible)   | [Layout: [⊞ 1+2 Tiled ▾]] [🔗 Sync: Session [ON]] [▷ Run All] [⏹ Stop All]     [+ New Tile] [⚡ 3 Ngầm]|
|-----|-----------------|-------------------------------------------------------------------------------------------------|
| [W] | ≡ SESSIONS  [+] | +-------------------------------------------+-------------------------------------------------+ |
| [C] |-----------------| | PANEL 1: api-server (PID: 3102)           | PANEL 2: vite-dev (PID: 4091) [🔗 SYNC]         | |
| [T] | ⊞ Dev Stack (3) | | [● Running] · ttys004 · ~/repo/api        | [● Running] · ttys005 · ~/repo/web              | |
| [G] |   [ 1 lớn + 2 ] | | [◫ Split R] [⬒ Split D] [🗖 Zoom] [⏹] [✕] | [◫ Split R] [⬒ Split D] [🗖 Zoom] [⏹] [✕]       | |
| [H] |   ● 3 running   | |-------------------------------------------+-------------------------------------------------+ |
| [S] |                 | | [NestFactory] Starting Nest application...| VITE v5.4.2 ready in 280 ms                     | |
|     | ⊟ Microservices | | [InstanceLoader] DatabaseModule init +36ms| ➜ Local:   http://localhost:5173/               | |
|     |   [ 2x2 Grid ]  | | [DB] Connected to PostgreSQL pool :5432   | ➜ Network: http://192.168.1.15:5173/            | |
|     |   ○ Idle        | | ➜ cm-api git:(main) $                     | ➜ press h + enter to show help                  | |
|     |                 | |                                           |====================== SASH PHÂN CÁCH NGANG =====| |
|     | ◫ Docker Infra  | |                                           | PANEL 3: redis-worker (PID: 4120) [🔗 SYNC]     | |
|     |   [ 2 Cột Dọc ] | |                                           | [● Running] · ttys006 · ~/repo                  | |
|     |   ● 1 running   | |                                           | [◫ Split R] [⬒ Split D] [🗖 Zoom] [⏹] [✕]       | |
|     |                 | |                                           |-------------------------------------------------+ |
|     |                 | | <== Viền Focus Tím #744791 active ==>     | 1:M 28 Sep 10:15:01 * Ready to accept connection| |
+-----+-----------------+---------------------------------------------+-------------------------------------------------+ |
| STATUS BAR: ● 3 running | Ring Buffer: 680KB/2MB | Tiling: 3 Panes (1 Main + 2 Sub) | Sync Input: Active (2 panes linked)       |
+-------------------------------------------------------------------------------------------------------------------------+
```

### 3.2. Thanh Công cụ Workspace (Top Workspace Ribbon Toolbar - 38px)

Thanh công cụ đỉnh workspace chứa các điều khiển cấp cao nhất cho phiên làm việc hiện tại:

1. **Bộ chọn Bố cục Mẫu (Layout Presets Dropdown):**
   - Icon đại diện trực quan kèm menu popup cho phép thay đổi cấu trúc lưới nhanh:
     - `[ ◻ ] Single Pane`: 1 màn hình duy nhất (100%).
     - `[ ◫ ] 2 Columns`: 2 cột song song (50:50).
     - `[ ⬒ ] 2 Rows`: 2 hàng trên-dưới (50:50).
     - `[ ⊞ ] 2x2 Grid`: Lưới 4 ô vuông cân xứng.
     - `[ ◨ ] 1 Main Left + 2 Stacked Right (1+2 Tiled)`: Layout lập trình viên kinh điển của Tilix (Khung chính bên trái 60%, 2 khung phụ bên phải mỗi khung 50% chiều cao).
     - `[ ⬓ ] 1 Top Main + 2 Bottom Stacked`: Khung chính ở trên (ví dụ chạy log stream/htop), 2 khung gõ lệnh ở dưới.
     - `[ ⚏ ] 3 Columns`: 3 cột song song (33:33:33) cho giám sát microservices.
     - `[ 💾 ] Lưu Layout Hiện Tại...`: Lưu cấu trúc phân nhánh hiện tại thành template preset cá nhân.

2. **Bộ điều khiển Đồng bộ Bàn phím (Synchronized Input Controller):**
   - Nút bật/tắt chế độ phát sóng bàn phím với 3 mức độ:
     - `Off (Độc lập)`: Gõ vào đâu chỉ terminal đó nhận dữ liệu.
     - `Session (Toàn bộ phiên)`: Gõ vào 1 ô, tất cả các panel trong session hiện tại đều nhận chuỗi phím giống hệt nhau cùng lúc.
     - `Group (Theo nhóm)`: Chỉ phát sóng tới các panel có cùng huy hiệu màu (Group Tím, Group Xanh).
   - Khi bật, thanh công cụ hiển thị banner cảnh báo nhẹ: `🔗 ĐANG PHÁT SÓNG PHÍM: 2 terminals được liên kết`.

3. **Điều khiển Thực thi Hàng loạt (Batch Actions):**
   - `[▷ Chạy Lại Tất Cả]`: Kích hoạt lại toàn bộ các lệnh đang dừng trong session.
   - `[⏹ Dừng Tất Cả]`: Gửi SIGTERM tới toàn bộ tiến trình con của session.

4. **Quản lý Tiến trình Chạy Ngầm & Nút Thêm Nhanh:**
   - Huy hiệu `[⚡ 3 Ngầm]`: Mở Drawer quản lý tiến trình nền.
   - Nút `[+ Thêm Tile Mới]`: Tự động tìm panel đang focus và chia đôi theo hướng tối ưu không gian nhất (ví dụ nếu width > height thì chia ngang, ngược lại chia dọc).

---

### 3.3. Chi tiết Mini-Headerbar trên Từng Terminal Panel (28px Per-Pane Chrome)

Mỗi terminal tile được bao bọc bởi một header thanh mảnh 28px nằm ngay trên viewport xterm, đóng vai trò là "trung tâm chỉ huy" cục bộ của panel đó:

```
+----------------------------------------------------------------------------------------------------------+
| [●] api-server · PID: 3102  [🔗 SYNC]  ttys004  ~/projects/api     [◫ Chia Phải] [⬒ Chia Dưới] [🗖] [⏹] [✕] |
+----------------------------------------------------------------------------------------------------------+
```

* **Thành phần Bên trái (Telemetry & Info):**
  - **Dot Trạng thái (Status Pulse):**
    - 🟢 Xanh lá: Đang chạy (Active PTY streaming).
    - 🟡 Vàng cam: Đang khởi chạy hoặc PTY hook.
    - ⚪ Xám: Đã kết thúc (Exit code 0).
    - 🔴 Đỏ: Lỗi hoặc crash (Exit code != 0).
  - **Tên Terminal & Title Động:** Hiển thị tên gán (ví dụ `api-server`), nháy đúp (`dblclick`) vào tên để mở modal đổi tên nhanh (MOD-14). Nếu có tiêu đề ứng dụng (OSC 0/2 từ `vim`, `htop`, `claude`), hiển thị trong dấu ngoặc hoặc tooltip.
  - **Huy hiệu PID:** Phông `JetBrains Mono` 10px nền tối.
  - **Huy hiệu [🔗 SYNC]:** Hiện rõ ràng khi panel này đang tham gia nhóm gõ phím đồng bộ.
  - **Thư mục CWD:** Hiển thị đường dẫn rút gọn (vd: `~/repo/api`).

* **Thành phần Bên phải (Quick Action Icons):**
  - **`[◫]` Split Right (Ctrl+Alt+R):** Chia panel hiện tại làm đôi theo cột dọc; panel mới nằm ngay bên phải và thừa hưởng thư mục CWD hiện tại.
  - **`[⬒]` Split Down (Ctrl+Alt+D):** Chia panel hiện tại làm đôi theo hàng ngang; panel mới nằm bên dưới.
  - **`[🗖]` Zoom / Maximize (Ctrl+Shift+Z):** Phóng to panel này chiếm 100% diện tích làm việc của session. Các panel khác tạm thời bị ẩn. Nhấn lần nữa (icon chuyển thành `[🗗] Unzoom`) để khôi phục chính xác vị trí trong cây nhị phân.
  - **`[⏹]` Stop Process:** Gửi tín hiệu SIGTERM tới PID riêng của terminal này.
  - **`[✕]` Close / Detach Pane:** Đóng panel. Vùng không gian bị trống sẽ tự động được thu hồi và gộp vào panel anh em lân cận (sibling node) trong cây nhị phân. Tiến trình nếu chạy ngầm vẫn tiếp tục duy trì trong background daemons.

---

### 3.4. Thanh Ngăn Cách Đa Hướng (Interactive Multi-Axis Resizable Sashes)

Khác với thanh sash đơn thuần ở chế độ Dual-Pane, hệ thống Tiling quản lý nhiều thanh phân cách (Horizontal Sashes và Vertical Sashes):

- **Kích thước & Visual:** Dày 5px, màu viền `--border-subtle` (`#242a3e`).
- **Hiệu ứng Hover & Drag:** Khi di chuột hoặc đang kéo, sash sáng rực màu chủ đạo `--primary: #744791` kèm hiệu ứng glow nhẹ (`rgba(116, 71, 145, 0.4)`).
- **Ràng buộc An toàn:** Mỗi panel được bảo vệ với kích thước tối thiểu **min-width: 200px** và **min-height: 120px**. Thanh sash sẽ tự động dừng lại nếu kéo vượt quá ngưỡng giới hạn này để bảo vệ bộ vẽ ký tự của xterm.js.
- **Nháy đúp Chuột (Double-Click Reset):** Nháy đúp vào bất kỳ thanh phân cách nào sẽ cân bằng lại tỉ lệ của 2 nhánh con thuộc node cha đó về đúng **50:50**.
- **Lớp chắn Sự kiện Chuột (Pointer-Events Guard):** Trong suốt quá trình kéo thanh sash, một overlay trong suốt (`pointer-events: all`) sẽ phủ lên toàn bộ các terminal iframe/canvas để tránh việc con trỏ chuột bị xterm "nuốt" mất sự kiện `mousemove` và `mouseup`.

---

### 3.5. Thanh Bên Quản lý Phiên Làm Việc (Sessions Drawer / Sidebar)

Lấy cảm hứng từ Session Drawer của Tilix, thanh bên trái có thể chuyển đổi linh hoạt giữa:
1. **Group Tree Explorer (Mặc định):** Quản lý cây nhóm lệnh và Quick Access.
2. **Sessions Manager (Tilix Session Drawer):** Hiển thị danh sách các Phiên làm việc (Sessions):
   - Mỗi Session Card hiển thị:
     - Tên Session (ví dụ: `Dev Stack`, `Docker Services`, `Prod Logs Monitoring`).
     - Sơ đồ thu nhỏ của Layout (Mini layout wireframe icon: icon 1+2, icon 2x2, icon 3 cột).
     - Số lượng tile terminal đang hoạt động và số tiến trình running.
     - Nút đóng session hoặc đổi tên session.
   - Thao tác kéo thả: Người dùng có thể kéo một panel từ Session này thả sang Session khác một cách tiện lợi.

---

### 3.6. Thuật toán Phóng to Panel (Zoom / Maximize Logic)

Khi người dùng làm việc trong một layout phức tạp (ví dụ 4 terminal 2x2) và cần mở rộng một terminal để đọc log dài hoặc tương tác với `htop`/`vim`:
1. Người dùng click nút `[🗖 Zoom]` trên header của panel hoặc nhấn phím tắt `Ctrl+Shift+Z`.
2. Hệ thống đặt `zoomedPanelId = targetPanel.id` trong state của Session.
3. Không làm thay đổi hay phá hủy cấu trúc cây nhị phân `rootNode`.
4. Giao diện ẩn tất cả các nhánh khác và render duy nhất viewport của `targetPanel` với kích thước 100% không gian workspace.
5. Trên header của panel hiển thị huy hiệu sáng nổi bật: `[🗗 ĐANG PHÓNG TO TOÀN MÀN HÌNH - Nhấn Ctrl+Shift+Z để quay lại layout]`.
6. Khi người dùng click unzoom hoặc nhấn `Ctrl+Shift+Z` lần nữa: `zoomedPanelId` được gán về `null`, layout cây tiling lập tức hiển thị lại nguyên vẹn từng vị trí, đồng thời kích hoạt sự kiện `fitAddon.fit()` trên từng panel để căn chỉnh lại dòng cột terminal.

---

### 3.7. Đồng bộ Bàn phím Thực tế (Synchronized Keystroke Broadcasting)

Cơ chế gõ phím đồng bộ của Tilix là tính năng vô cùng hữu ích cho DevOps / Sysadmin:
1. Khi một terminal panel đang nhận focus và chế độ `syncMode` đang kích hoạt (`session` hoặc `group`):
2. Mỗi ký tự hoặc chuỗi escape sequence người dùng gõ vào bàn phím (sự kiện `onData` của xterm.js) sẽ được bắt tại terminal đang active.
3. `useTilingLayout` sẽ duyệt danh sách các panel hợp lệ:
   - Nếu `syncMode === 'session'`: Tất cả các panel đang có tiến trình running trong session hiện tại.
   - Nếu `syncMode === 'group'`: Các panel có cùng `syncGroup` (ví dụ cùng màu Tím hoặc Xanh).
4. Hệ thống gọi IPC `pty_write` hoặc gửi trực tiếp dữ liệu tới stream stdin của các tiến trình PTY tương ứng ở Rust backend.
5. Cả 2 hoặc N màn hình terminal đều phản hồi và in ra cùng một câu lệnh đồng thời trong thời gian thực.
6. **Bảo vệ An toàn:** Khi một panel trong nhóm đồng bộ bị đóng hoặc tiến trình kết thúc, panel đó tự động tách khỏi nhóm broadcast để tránh gửi phím thừa vào tiến trình rỗng.

---

## 4. Hệ thống Phím Tắt Toàn Cục (Keyboard Shortcuts Matrix)

| Thao tác | Phím tắt Chuẩn | Mô tả Hành vi |
| :--- | :--- | :--- |
| **Chia đôi sang phải** | `Ctrl + Alt + R` | Chia terminal đang focus làm đôi theo chiều dọc, mở terminal mới bên phải |
| **Chia đôi xuống dưới** | `Ctrl + Alt + D` | Chia terminal đang focus làm đôi theo chiều ngang, mở terminal mới bên dưới |
| **Thu phóng Panel (Zoom)** | `Ctrl + Shift + Z` | Phóng to 100% panel đang focus / Khôi phục lại bố cục lưới cũ |
| **Đổi chế độ Đồng bộ Phím** | `Ctrl + Alt + S` | Bật/tắt phát sóng bàn phím đồng bộ (Off ⇄ Session ⇄ Group) |
| **Di chuyển Focus Sang Trái** | `Alt + Phím Mũi tên Trái` | Chuyển tiêu điểm bàn phím sang panel liền kề bên trái |
| **Di chuyển Focus Sang Phải** | `Alt + Phím Mũi tên Phải` | Chuyển tiêu điểm bàn phím sang panel liền kề bên phải |
| **Di chuyển Focus Lên Trên** | `Alt + Phím Mũi tên Lên` | Chuyển tiêu điểm bàn phím sang panel liền kề bên trên |
| **Di chuyển Focus Xuống Dưới** | `Alt + Phím Mũi tên Xuống` | Chuyển tiêu điểm bàn phím sang panel liền kề bên dưới |
| **Tạo Session Mới** | `Ctrl + Shift + T` | Mở một Phiên làm việc (Session) mới với 1 terminal đơn rỗng |
| **Chuyển Session Tiếp Theo** | `Ctrl + PageDown` | Chuyển sang session bên phải |
| **Chuyển Session Trước Đó** | `Ctrl + PageUp` | Chuyển sang session bên trái |
| **Cân bằng Tỉ lệ Phân cách** | `Ctrl + Alt + 0` (hoặc nháy đúp sash) | Đưa thanh chia phân cách về tỉ lệ chuẩn 50:50 |
| **Đóng Panel Hiện Tại** | `Ctrl + Shift + W` | Đóng panel đang focus và gộp không gian vào panel lân cận |

---

## 5. Kế hoạch Triển khai Chi tiết (Implementation Roadmap & Milestones)

### Giai đoạn 1: Thiết kế & Chuẩn hóa Tài liệu (Hoàn tất)
- [x] Phân tích kiến trúc Tilix GTK3 và đối chiếu với hạn chế của Dual-Pane hiện tại.
- [x] Lập tài liệu đặc tả kiến trúc `docs/tilix-tiling-spec.md`.
- [x] Cập nhật thiết kế chi tiết SCR-01 trong `docs/screens.md`.
- [x] Đẩy thiết kế giao diện lên dự án Stitch `14914224436748087443`.

### Giai đoạn 2: Xây dựng Core Composable `useTilingTree.ts`
- [ ] Định nghĩa các cấu trúc dữ liệu đệ quy `TilingNode`, `TilingSplitNode`, `TilingLeafNode`.
- [ ] Viết các hàm biến đổi cây:
  - `splitNode(tree, targetLeafId, orientation, newLeaf)`: Tìm lá chỉ định, thay thế bằng một SplitNode chứa lá cũ và lá mới.
  - `removeNode(tree, targetLeafId)`: Xóa một lá, tự động nâng cấp lá anh em (sibling) lên thay thế vị trí SplitNode cha.
  - `updateRatio(tree, splitNodeId, newRatio)`: Cập nhật tỉ lệ chia phân cách với chặn ngưỡng min/max.
  - `findAdjacentLeaf(tree, currentLeafId, direction)`: Tính toán hình học không gian (bounding boxes) để tìm panel liền kề theo hướng mũi tên (Left, Right, Up, Down).
- [ ] Hỗ trợ lưu trữ trạng thái cây vào `localStorage` theo từng Session.

### Giai đoạn 3: Phát triển Component Cây Tiling Đệ quy (`TilingContainer.vue` & `TilingPaneChrome.vue`)
- [ ] Xây dựng `TilingContainer.vue` tự gọi đệ quy (recursive component) dựa trên `node.type === 'split' ? TilingContainer : TerminalTile`.
- [ ] Xây dựng `TilingPaneChrome.vue` (Mini-Headerbar 28px) với đầy đủ telemetry, status dot, nút Split Right, Split Down, Zoom, Sync, Close.
- [ ] Tích hợp `ResizeObserver` và `fitAddon.fit()` độc lập trên từng ô lá để đồng bộ kích thước dòng/cột với PTY backend.
- [ ] Xây dựng thanh `TilingSash.vue` hỗ trợ kéo thả chuột mượt mà và nháy đúp chuột đặt lại 50:50.

### Giai đoạn 4: Tích hợp Bộ điều khiển Đồng bộ Phím (Sync Broadcast) & Session Drawer
- [ ] Viết logic đánh chặn phím `xterm.onData` để phát sóng song song tới các instance PTY khi `syncMode !== 'off'`.
- [ ] Xây dựng giao diện visual badge và viền sáng phát xung (pulsing border) cho các terminal tham gia đồng bộ.
- [ ] Xây dựng `SessionsDrawer.vue` hiển thị danh sách các session kèm thumbnail layout mini-map.
- [ ] Thêm menu Layout Presets (1+2, 2x2, 3 cols) trên thanh Workspace Toolbar.

### Giai đoạn 5: Kiểm thử & Tối ưu Hóa Hiệu năng
- [ ] Kiểm tra khả năng render mượt mà khi mở đồng thời 4 đến 6 terminal PTY streaming log tốc độ cao.
- [ ] Đảm bảo cơ chế đóng terminal chỉ thu hồi DOM/bố cục cây mà không làm gián đoạn các daemon đang chạy ngầm trong backend.
- [ ] Kiểm thử toàn diện phím tắt điều hướng không gian (`Alt + Arrows`).
