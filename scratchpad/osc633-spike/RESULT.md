# Kết quả spike OSC 633 / PowerShell

Ngày chạy: 26/09/2026 trên Windows 10 build 19045.

## Môi trường

- PowerShell 7.6.5.
- PSReadLine 2.4.5.
- portable-pty 0.8.1, backend ConPTY.
- Nonce thử nghiệm: spike-20260926-rerun.

## Kết quả thực tế

Harness đã nhận 28 frame OSC 633 qua PTY, trong 27 chunk đọc, không mất frame
và không thấy đảo thứ tự:

    D;0
    P;Cwd=C:\\Users\\HOA
    A
    B
    E;Write-Output\x20"CM_SPIKE_ONE";spike-20260926-rerun
    C
    D;0
    P;Cwd=C:\\Users\\HOA
    A
    B
    E;Write-Output\x20"CM_SPIKE\x3bXin\x20chào";spike-20260926-rerun
    C
    D;0
    P;Cwd=C:\\Users\\HOA
    A
    B
    E;&\x20{;spike-20260926-rerun
    C
    E;&\x20{\x0a\x20\x20Write-Output\x20"CM_MULTI_1";spike-20260926-rerun
    C
    E;&\x20{\x0a\x20\x20Write-Output\x20"CM_MULTI_1"\x0a\x20\x20Write-Output\x20"CM_MULTI_2";spike-20260926-rerun
    C
    E;&\x20{\x0a\x20\x20Write-Output\x20"CM_MULTI_1"\x0a\x20\x20Write-Output\x20"CM_MULTI_2"\x0a};spike-20260926-rerun
    C
    D;0
    P;Cwd=C:\\Users\\HOA
    A
    B

Các điểm đã xác nhận:

- Dấu ; trong command trở thành \x3b.
- Khoảng trắng trở thành \x20.
- Newline của lệnh nhiều dòng trở thành \x0a.
- Dấu \ trong path được escape thành \\.
- Unicode chào đi qua nguyên vẹn.
- Nonce xuất hiện ở trường cuối của mọi E.
- ConPTY giữ nguyên byte OSC trong lần chạy này.
- Lệnh nhiều dòng phát các buffer trung gian trước khi kết thúc; analyzer và
  `ShellTracker` giữ buffer cuối cùng cho tới `D` rồi mới ghi lịch sử.

Kết quả này chưa chứng minh frame luôn nằm gọn trong một lần read; lần chạy
này không có frame nào cắt qua ranh giới chunk. Vì vậy OscScanner vẫn phải
giữ state qua nhiều chunk và không được giả định một lần đọc tương ứng một
frame.

## Quyết định

Tiếp tục hướng OSC 633/PowerShell. Không cần đổi hướng vì ConPTY không làm mất
hoặc sắp xếp lại frame trong phép thử này.

## Đề xuất cấu trúc triển khai

    src-tauri/src/pty/
      osc.rs
      shell_integration/
        mod.rs
        powershell.ps1
        bash.sh
        zsh.zsh
        sh.sh

spawn_interactive_on_slave hiện thực:

1. Chọn pwsh → powershell.exe → cmd.exe trên Windows.
2. Sinh nonce riêng cho mỗi terminal và truyền qua CM_NONCE.
3. Với PowerShell, nạp script bằng `-NoLogo -NoExit -File`; handler Enter của
   PSReadLine đọc buffer hiện tại, phát `E/C`, rồi gọi `AcceptLine()` gốc.
4. Với cmd.exe, mở shell bình thường và đánh dấu shell_kind = cmd; không
   nhận diện rich command nếu không có integration.

OscScanner nên là state machine trong reader:

- Giữ byte raw nguyên vẹn trong Ring Buffer.
- Nhận byte qua nhiều chunk, tìm ESC ]633;, kết thúc bằng BEL hoặc ST.
- Parse A, B, C, D, E, P;Cwd=.
- Unescape \xAB và \\ cho payload E.
- Chỉ chấp nhận E có nonce đúng với phiên.
- Nếu frame lỗi hoặc vượt giới hạn kích thước, bỏ qua metadata nhưng vẫn giữ
  nguyên byte trong Ring Buffer.

Các event tối thiểu cần phát ra:

    PromptStart
    PromptEnd
    CommandLine { command, nonce_valid }
    CommandStart
    CommandFinished { exit_code }
    Cwd { path }

Điều kiện an toàn cho autosuggestion: chỉ ghi history từ CommandLine có
nonce hợp lệ và chuỗi lifecycle hợp lệ; không dùng keystroke hoặc dữ liệu
terminal không xác thực.

## Smoke POSIX-compatible trên Windows (26/09)

Test PTY `git_bash_integration_records_compound_commands_and_skips_ignored_lines`
chạy `bash.sh` qua Git Bash với nonce riêng và xác nhận:

- `echo one; echo two` được gộp thành một dòng history, không tạo bản ghi cho
  từng simple command.
- `P;Cwd=` được phát ở prompt.
- Sau khi bật `HISTCONTROL=ignorespace`, dòng bắt đầu bằng khoảng trắng không
  phát `E/C` và không lọt vào history.

Đây là bằng chứng tương thích POSIX trên Windows, không thay thế runtime Linux;
smoke bash/zsh/dash thật vẫn do job `ubuntu-latest` trong `gate11.yml` xác nhận.
