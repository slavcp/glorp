use std::mem;
use windows::Win32::{
    Foundation::*,
    System::{Diagnostics::ToolHelp::*, Threading::*},
};

pub fn set(level: &str) {
    let priority_class = match level {
        "High" => HIGH_PRIORITY_CLASS,
        "Above Normal" => ABOVE_NORMAL_PRIORITY_CLASS,
        "Below Normal" => BELOW_NORMAL_PRIORITY_CLASS,
        "Idle" => IDLE_PRIORITY_CLASS,
        _ => NORMAL_PRIORITY_CLASS,
    };

    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).unwrap();
        let mut entry = PROCESSENTRY32W {
            dwSize: mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };

        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let pid = entry.th32ProcessID;
                let len = entry
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(entry.szExeFile.len());
                const WEBVIEW2: [u16; 8] = [0x77, 0x65, 0x62, 0x76, 0x69, 0x65, 0x77, 0x32];
                if entry.szExeFile[..len].windows(WEBVIEW2.len()).any(|w| {
                    w.iter()
                        .zip(WEBVIEW2.iter())
                        .all(|(&a, &b)| (if (0x41..=0x5A).contains(&a) { a + 0x20 } else { a }) == b)
                }) && let Ok(handle) = OpenProcess(PROCESS_ALL_ACCESS, false, pid)
                {
                    SetPriorityClass(handle, priority_class).ok();
                    CloseHandle(handle).ok();
                }

                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        CloseHandle(snapshot).ok();
        SetPriorityClass(GetCurrentProcess(), priority_class).ok();
    };
}
