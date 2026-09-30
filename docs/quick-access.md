# Quick Access — Thiết kế chức năng

> Trạng thái: **Đã triển khai** · Phạm vi: SCR-01 Workspace, SCR-02 Command Library, MOD-02 Command Editor
> Liên quan: [screens.md](./screens.md), [plan.md](./plan.md), [checklist.md](./checklist.md)

## 1. Mục tiêu & Phạm vi

Quick Access là danh sách các lệnh đã lưu (`command_definition`) mà người dùng đánh dấu để gửi nhanh vào **terminal đang focus**, thay vì mở một tab PTY riêng như nút `[▷]` hiện tại.

| Thao tác | Kết quả |
| :--- | :--- |
| **Run** | Dán chuỗi lệnh vào terminal đang focus **và chạy ngay** (gửi Enter). |
| **Paste** | Chỉ dán chuỗi lệnh vào vị trí con trỏ của terminal đang focus, **không chạy**, để người dùng sửa tiếp. |

**Trong phạm vi**
* Cờ `quick_access` cho từng lệnh, bật/tắt tại SCR-02 (switch trên từng dòng) và MOD-02 (switch trong form).
* Section **Quick Access** ở nửa dưới thanh bên "Nhóm Lệnh & Phiên" của SCR-01, mỗi item có nút `Run` và `Paste`.

**Ngoài phạm vi (xem §9)**
* Template có tham số `{{param}}` (MOD-10 vẫn là cách chạy template).
* Sắp xếp thủ công, phím tắt chạy item, hiển thị trong Command Palette.

**Khác biệt với nút `[▷]` của SCR-02:** `[▷]` spawn lệnh thành một tiến trình PTY riêng, có `run_event`, PID và tab riêng. Quick Access **không spawn gì**: nó chỉ ghi văn bản vào stdin của terminal đang có, giống như người dùng tự gõ. Lệnh chạy trong shell, thư mục hiện tại và môi trường của terminal đó.

## 2. Thuật ngữ: "Terminal đang focus"

Khi người dùng bấm nút ở thanh bên, focus DOM rời khỏi xterm. Vì vậy "terminal đang focus" **không** lấy từ `document.activeElement`, mà từ trạng thái của DockHost:

* **Khung đích** = `activePane` (ở chế độ Single luôn là `paneA`).
* **Tab đích** = tab active của khung đích (`activeTabIdA` / `activeTabIdB`).
* **Terminal đích** = instance `XtermPane` của tab đó (registry `xtermPaneRefs` đã có từ tính năng kéo thả file).

Đây cũng là cách Ctrl+N (lấy cwd) và kéo thả file đang xác định terminal đích. Sau khi Run/Paste, focus được trả về terminal đích để người dùng gõ tiếp ngay.

**Section Quick Access chỉ hiển thị khi có terminal đích**, tức khung đang active có ít nhất một tab. Khi Workspace chưa có tab nào, hoặc ở chế độ Split mà khung đang active trống, section ẩn hẳn và GroupTree chiếm toàn bộ thanh bên. Quick Access không bao giờ tự mở terminal mới.

## 3. Thiết kế giao diện

### 3.1. SCR-02 — Command Library: cột & bộ lọc Quick Access

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

* **Cột `⚡ Quick`:** một switch trên mỗi dòng. Bật/tắt có hiệu lực ngay (gọi IPC `commands_set_quick_access`), không cần mở form. Switch bị khoá (loading) trong lúc chờ IPC; nếu lỗi thì trả về trạng thái cũ và hiện toast.
* **Bộ lọc `⚡ Quick Access`:** thêm vào nhóm filter All / Shell / Argv, chỉ hiện các lệnh đang bật.
* **Footer:** thêm bộ đếm `⚡ N Quick Access`.
* Tooltip của switch: *"Hiện lệnh này trong mục Quick Access ở màn Terminal"*.

### 3.2. MOD-02 — Command Editor: switch trong form

Thêm một dòng ngay dưới "Gán vào Nhóm lệnh":

```
|  Gán vào Nhóm lệnh                                                          |
|  [x] Web Platform Development    [ ] Infrastructure    [ ] Microservices     |
|                                                                             |
|  ⚡ Quick Access                                                   [●━]      |
|  Hiện trong mục Quick Access ở màn Terminal để dán hoặc chạy nhanh           |
|  vào terminal đang focus.                                                   |
```

