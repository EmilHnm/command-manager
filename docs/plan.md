# **Báo cáo Nghiên cứu và Thiết kế Kiến trúc: Hệ thống Ứng dụng Desktop Quản lý, Thực thi Lệnh và Terminal Đa Nhiệm**

## **1\. Bối cảnh, Tầm nhìn và Phạm vi MVP**

Trong kỷ nguyên của điện toán đám mây và cơ sở hạ tầng dưới dạng mã (Infrastructure as Code), người dùng thành thạo thường xuyên phải duy trì hàng chục cửa sổ terminal, ghi nhớ hàng trăm tham số phức tạp, và tự động hóa thủ công các tiến trình khởi động. Báo cáo này trình bày bản thiết kế kiến trúc cho một ứng dụng desktop quản lý các tập hợp lệnh, kết hợp giữa sức mạnh của CLI và tính tiện dụng của GUI.  
Để đảm bảo tính khả thi trong triển khai, **Phạm vi Sản phẩm Khả thi Tối thiểu (MVP)** được chốt lại như sau:

* **Tính năng lõi:** Lưu trữ lệnh/nhóm lệnh; chạy bằng argv hoặc shell tường minh; hiển thị terminal PTY theo thẻ (tabs).  
* **Quản lý phiên chạy:** Theo dõi trạng thái (start/stop) và log trong phiên chạy.  
* **Tự khởi động:** Hỗ trợ tự khởi động ứng dụng và các nhóm lệnh được cấu hình autostart, kết hợp khóa cấp ứng dụng để chống chạy trùng.  
* **Sao lưu:** Sao lưu và khôi phục cục bộ (thủ công) với quy trình khép kín, kiểm tra tính toàn vẹn và có khả năng phục hồi (rollback).  
* *Giai đoạn sau (Post-MVP):* Đẩy systemd services, biểu mẫu động cho CLI (RJSF), đồng bộ Google Drive sang các phiên bản tiếp theo.

Nguyên tắc thiết kế quan trọng nhất chi phối kiến trúc: **Ứng dụng này thực thi các lệnh được người dùng tin cậy, không phải là một môi trường hộp cát (sandbox).** Hợp đồng vòng đời tiến trình (đóng app thì toàn bộ lệnh dừng lại) được xác định thống nhất để làm kim chỉ nam cho Trình Quản lý Tiến trình (Process Manager).

## **2\. Kiến trúc Cốt lõi và Lựa chọn Nền tảng Công nghệ**

Hệ thống được xây dựng trên nền tảng Tauri v2, tận dụng hệ sinh thái Rust ở backend và React kết hợp TypeScript ở frontend1. Tauri giao phó việc hiển thị giao diện cho tiến trình Webview gốc của hệ điều hành, giúp giảm thiểu tiêu thụ RAM1. Lớp backend bằng Rust đảm nhận vai trò quản lý luồng (process manager), cơ sở dữ liệu SQLite và PTY.

| Thành phần Hệ thống | Công nghệ Lựa chọn | Trách nhiệm và Chức năng Cốt lõi |
| :---- | :---- | :---- |
| **Giao diện Người dùng (Frontend)** | Vue, TypeScript, Vite | Quản lý DOM của terminal ảo, phân chia bố cục giao diện (tabs). |
| **Khung Tích hợp (Bridge)** | Tauri v2 (IPC) | Định tuyến thông điệp, quản lý Capabilities và phân quyền. |
| **Trình Quản lý (Backend)** | Rust (Tokio Runtime) | Quản lý vòng đời tiến trình, buffer PTY, điều phối trạng thái memory. |
| **Lưu trữ Cục bộ (Storage)** | SQLite (chế độ WAL) | Lưu trữ danh mục lệnh, nhóm, và lịch sử chạy tĩnh. |

## **3\. Thiết kế Lược đồ Dữ liệu và Trạng thái Tiến trình**

