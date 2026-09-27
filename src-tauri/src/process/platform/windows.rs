use crate::error::{Error, Result};
use std::os::windows::process::CommandExt;
use std::process::Command;
use std::ptr::null_mut;
use std::sync::OnceLock;

/// The app is a GUI process without a console, so every console tool it
/// starts would otherwise open (and flash) a console window of its own.
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

const CREATE_PROCESS_ACCESS: u32 = 0x0001 | 0x0100 | 0x1000;
const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION: u32 = 9;
const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x2000;

#[repr(C)]
struct IoCounters {
    read_operations: u64,
    write_operations: u64,
    other_operations: u64,
    read_bytes: u64,
    write_bytes: u64,
    other_bytes: u64,
}

#[repr(C)]
struct BasicLimitInformation {
    per_process_user_time_limit: i64,
    per_job_user_time_limit: i64,
    limit_flags: u32,
    minimum_working_set_size: usize,
    maximum_working_set_size: usize,
    active_process_limit: u32,
    affinity: usize,
    priority_class: u32,
    scheduling_class: u32,
}

#[repr(C)]
struct ExtendedLimitInformation {
    basic_limit_information: BasicLimitInformation,
    io_info: IoCounters,
    process_memory_limit: usize,
    job_memory_limit: usize,
    peak_process_memory_used: usize,
    peak_job_memory_used: usize,
}

#[link(name = "kernel32")]
extern "system" {
    fn CreateJobObjectW(attributes: *const u8, name: *const u16) -> isize;
    fn SetInformationJobObject(job: isize, class: u32, info: *const u8, length: u32) -> i32;
    fn OpenProcess(access: u32, inherit_handle: i32, process_id: u32) -> isize;
    fn AssignProcessToJobObject(job: isize, process: isize) -> i32;
    fn CloseHandle(handle: isize) -> i32;
    fn GetLastError() -> u32;
}

static APP_JOB: OnceLock<isize> = OnceLock::new();

fn app_job() -> Result<isize> {
    if let Some(&job) = APP_JOB.get() {
        return Ok(job);
    }

    let job = unsafe { CreateJobObjectW(null_mut(), null_mut()) };
    if job == 0 {
        return Err(Error::msg(format!("CreateJobObjectW failed: {}", unsafe {
            GetLastError()
        })));
    }

    let mut limits = ExtendedLimitInformation {
        basic_limit_information: BasicLimitInformation {
            per_process_user_time_limit: 0,
            per_job_user_time_limit: 0,
            limit_flags: JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            minimum_working_set_size: 0,
            maximum_working_set_size: 0,
            active_process_limit: 0,
            affinity: 0,
            priority_class: 0,
            scheduling_class: 0,
        },
        io_info: IoCounters {
            read_operations: 0,
            write_operations: 0,
            other_operations: 0,
            read_bytes: 0,
            write_bytes: 0,
            other_bytes: 0,
        },
        process_memory_limit: 0,
        job_memory_limit: 0,
        peak_process_memory_used: 0,
        peak_job_memory_used: 0,
    };
    let configured = unsafe {
        SetInformationJobObject(
            job,
            JOB_OBJECT_EXTENDED_LIMIT_INFORMATION,
            (&mut limits as *mut ExtendedLimitInformation).cast(),
            std::mem::size_of::<ExtendedLimitInformation>() as u32,
        )
    };
    if configured == 0 {
        return Err(Error::msg(format!(
            "SetInformationJobObject failed: {}",
            unsafe { GetLastError() }
        )));
    }

    // A concurrent first call may lose the race. The losing handle is left
    // for the OS to reclaim at process exit; only the published handle is used.
    let _ = APP_JOB.set(job);
    Ok(*APP_JOB.get().unwrap_or(&job))
}

/// Best-effort registration: nested Job Objects can reject assignment. The
/// process remains usable in that case and normal child-killer cleanup still
/// applies.
pub fn register_process(pid: u32) -> Result<()> {
    if pid == 0 {
        return Ok(());
    }
    let job = app_job()?;
    let process = unsafe { OpenProcess(CREATE_PROCESS_ACCESS, 0, pid) };
    if process == 0 {
        return Err(Error::msg(format!("OpenProcess failed: {}", unsafe {
            GetLastError()
        })));
    }
    let assigned = unsafe { AssignProcessToJobObject(job, process) };
    let _ = unsafe { CloseHandle(process) };
    if assigned == 0 {
        return Err(Error::msg(format!(
            "AssignProcessToJobObject failed: {}",
            unsafe { GetLastError() }
        )));
    }
    Ok(())
}

fn taskkill() -> Command {
    let mut command = Command::new("taskkill");
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

/// Attempts tree termination with taskkill /T /PID <pid>.
/// If the process cannot be terminated gracefully (e.g., console apps requiring /F),
/// falls back to taskkill /F /T /PID <pid> to guarantee the entire process tree is terminated.
pub fn terminate_graceful(pid: u32) -> Result<()> {
    let status = taskkill().args(["/T", "/PID", &pid.to_string()]).status();

    match status {
        Ok(s) if s.success() => Ok(()),
        _ => kill_force(pid),
    }
}

pub fn kill_force(pid: u32) -> Result<()> {
    let status = taskkill()
        .args(["/F", "/T", "/PID", &pid.to_string()])
        .status()?;
    if !status.success() {
        return Err(Error::msg("taskkill force failed"));
    }
    Ok(())
}
