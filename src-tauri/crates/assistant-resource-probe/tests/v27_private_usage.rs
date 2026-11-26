//! V27 实机断言（I76 §10.2 V27）：`sysinfo::Process::virtual_memory()` 与
//! `GetProcessMemoryInfo(PROCESS_MEMORY_COUNTERS_EX).PrivateUsage` 逐字节相等。
//!
//! 断言对象必须是静止子进程：采样器自身在取数过程中还在分配内存，读之间必然漂移，
//! 拿它做断言只会得到假红（探针已实测，I76 §10.2 V27 备注）。

#![cfg(windows)]

use std::process::Command;
use std::thread::sleep;
use std::time::Duration;

use assistant_resource_probe::default_source;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::ProcessStatus::{
    GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX,
};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_VM_READ,
};

fn private_usage(pid: u32) -> Option<u64> {
    unsafe {
        let handle = OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ,
            false,
            pid,
        )
        .ok()?;
        let mut counters = PROCESS_MEMORY_COUNTERS_EX::default();
        let size = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
        let result = GetProcessMemoryInfo(
            handle,
            &mut counters as *mut PROCESS_MEMORY_COUNTERS_EX as *mut PROCESS_MEMORY_COUNTERS,
            size,
        );
        let _ = CloseHandle(handle);
        result.ok()?;
        Some(counters.PrivateUsage as u64)
    }
}

#[test]
fn v27_private_usage_matches_sysinfo_virtual_memory() {
    let mut child = Command::new("cmd")
        .args(["/c", "ping -n 20 127.0.0.1 >nul"])
        .spawn()
        .expect("spawn static child");
    let child_pid = child.id();
    sleep(Duration::from_millis(1_200));

    let mut source = default_source();
    let raw = source.snapshot().expect("snapshot from windows backend");
    let process = raw
        .processes
        .iter()
        .find(|process| process.pid == child_pid)
        .expect("static child present in snapshot");

    let expected = private_usage(child_pid).expect("win32 private usage");
    assert!(expected > 0, "static child private usage must be readable");
    assert_eq!(
        process.memory_bytes,
        Some(expected),
        "virtual_memory() must equal PrivateUsage byte for byte"
    );

    let _ = child.kill();
    let _ = child.wait();
}