Lưu trữ, định danh và phân loại câu lệnh là nền tảng của hệ thống. Việc ghi đè mọi log hay trạng thái liên tục vào SQLite là quá mức cho một ứng dụng desktop, do đó kiến trúc tách bạch rõ ràng giữa **lưu trữ tĩnh (DB)** và **trạng thái động (Memory)**.

### **3.1. Cấu trúc Mô hình Quan hệ (Chuẩn hóa)**

Lược đồ cơ sở dữ liệu bao gồm 5 bảng chính để biểu diễn rõ ràng sự tách biệt giữa định nghĩa lệnh và các phiên chạy:

| Bảng Cơ sở Dữ liệu | Trực quan hóa Cấu trúc và Trường thông tin | Ràng buộc và Vai trò |
| :---- | :---- | :---- |
| command\_definition | id (PK), name, execution\_string, is\_shell. | Định nghĩa lõi của lệnh. Lựa chọn chạy thẳng qua argv hay bọc qua shell phải rõ ràng. |
| command\_group | id (PK), group\_name, autostart (BOOLEAN). | Nhóm các lệnh. Cờ autostart áp dụng ở cấp độ nhóm để kích hoạt toàn nhóm khi app chạy. |
| group\_membership | group\_id (FK), command\_id (FK), execution\_order. | Bảng trung gian định nghĩa các lệnh thuộc nhóm nào và thứ tự khởi chạy. |
| run\_session | id (PK), group\_id (FK), started\_at, status. | Đại diện cho một lần bấm "Play" tổng thể của một nhóm. |
| run\_event | id (PK), session\_id (FK), command\_id, started\_at, ended\_at, status, exit\_code, pid. | Bản ghi sự kiện chi tiết cho mỗi lần thực thi lệnh. pid chỉ lưu để chẩn đoán tạm thời; không dùng để định danh tiến trình sau khi khởi động lại app. |

### **3.2. Quản lý Trạng thái Bộ nhớ và Tối ưu SQLite**

Thay vì tạo ra một pool reader bằng số lượng lõi CPU gây lãng phí, ứng dụng khởi đầu với **một writer duy nhất và một số lượng nhỏ kết nối đọc** (ví dụ: 2-3 connections)3.

* **Trạng thái đang chạy (Running State):** Toàn bộ trạng thái tiến trình (PID, buffer log hiện hành) được giữ trong bộ nhớ (Memory) của Rust thông qua các cấu trúc như DashMap hoặc RwLock.  
* **Giới hạn Log (Log Buffer):** Log phát sinh từ stdout/stderr không được ghi ngay vào DB. Chúng được đưa vào một bộ đệm vòng (ring buffer) có giới hạn theo số **byte** (ví dụ: 1MB hoặc 2MB) trong bộ nhớ để đẩy lên UI. Kích thước theo byte giúp khống chế mức tiêu thụ bộ nhớ hiệu quả hơn giới hạn dòng, do một "dòng" có thể dài vô tận.  
* **Cập nhật DB:** SQLite chỉ được ghi vào khi có sự thay đổi vòng đời quan trọng (bắt đầu chạy, kết thúc tiến trình kèm exit code)3.

## **4\. Ảo hóa Terminal và Quản lý Tương tác Dòng lệnh**

Việc sử dụng Pseudo-Terminal (PTY) giải quyết việc các ứng dụng CLI kiểm tra TTY và giữ được màu sắc ANSI5. Tuy nhiên, PTY không phải là viên đạn bạc và cần được thiết kế cẩn trọng.

### **4.1. Kiến trúc PTY và Xử lý Luồng Dữ liệu**

Khi backend phân bổ PTY thông qua plugin (như tauri-plugin-pty), luồng byte thô (bao gồm mã ANSI) được chuyển tới xterm.js trên frontend6.

