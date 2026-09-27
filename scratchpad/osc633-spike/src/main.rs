use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

const BEL: u8 = 0x07;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut script = None;
    let mut nonce = String::from("osc633-spike");
    let args: Vec<String> = env::args().collect();
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--script" => {
                index += 1;
                script = args.get(index).map(PathBuf::from);
            }
            "--nonce" => {
                index += 1;
                nonce = args.get(index).cloned().ok_or("--nonce requires a value")?;
            }
            other => return Err(format!("unknown argument: {other}").into()),
        }
        index += 1;
    }
    let script = script.ok_or("--script <path> is required")?;
    let script = script.canonicalize()?;

    let pty = native_pty_system().openpty(PtySize {
        rows: 30,
        cols: 120,
        pixel_width: 0,
        pixel_height: 0,
    })?;
    let shell = env::var("CM_SPIKE_PWSH").unwrap_or_else(|_| "pwsh.exe".into());
    let mut command = CommandBuilder::new(shell);
    command.args(["-NoLogo", "-NoProfile", "-NoExit", "-Command"]);
    let script_path = script.to_string_lossy().replace(r"\\?\", "");
    command.arg(format!(". '{}'", script_path.replace('\'', "''")));
    command.env("CM_NONCE", &nonce);
    let mut child = pty.slave.spawn_command(command)?;
    let mut writer = pty.master.take_writer()?;
    let reader = pty.master.try_clone_reader()?;
    let (tx, rx) = mpsc::channel::<Vec<u8>>();

    thread::spawn(move || {
        let mut reader = reader;
        let mut buffer = [0u8; 4096];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(size) => {
                    if tx.send(buffer[..size].to_vec()).is_err() {
                        break;
                    }
                }
            }
        }
    });

    let mut raw = Vec::new();
    let mut chunk_sizes = Vec::new();
    collect_for(&rx, &mut raw, &mut chunk_sizes, Duration::from_secs(2));

    send_line(&mut writer, r#"Write-Output "CM_SPIKE_ONE""#)?;
    collect_for(&rx, &mut raw, &mut chunk_sizes, Duration::from_secs(2));

    send_line(&mut writer, r#"Write-Output "CM_SPIKE;Xin chào""#)?;
    collect_for(&rx, &mut raw, &mut chunk_sizes, Duration::from_secs(2));

    // Starts a multiline PowerShell construct. The second line is sent only
    // after the continuation prompt appears, so PSReadLine should report one
    // multiline command in OSC 633;E.
    writer.write_all(b"& {\r")?;
    writer.flush()?;
    collect_for(&rx, &mut raw, &mut chunk_sizes, Duration::from_millis(500));
    writer.write_all(b"  Write-Output \"CM_MULTI_1\"\r")?;
    writer.flush()?;
    collect_for(&rx, &mut raw, &mut chunk_sizes, Duration::from_millis(500));
    writer.write_all(b"  Write-Output \"CM_MULTI_2\"\r")?;
    writer.flush()?;
    collect_for(&rx, &mut raw, &mut chunk_sizes, Duration::from_millis(500));
    writer.write_all(b"}\r")?;
    writer.flush()?;
    collect_for(&rx, &mut raw, &mut chunk_sizes, Duration::from_secs(2));

    let frames = parse_osc633(&raw);
    let escaped = String::from_utf8_lossy(&raw);
    let output_dir = script.parent().ok_or("script has no parent")?;
    fs::write(output_dir.join("osc633.raw"), &raw)?;
    fs::write(
        output_dir.join("osc633.frames.txt"),
        frames
            .iter()
            .enumerate()
            .map(|(i, frame)| format!("{i:03}: {frame}\n"))
            .collect::<String>(),
    )?;

    println!("Captured {} OSC 633 frames", frames.len());
    for (index, frame) in frames.iter().enumerate() {
        println!("{index:03}: {frame}");
    }
    println!("Captured {} raw bytes", raw.len());
    println!(
        "Read {} PTY chunks (max {} bytes)",
        chunk_sizes.len(),
        chunk_sizes.iter().copied().max().unwrap_or(0)
    );
    println!(
        "OSC 633 frames crossing a PTY chunk boundary: {}",
        count_split_frames(&raw, &chunk_sizes)
    );
    println!(
        "Contains CM_SPIKE_ONE: {}",
        escaped.contains("CM_SPIKE_ONE")
    );
    println!("Contains CM_MULTI_1: {}", escaped.contains("CM_MULTI_1"));
    println!("Contains CM_MULTI_2: {}", escaped.contains("CM_MULTI_2"));

    let _ = child.kill();
    let _ = child.wait();
    Ok(())
}

fn send_line(writer: &mut Box<dyn Write + Send>, line: &str) -> std::io::Result<()> {
    writer.write_all(line.as_bytes())?;
    writer.write_all(b"\r")?;
    writer.flush()
}

fn collect_for(
    rx: &mpsc::Receiver<Vec<u8>>,
    output: &mut Vec<u8>,
    chunk_sizes: &mut Vec<usize>,
    duration: Duration,
) {
    let deadline = Instant::now() + duration;
    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(remaining.min(Duration::from_millis(100))) {
            Ok(chunk) => {
                chunk_sizes.push(chunk.len());
                output.extend_from_slice(&chunk);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn parse_osc633(bytes: &[u8]) -> Vec<String> {
    let prefix = b"\x1b]633;";
    let mut frames = Vec::new();
    let mut cursor = 0;
    while let Some(relative) = bytes[cursor..]
        .windows(prefix.len())
        .position(|window| window == prefix)
    {
        let start = cursor + relative + prefix.len();
        let Some(end_relative) = bytes[start..].iter().position(|byte| *byte == BEL) else {
            break;
        };
        let end = start + end_relative;
        frames.push(String::from_utf8_lossy(&bytes[start..end]).to_string());
        cursor = end + 1;
    }
    frames
}

fn count_split_frames(bytes: &[u8], chunk_sizes: &[usize]) -> usize {
    let prefix = b"\x1b]633;";
    let mut boundaries = Vec::new();
    let mut offset = 0;
    for size in chunk_sizes.iter().take(chunk_sizes.len().saturating_sub(1)) {
        offset += size;
        boundaries.push(offset);
    }

    let mut split_count = 0;
    let mut cursor = 0;
    while let Some(relative) = bytes[cursor..]
        .windows(prefix.len())
        .position(|window| window == prefix)
    {
        let start = cursor + relative;
        let payload_start = start + prefix.len();
        let Some(end_relative) = bytes[payload_start..].iter().position(|byte| *byte == BEL) else {
            break;
        };
        let end = payload_start + end_relative + 1;
        if boundaries
            .iter()
            .any(|boundary| *boundary > start && *boundary < end)
        {
            split_count += 1;
        }
        cursor = end;
    }
    split_count
}