* Mặc định **tắt** khi tạo lệnh mới. Nút "Nhân bản" `[⧉]` giữ nguyên giá trị của lệnh gốc.
* Giá trị được lưu cùng lúc với phần còn lại của form (`commands_create` / `commands_update`).

### 3.3. SCR-01 — Thanh bên "Nhóm Lệnh & Phiên" chia hai section

Thanh bên hiện chỉ có `GroupTree`. Sau thay đổi, nó chia dọc thành hai section có thanh kéo ở giữa:

```
+---------------------------+
| ≡ NHÓM LỆNH & PHIÊN   [⟳] |   <- header hiện có
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

* **Hiển thị:** section 2 và sash chỉ render khi có terminal đích (§2). Khi section ẩn đi rồi hiện lại, tỉ lệ và trạng thái thu gọn trước đó được giữ nguyên.
* **Tỉ lệ:** mặc định 50:50. Mỗi section tối thiểu 120px. Tỉ lệ và trạng thái thu gọn được lưu vào `localStorage`: `cm_quick_access_ratio_v1`, `cm_quick_access_collapsed_v1` (cùng quy ước với `cm_split_mode_v1`).
* **Thu gọn:** khi thu gọn, section 2 chỉ còn header (28px) và GroupTree chiếm phần còn lại.
* **Sắp xếp:** theo tên (A→Z), giống `commands_list`.

### 3.4. Item Quick Access

| Phần | Nội dung |
| :--- | :--- |
| Dòng 1 | Tên lệnh (ellipsis), badge `⚠` nếu có cảnh báo (§6), hai nút icon luôn hiển thị: `▷` **Run** (`Play`) và `⎘` **Paste** (`ClipboardPaste`, lucide). |
| Dòng 2 | `execution_string` dạng mono cỡ nhỏ, một dòng, ellipsis; lệnh nhiều dòng hiện dòng đầu kèm `↵ +N`. |
| Tooltip của item | Chuỗi lệnh đầy đủ và kiểu thực thi (Shell/Argv). |
| Tooltip của nút | `Run: Chạy "<tên>" trong <tên terminal đích>` / `Paste: Dán "<tên>" vào <tên terminal đích>`. |
| `aria-label` | Giống tooltip của nút. |
| Nháy đúp vào item | Tương đương **Paste**: an toàn hơn Run nếu người dùng bấm nhầm. |

### 3.5. Trạng thái của section

| Trạng thái | Hiển thị | Nút Run / Paste |
| :--- | :--- | :--- |
| Chưa có lệnh nào bật Quick Access | Empty state: icon `Zap` + *"Chưa có lệnh Quick Access. Bật switch ⚡ trong màn Lệnh."* + nút `[Mở màn Lệnh]` (router `/commands`). | Không có item |
| Có terminal đích đang chạy | Chỉ báo `→ <tên tab> · Khung A/B`, dùng `tabLabel()` nên tên có thể là tiêu đề chương trình, ví dụ `→ ✳ Claude Code · Khung A`. | Bật |
| Không có terminal đích (Workspace chưa có tab, hoặc khung đang active trống) | **Section ẩn hoàn toàn**, GroupTree chiếm toàn bộ thanh bên | — |
| Tab đích đã dừng tiến trình | Section vẫn hiện; chỉ báo màu cảnh báo `→ <tên tab> (đã dừng)`. | Tắt, tooltip *"Tiến trình của tab đích đã dừng"* |
| Tab đích đang chạy một chương trình (`shellPhase = running`, ví dụ `vite`, `claude`) | Chỉ báo kèm `· đang bận`. | Bật, văn bản đi vào stdin của chương trình đó (§4.2) |

## 4. Hành vi Run / Paste

### 4.1. Cơ chế gửi

Cả hai thao tác đều đi qua **xterm** chứ không ghi thẳng vào PTY, để hành vi giống hệt người dùng tự gõ hoặc dán (cùng lý do như tính năng kéo thả file):

* **Paste** = `term.paste(text)`. xterm tự bọc bracketed paste (`\x1b[200~ … \x1b[201~`) khi chương trình đã bật DECSET 2004. Dữ liệu chạy qua `onData`, nên `inputShadow` (history level 2), cờ `bracketedPaste` và gợi ý đều được cập nhật đúng.
* **Run** = xoá dòng đang gõ (§4.2), rồi `term.paste(text)`, rồi `term.input('\r', true)`. `Terminal.input()` là API công khai của xterm 6; Enter cũng đi qua `onData`, nên lệnh được ghi lịch sử như khi tự gõ (level 1 qua marker `E`, level 2 qua `recordTypedHistory`).

API mới trên `XtermPane` (bổ sung cạnh `pasteText` đã có):

```ts
interface XtermPaneInstance {
  focusTerminal(): void;
  pasteText(text: string): boolean;                  // đã có
  sendCommand(text: string, execute: boolean): SendResult; // mới
  inputState(): { phase: 'prompt' | 'input' | 'running'; hasInput: boolean; cursorAtEnd: boolean };
}
type SendResult = 'sent' | 'stopped' | 'input-not-at-end';
```

### 4.2. Dòng đang gõ dở khi bấm Run

Nếu dòng lệnh đang có chữ, dán rồi Enter sẽ chạy `<chữ cũ><lệnh>`, là kết quả sai. Quy tắc:

| Trạng thái terminal đích | Paste | Run |
| :--- | :--- | :--- |
| Ở prompt (`phase = input`), dòng trống | Dán | Dán + Enter |
| Ở prompt, có chữ, con trỏ ở cuối dòng | Dán **nối vào** vị trí con trỏ | Xoá dòng bằng `DEL × số ký tự` (như `replaceCurrentInput` khi duyệt ↑/↓), rồi dán + Enter |
| Ở prompt, có chữ, con trỏ **không** ở cuối | Dán tại con trỏ | **Không gửi**, toast *"Đưa con trỏ về cuối dòng hoặc xoá dòng đang gõ trước khi Run"* |
| Đang chạy chương trình (`phase = running`) hoặc không có shell integration | Dán | Dán + Enter, **không** xoá gì (app không biết chương trình đó đang có gì trong ô nhập) |

Xoá bằng `DEL × n` thay vì phím tắt kiểu Ctrl+U vì DEL hoạt động giống nhau trên bash, zsh (cả vi mode), PSReadLine và cmd. Ctrl+U thì khác nhau giữa các shell, và Escape trên PSReadLine ở chế độ Emacs không xoá dòng.

### 4.3. Chuẩn hoá văn bản trước khi gửi

* `\r\n` và `\r` được đổi thành `\n` (xterm `paste()` tự đổi `\n` thành `\r` khi gửi).
* Loại bỏ ký tự điều khiển C0/C1 trừ `\t` và `\n`, đặc biệt là `ESC`, để không thể đóng bracketed paste sớm. Chuỗi lệnh do người dùng tự lưu nên đây chỉ là lớp phòng vệ, không phải kiểm tra bảo mật chính.
* Không tự thêm dấu cách hay `\n` ở cuối. Run chỉ thêm đúng một `\r`.
* **Lệnh nhiều dòng:** với shell có bracketed paste (bash ≥ 5.1, zsh, pwsh), cả khối được chèn vào dòng lệnh rồi chạy một lần bằng Enter. Với shell không có bracketed paste (`sh`/`dash`, `cmd`), mỗi `\n` sẽ chạy từng dòng ngay khi dán; Paste khi đó cũng thực chất là chạy. Trường hợp này hiện badge `⚠` (§6).

## 5. Dữ liệu & Backend

### 5.1. Migration `010_command_quick_access.sql`

```sql
BEGIN;

ALTER TABLE command_definition
  ADD COLUMN quick_access INTEGER NOT NULL DEFAULT 0 CHECK (quick_access IN (0, 1));

UPDATE schema_version SET version = 10;

COMMIT;
```

* Đăng ký `MIGRATION_010_SQL` trong `db/schema.rs` và nhánh `10 =>` trong `db/pool.rs::migrate`; nâng `SCHEMA_VERSION`.
* **Sao lưu / khôi phục:** snapshot `VACUUM INTO` tự mang theo cột mới. Khôi phục một snapshot phiên bản 9 phải chạy migrate lên 10 (mặc định `0` cho mọi lệnh). Cần kiểm tra lại luồng `backup::import` với snapshot cũ.

### 5.2. Repository & IPC

| Thành phần | Thay đổi |
| :--- | :--- |
| `db/repos/commands.rs::CommandDef` | Thêm `pub quick_access: bool` với `#[serde(default)]` để payload cũ vẫn parse được. |
| `list` / `get` / `insert` / `update` | Đọc và ghi cột `quick_access`. |
| `set_quick_access(conn, id, enabled)` | Hàm mới: `UPDATE command_definition SET quick_access = ?2 WHERE id = ?1`; lỗi nếu không có dòng nào bị ảnh hưởng. |
| IPC `commands_create` | Thêm tham số `quick_access: Option<bool>` (mặc định `false`). |
| IPC `commands_update` | Nhận qua `CommandDef`, không đổi chữ ký. |
| IPC mới `commands_set_quick_access(id, enabled, confirmed)` | Theo quy ước mutation có đặc quyền: `require(confirmed)?`. Frontend gửi `confirmed: true` từ thao tác bật/tắt switch tường minh, không cần hộp thoại xác nhận. |
| Sự kiện mới `commands://changed` (`events::COMMANDS_CHANGED`) | Phát sau create / update / delete / set_quick_access, để Workspace (được `keep-alive`) cập nhật Quick Access mà không cần tải lại. |

### 5.3. Frontend

| File | Thay đổi |
| :--- | :--- |
| `src/types/models.ts` | `CommandDefinition.quick_access?: boolean`. |
| `src/ipc/client.ts` | Map `quick_access`; `setCommandQuickAccess(id, enabled)`; `onCommandsChanged(cb)`; bổ sung mock tương ứng. |
| `src/composables/useQuickAccess.ts` (mới) | `items` (lệnh có `quick_access` = true, sắp theo tên), `reload()` khi mount, khi `onActivated` và khi nhận `commands://changed`; `filter`. |
| `src/components/explorer/QuickAccessPanel.vue` (mới) | Section 2 của thanh bên (§3.3–3.5). Phát sự kiện `run(command)` / `paste(command)` và nhận prop `target` (tên, khung, trạng thái; `null` khi không có terminal đích). |
| `src/views/workspace/WorkspaceView.vue` | Chia `sidebar-content` thành GroupTree + sash + QuickAccessPanel, chỉ render sash và panel khi `quickAccessTarget` khác `null`; nối `run` / `paste` tới `dockHostRef.sendToFocusedTerminal()`. |
| `src/components/terminal/DockHost.vue` | Expose `quickAccessTarget` (computed: `{ tabId, label, pane, status, busy, shellKind }` hoặc `null` khi khung active không có tab) và `sendToFocusedTerminal(text, { execute })`: xác định tab đích (§2), gọi `XtermPane.sendCommand`, trả focus về terminal, hiện toast theo `SendResult`. |
| `src/components/terminal/XtermPane.vue` | `sendCommand()` và `inputState()` (§4.1–4.2), dùng lại `currentInput()`, `cursorIsAtInputEnd()` và `shellPhase`. |
| `src/views/commands/CommandLibraryView.vue` | Cột switch, bộ lọc `⚡ Quick Access`, bộ đếm ở footer (§3.1). |
| `src/components/dialogs/CommandEditorModal.vue` | Switch trong form (§3.2). |

## 6. Trường hợp biên & Quyết định

| Tình huống | Quyết định | Lý do |
| :--- | :--- | :--- |
| Tab đích là tab của lệnh đã lưu (ví dụ `vite dev`), không phải shell | Vẫn cho gửi; chỉ báo đích hiện rõ tên tab và `đang bận` | Người dùng có thể cố ý gửi vào REPL hoặc chương trình đang chờ nhập; chỉ báo đích giúp tránh gửi nhầm. |
| Lệnh có `shell_kind` khác shell của tab đích (ví dụ lệnh `pwsh` gửi vào tab `zsh`) | Badge `⚠`, tooltip *"Lệnh được lưu cho pwsh, terminal đích là zsh"*; vẫn cho gửi | Nhiều lệnh (git, docker…) chạy được ở mọi shell. |
| Lệnh nhiều dòng và tab đích là `sh`/`cmd` (không có bracketed paste) | Badge `⚠`, tooltip *"Paste sẽ chạy từng dòng"* | Hạn chế của shell, không chặn được ở phía app. |
| Lệnh kiểu Direct Argv | Gửi nguyên `execution_string` như văn bản | Quick Access dán văn bản vào shell; shell tự tách đối số. Chuỗi có quote vẫn đúng với shell POSIX. |
| Thư mục làm việc | Lệnh chạy ở cwd hiện tại của terminal đích | Quick Access là "gõ hộ", không `cd` ngầm; `command_definition` hiện cũng không lưu cwd. |
| Bấm Run liên tục nhiều lần | Mỗi lần bấm gửi một lần, không gộp | Giống hành vi gõ tay; người dùng thấy kết quả ngay trong terminal. |
| Tắt `quick_access` hoặc xoá lệnh khi đang ở Workspace | Item biến mất nhờ `commands://changed` | Không để lại item trỏ vào lệnh không còn tồn tại. |
| Màn Workspace đang ẩn (keep-alive) | Không phát sinh thao tác nào | Panel chỉ nhận click khi đang hiển thị. |

## 7. Kiểm thử

**Rust (`cargo test --lib`)**
* `db::pool::tests::quick_access_migration_defaults_to_off`: DB phiên bản 9 có sẵn lệnh, migrate lên 10 thì `quick_access = 0` và các cột khác giữ nguyên (theo mẫu `command_definition_migration_preserves_shell_kind`).
* `db::repos::commands` tests:
  * `set_quick_access` bật/tắt rồi `list` phản ánh đúng.
  * `set_quick_access` với id không tồn tại trả lỗi.
  * `update` giữ nguyên `quick_access` khi payload không gửi trường này (`serde(default)`).
* `backup::import`: khôi phục snapshot phiên bản 9 thì migrate thành công.

**Thủ công (chưa có test runner frontend)**

| # | Bước | Kỳ vọng |
| :--- | :--- | :--- |
| 1 | Bật switch ⚡ của 3 lệnh tại SCR-02, quay lại Terminal | 3 item hiện ngay, sắp theo tên, không cần bấm làm mới |
| 2 | Paste vào zsh trống | Lệnh nằm trên dòng lệnh, chưa chạy, con trỏ ở cuối |
| 3 | Run vào zsh trống | Lệnh chạy một lần và xuất hiện trong lịch sử gợi ý |
| 4 | Gõ `abc` rồi Run | `abc` bị xoá, chỉ lệnh Quick Access chạy |
| 5 | Gõ `abc`, đưa con trỏ vào giữa dòng rồi Run | Không gửi, hiện toast hướng dẫn |
| 6 | Split view, focus Khung B rồi Run | Lệnh chạy ở tab active của Khung B |
| 7 | Đóng hết tab; ở Split, focus khung trống | Section Quick Access ẩn, GroupTree chiếm toàn bộ thanh bên; mở lại tab thì section hiện lại với tỉ lệ cũ |
| 8 | Tab đích đã dừng | Nút bị tắt, có tooltip lý do |
| 9 | Run khi đang ở trong `claude` | Lệnh được gửi vào ô nhập của `claude` và gửi đi |
| 10 | Lệnh nhiều dòng, Paste vào zsh | Cả khối nằm trên dòng lệnh, không chạy từng dòng |
| 11 | Kéo sash, thu gọn section, khởi động lại app | Tỉ lệ và trạng thái thu gọn được giữ |
| 12 | Windows: Run vào PowerShell và cmd | Chạy đúng; bước 4 xoá dòng đúng trên PSReadLine |

## 8. Kế hoạch triển khai

1. **Backend:** migration 010, `CommandDef.quick_access`, repo và `set_quick_access`, IPC `commands_set_quick_access`, sự kiện `commands://changed`, các test Rust ở §7.
2. **Client IPC & types:** `models.ts`, `client.ts` (bao gồm mock).
3. **SCR-02 & MOD-02:** cột switch, bộ lọc, bộ đếm, switch trong form.
4. **Terminal API:** `XtermPane.sendCommand` / `inputState`, `DockHost.sendToFocusedTerminal` / `quickAccessTarget`.
5. **SCR-01 thanh bên:** `useQuickAccess`, `QuickAccessPanel`, chia section chỉ khi có terminal đích, sash và lưu trạng thái.
6. **Tài liệu:** cập nhật `screens.md` (SCR-01 §3, SCR-02, MOD-02) từ "thiết kế" sang "đã triển khai", và thêm mục vào `checklist.md`.
7. **Kiểm thử:** chạy bảng thủ công ở §7 trên Linux (zsh, bash) và Windows (pwsh, cmd).

## 9. Mở rộng về sau (không nằm trong đợt này)

* **Sắp xếp thủ công:** thêm cột `quick_access_order INTEGER` và kéo thả trong panel.
* **Phím tắt:** `Ctrl+Alt+1…9` để Run (và `Ctrl+Alt+Shift+1…9` để Paste) item thứ N khi Workspace đang hiển thị.
* **Command Palette (Ctrl+K):** nhóm "Quick Access" với hai hành động Run / Paste.
* **Template trong Quick Access:** mở MOD-10 để điền tham số, rồi Paste/Run chuỗi đã render vào terminal đang focus thay vì spawn PTY mới.