* **Kiểm soát Áp lực ngược (Backpressure):** Nếu tiến trình con in log quá nhanh, UI sẽ bị treo. Việc giới hạn buffer theo byte ở bộ nhớ (nêu tại phần 3.2) chỉ giúp khống chế dung lượng RAM, nhưng không tự giải quyết được việc UI nhận dữ liệu chậm. Backend Rust cần quy định rõ chính sách xử lý áp lực ngược: tạm ngừng đọc từ PTY, chủ động vứt bỏ (drop) các phần đầu ra cũ, hoặc gộp sự kiện (chunking/debouncing) trước khi gửi qua IPC.  
* **Resize Terminal:** Kích thước xterm.js trên UI (cols/rows) phải được đồng bộ liên tục với PTY ở kernel thông qua các lệnh resize cụ thể mỗi khi người dùng thay đổi kích thước thẻ (tab) hoặc cửa sổ ứng dụng.  
* **Gắn lại (Reattaching):** Khi người dùng chuyển qua lại giữa các thẻ, hoặc đóng một thẻ UI nhưng tiến trình vẫn chạy ngầm, backend tiếp tục lưu luồng byte vào buffer memory. Khi thẻ được mở lại, backend xả buffer này xuống xterm.js để **xem lại đầu ra gần đây (recent output)**. Cần lưu ý, việc xả buffer thô không thể khôi phục chính xác trạng thái màn hình của các chương trình terminal toàn màn hình (như vim, htop).  
* **Lọc Bí mật (Secrets):** Cần lưu ý rằng luồng dữ liệu PTY chứa mọi ký tự, bao gồm cả mật khẩu người dùng gõ. Việc không ghi log PTY bừa bãi vào đĩa cứng là bắt buộc.

### **4.2. Quản lý Bố cục (Layout Manager)**

UI sử dụng dockview (thông qua dockview-react) để hỗ trợ thẻ (tabs)7. Khi một run\_session kích hoạt, frontend gọi api.addPanel() để sinh ra các tab tương ứng7. Việc tổ chức dạng lưới (grid) phức tạp sẽ được đánh giá ở giai đoạn sau nếu nhu cầu đa nhiệm tăng cao10. Chi tiết thiết kế toàn diện các màn hình và trạng thái xem tại [screens.md](./screens.md).

## **5\. Quản trị Vòng đời Tiến trình: Hợp đồng Tắt/Bật**

Thiết kế chính sách quản lý vòng đời tiến trình là quyết định cốt lõi nhất của Process Manager. Trong phiên bản MVP, mọi tiến trình do ứng dụng sinh ra đều là **Tiến trình gắn liền (App-bound)**.

### **5.1. Khởi động Cùng Hệ điều hành và Khóa Single-Instance**

Tính năng autostart được quản lý bằng tauri-plugin-autostart12. Khi Tauri khởi chạy (bất kể do OS boot hay do người dùng tự mở), ứng dụng sẽ truy vấn bảng command\_group để tìm các nhóm có cờ autostart. **Khóa Đơn bản thể (Single-Instance Lock):** Để chống chạy trùng lặp một cách triệt để, hệ thống phải thiết lập khóa single-instance ở cấp độ ứng dụng ngay khi khởi động, *trước khi* bộ quản lý tiến trình được khởi tạo. Nếu phát hiện một instance khác đang chạy, instance mới sẽ chuyển quyền điều khiển (ví dụ: focus cửa sổ) cho instance cũ và tự thoát. Sau khi đã nắm giữ khóa single-instance, hệ thống mới kiểm tra các run\_session đang active để đảm bảo không kích hoạt lại các nhóm autostart đã và đang chạy trong instance duy nhất này.

### **5.2. Chính sách Chấm dứt Tiến trình (Graceful Shutdown)**

Hợp đồng nhất quán khi người dùng thực hiện các thao tác:

* **Đóng tab UI:** Tab bị ẩn đi và terminal đóng lại, nhưng **tiến trình ở backend Rust vẫn tiếp tục chạy** bình thường. Trạng thái chỉ dừng khi người dùng bấm nút "Stop" hoặc khi thoát ứng dụng hoàn toàn.  
* **Đóng cửa sổ/Thoát App:** Dừng **toàn bộ** mọi tiến trình đang chạy theo chính sách: dừng mềm → chờ có hạn → dừng cưỡng bức → thu hồi tiến trình.  
* **Crash đột ngột:** Phụ thuộc vào cơ chế thu hồi của Kernel.

Cụ thể, quy trình chấm dứt duyên dáng (Graceful Shutdown) toàn cục khi thoát app được triển khai qua các bước:

> 1. **Dừng mềm:** Rust backend duyệt qua toàn bộ danh sách tiến trình con đang active. Trên Linux/macOS, gửi tín hiệu SIGTERM14. Trên Windows, gửi CTRL\_C\_EVENT (việc gửi event này cho một process group cụ thể trên Windows có nhiều giới hạn kỹ thuật nên cần adapter xử lý riêng)16.  
> 2. **Chờ đợi (Wait Timeout):** Đặt một khoảng thời gian chờ (ví dụ: 5-10 giây) để các tiến trình tự dọn dẹp.  
> 3. **Dừng cưỡng bức:** Nếu timeout, gửi SIGKILL (hoặc TerminateProcess trên Windows) để diệt toàn bộ thây ma14.  
> 4. **Dự phòng (Crash):** Hàm PR\_SET\_PDEATHSIG có thể được sử dụng trên Linux trước khi gọi exec. Dù tín hiệu này không bao quát toàn bộ cây tiến trình con (descendants), nó là phương án giảm thiểu rủi ro hữu hiệu, giúp các tiến trình trực tiếp do Tauri sinh ra có khả năng bị tiêu diệt khi app crash17.

## **6\. Giao diện Tham số Động (RJSF) cho Post-MVP**

Trong các bản cập nhật sau MVP, hệ thống sẽ hỗ trợ Lập trình Giao diện Định hướng Lược đồ (Schema-Driven UI) qua react-jsonschema-form (RJSF)18. Tuy nhiên, RJSF chỉ xác thực dữ liệu biểu mẫu trên UI, nó không bảo đảm cú pháp CLI hay tính an toàn của lệnh. **Giải pháp:** Xây dựng một lớp Bộ chuyển đổi (Adapter) tại Rust. Adapter này nhận JSON từ RJSF, kiểm tra logic nghiệp vụ theo từng công cụ cụ thể (như ffmpeg hoặc cloudflared), sau đó mới sinh ra mảng argv hoặc file cấu hình YAML20. UI sẽ tích hợp thêm tính năng **"Preview Command"** để người dùng xác nhận chuỗi lệnh trước khi thực sự chạy.

## **7\. Sao lưu, Khôi phục và Bảo mật Cấu hình**

Khả năng sao lưu an toàn là thiết yếu, nhưng trong MVP, ứng dụng chỉ tập trung vào sao lưu và khôi phục cục bộ theo phương pháp thủ công, đảm bảo quy trình khép kín và toàn vẹn dữ liệu.

### **7.1. Quy trình Khép kín và Cơ chế Rollback**

Do SQLite vận hành ở chế độ WAL, API VACUUM INTO được dùng để tạo ra bản snapshot sao lưu22. Quy trình được thiết kế chặt chẽ và có khả năng phục hồi:

* **Khi Sao lưu (Export):** Lệnh VACUUM INTO được gọi. Hệ thống chỉ thông báo "Sao lưu thành công" tới người dùng *sau khi* file snapshot đã được tạo ra hoàn chỉnh và vượt qua bài kiểm tra PRAGMA integrity\_check.  
* **Khi Khôi phục (Import):** Việc thay thế cơ sở dữ liệu khi dùng chế độ WAL cần kiểm soát nghiêm ngặt các tệp liên quan và kết nối. Trình tự thực hiện như sau:  
  1. **Tải file:** Tải file sao lưu được cung cấp vào một vùng tạm.  
  2. **Kiểm tra hợp lệ:** Chạy PRAGMA integrity\_check trên file tạm để xác nhận cấu trúc SQLite, đồng thời kiểm tra phiên bản lược đồ (schema version) để đảm bảo ứng dụng có thể đọc được dữ liệu.  
  3. **Xác nhận từ người dùng:** Hiển thị cảnh báo và yêu cầu người dùng xác nhận *trước khi* tiến hành bất kỳ thao tác can thiệp nào.  
  4. **Sao lưu dự phòng:** Tạo ra một bản sao dự phòng (backup) của tệp cơ sở dữ liệu app.db hiện tại.  
  5. **Ngắt kết nối:** Dừng toàn bộ các run\_session đang chạy và đóng mọi kết nối đang mở tới SQLite.  
  6. **Thay thế tệp:** Xóa bỏ tệp app.db hiện tại cùng các tệp bộ nhớ đệm app.db-wal và app.db-shm để tránh tình trạng trộn lẫn dữ liệu22. Sau đó copy tệp sao lưu mới vào vị trí.  
  7. **Kiểm tra và Phục hồi (Rollback):** Mở lại kết nối và xác nhận dữ liệu. Nếu việc mở DB mới hoặc tải lại UI thất bại, ứng dụng sẽ ngay lập tức thực hiện rollback bằng cách khôi phục lại bản sao dự phòng ở bước 4, đảm bảo luôn có đường lui an toàn.

### **7.2. Bảo mật Token (Post-MVP)**

Khi tính năng tích hợp Google Drive được triển khai ở phiên bản sau, các token xác thực sẽ được bảo vệ bằng hầm chứa bảo mật nguyên bản (OS Keyring/Credential Manager) thông qua thư viện keyring24. *Lưu ý bảo mật:* Cơ chế này bảo vệ token khỏi việc bị đánh cắp dưới dạng văn bản thuần túy (plain-text) trên đĩa cứng24. Tuy nhiên, nếu chính tiến trình ứng dụng bị xâm nhập từ bên trong, kẻ tấn công vẫn có thể yêu cầu OS giải mã token. Do đó, đây là lớp phòng thủ bề mặt (mitigation), không loại bỏ hoàn toàn rủi ro.

## **8\. Mô hình Phân quyền Tauri và Ranh giới Bảo mật**

Vì đây là ứng dụng quản lý lệnh, các dòng lệnh lưu trong SQLite là các lệnh shell do người dùng tùy ý cấu hình.  
Trong Tauri v2, mặc dù hệ thống Quyền năng (Capabilities) giới hạn diện mạo tấn công28, mã chạy trong WebView (nếu bị tiêm XSS) vẫn có thể gọi các lệnh Rust đã đăng ký qua invoke\_handler để khởi chạy một Command ID bất kỳ có sẵn trong SQLite28. Để thiết lập ranh giới an toàn:

> 1. **Kiểm soát Quyền Sửa đổi:** Việc tạo mới, sửa cấu hình lệnh, sửa cờ autostart, và nhập (import) bản sao lưu phải được đối xử như các tác vụ đặc quyền. Giao diện (UI) hoặc Backend có thể yêu cầu xác nhận trước khi lưu các thay đổi này, đặc biệt nếu bản sao lưu đến từ một nguồn bên ngoài.  
> 2. **Không gian Tin cậy (Trusted Zone):** Phải làm rõ với người dùng rằng bất cứ thứ gì đưa vào DB lệnh đều được ứng dụng coi là an toàn để chạy.  
> 3. **Chính sách CSP:** Cấu hình csp (default-src 'self') nghiêm ngặt được giữ nguyên để ngăn chặn việc tải mã độc từ bên ngoài, giảm thiểu tối đa rủi ro XSS2.

## **9\. Kết luận**

Tài liệu đặc tả này đã xác định một giới hạn MVP rõ ràng và khả thi. Sự kết hợp giữa Tauri v2, SQLite và hệ thống PTY mang lại nền tảng vững chắc cho việc quản lý các dòng lệnh.  
Bằng cách tách biệt trạng thái tiến trình động khỏi trạng thái định nghĩa tĩnh (lưu ở SQLite), kiến trúc đảm bảo hiệu năng không bị bóp nghẹt. Việc xây dựng một hợp đồng rõ ràng, nhất quán về vòng đời tiến trình (mọi tiến trình đều dừng khi thoát app), kèm theo cơ chế khóa cấp ứng dụng (single-instance) và quy trình sao lưu/khôi phục an toàn có bước xác nhận và rollback, đã giúp sản phẩm có định hướng triển khai cực kỳ sát với thực tế, chuẩn bị nền tảng hoàn hảo để bắt đầu phát triển Process Manager.

#### **Works cited**

> 1. Plugin Development \- Tauri, [https://v2.tauri.app/develop/plugins/](https://v2.tauri.app/develop/plugins/)  
> 2. Tauri V2 Overview, [https://huakun.tech/Full-Stack/Framework/Tauri/tauri-v2](https://huakun.tech/Full-Stack/Framework/Tauri/tauri-v2)  
> 3. PSA: Your SQLite Connection Pool Might Be Ruining Your Write, [https://emschwartz.me/psa-your-sqlite-connection-pool-might-be-ruining-your-write-performance/](https://emschwartz.me/psa-your-sqlite-connection-pool-might-be-ruining-your-write-performance/)  
> 4. CPU Bottleneck and SQLite Pooling: The Impact of Core Count, [https://www.cruzluna.dev/posts/sqliteconnectionpool/](https://www.cruzluna.dev/posts/sqliteconnectionpool/)  
> 5. tauri-plugin-pty \- crates.io: Rust Package Registry, [https://crates.io/crates/tauri-plugin-pty/versions](https://crates.io/crates/tauri-plugin-pty/versions)  
> 6. GitHub \- Tnze/tauri-plugin-pty: A Tauri2 plugin for embedding a, [https://github.com/Tnze/tauri-plugin-pty](https://github.com/Tnze/tauri-plugin-pty)  
> 7. npm:dockview \- Skypack.dev, [https://www.skypack.dev/view/dockview](https://www.skypack.dev/view/dockview)  
> 8. dockview-react \- NPM, [https://npmjs.com/package/dockview-react?ref=pkgstats.com](https://npmjs.com/package/dockview-react?ref=pkgstats.com)  
> 9. GitHub \- dockview/dockview: Zero dependency docking layout, [https://github.com/dockview/dockview](https://github.com/dockview/dockview)  
> 10. Core concepts \- Dockview, [https://dockview.dev/docs/core/overview/](https://dockview.dev/docs/core/overview/)  
> 11. Dockview: Docking Layout Manager, [https://dockview.dev/](https://dockview.dev/)  
> 12. Autostart \- Tauri, [https://v2.tauri.app/plugin/autostart/](https://v2.tauri.app/plugin/autostart/)  
> 13. tauri-apps/tauri-plugin-autostart: \[READ ONLY\] This ... \- GitHub, [https://github.com/tauri-apps/tauri-plugin-autostart](https://github.com/tauri-apps/tauri-plugin-autostart)  
> 14. Understanding signals in Linux, e.g., kill \-9, CTRL \+ C on programs, [https://medium.com/@techwithtwin/understanding-signals-in-linux-e-g-kill-9-ctrl-c-on-programs-6da5aee8ce68](https://medium.com/@techwithtwin/understanding-signals-in-linux-e-g-kill-9-ctrl-c-on-programs-6da5aee8ce68)  
> 15. Handling Unix Kill Signals in Rust \- DEV Community, [https://dev.to/talzvon/handling-unix-kill-signals-in-rust-55g6](https://dev.to/talzvon/handling-unix-kill-signals-in-rust-55g6)  
> 16. Creating a server and gracefully killing it. : r/C\_Programming \- Reddit, [https://www.reddit.com/r/C\_Programming/comments/1dz6dkr/creating\_a\_server\_and\_gracefully\_killing\_it/](https://www.reddit.com/r/C_Programming/comments/1dz6dkr/creating_a_server_and_gracefully_killing_it/)  
> 17. Fixing Ctrl+C in Rust terminal apps: Child process management, [https://news.ycombinator.com/item?id=44728796](https://news.ycombinator.com/item?id=44728796)  
> 18. Building Dynamic Forms in React with JSON Schema Form (RJSF), [https://medium.com/techverito/building-dynamic-forms-in-react-with-json-schema-form-rjsf-a-step-by-step-guide-f6d58ae6efe0](https://medium.com/techverito/building-dynamic-forms-in-react-with-json-schema-form-rjsf-a-step-by-step-guide-f6d58ae6efe0)  
> 19. GitHub \- rjsf-team/react-jsonschema-form, [https://github.com/rjsf-team/react-jsonschema-form](https://github.com/rjsf-team/react-jsonschema-form)  
> 20. Cloudflared Ingress Configuration | OpenCharts \- Community Charts, [https://community-charts.github.io/docs/charts/cloudflared/ingress-configuration](https://community-charts.github.io/docs/charts/cloudflared/ingress-configuration)  
> 21. Cloudflare tunnel ingress rules \- GitHub Gist, [https://gist.github.com/sirkirby/4d5a4b2f72490b8063e3c3554d2130af](https://gist.github.com/sirkirby/4d5a4b2f72490b8063e3c3554d2130af)  
> 22. Runnable SQLite Docs: Backup & Restore \- Coddy Tech, [https://coddy.tech/docs/sqlite/backup-and-restore](https://coddy.tech/docs/sqlite/backup-and-restore)  
> 23. I want to take a backup of a database at a snapshot point while other, [https://www.reddit.com/r/sqlite/comments/rx9phv/i\_want\_to\_take\_a\_backup\_of\_a\_database\_at\_a/](https://www.reddit.com/r/sqlite/comments/rx9phv/i_want_to_take_a_backup_of_a_database_at_a/)  
> 24. tauri-plugin-secure-keystore \- crates.io: Rust Package Registry, [https://crates.io/crates/tauri-plugin-secure-keystore](https://crates.io/crates/tauri-plugin-secure-keystore)  
> 25. Building a Jira Time Tracker with Tauri: How I Stored API Tokens, [https://dev.to/jorrygo\_dev/building-a-jira-time-tracker-with-tauri-how-i-stored-api-tokens-securely-46aj](https://dev.to/jorrygo_dev/building-a-jira-time-tracker-with-tauri-how-i-stored-api-tokens-securely-46aj)  
> 26. Building a Cross-Platform Admin Desktop App with Next.js, Tauri, [https://vincenteliezer.medium.com/building-a-cross-platform-admin-desktop-app-with-next-js-tauri-rust-token-storage-234c6e88bf2d](https://vincenteliezer.medium.com/building-a-cross-platform-admin-desktop-app-with-next-js-tauri-rust-token-storage-234c6e88bf2d)  
> 27. Storing API Keys Safely in a Tauri App — Don't Just Use LocalStorage, [https://dev.to/hiyoyok/storing-api-keys-safely-in-a-tauri-app-dont-just-use-localstorage-2i71](https://dev.to/hiyoyok/storing-api-keys-safely-in-a-tauri-app-dont-just-use-localstorage-2i71)  
> 28. Permissions and Capabilities | zudo-tauri-wisdom, [https://zudo-tauri-wisdom.takazudomodular.com/docs/frontend/capabilities](https://zudo-tauri-wisdom.takazudomodular.com/docs/frontend/capabilities)  
> 29. Capabilities | Tauri, [https://v2.tauri.app/security/capabilities/](https://v2.tauri.app/security/capabilities/)