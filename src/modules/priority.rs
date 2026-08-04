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
                    .position(|&char| char == 0)
                    .unwrap_or(entry.szExeFile.len());
                let exe_slice = &entry.szExeFile[..len];

                const TARGET: &[u16] = &[
                    'w' as u16, 'e' as u16, 'b' as u16, 'v' as u16, 'i' as u16, 'e' as u16, 'w' as u16, '2' as u16,
                ];

                let contains_target = exe_slice.windows(TARGET.len()).any(|window| {
                    window.iter().zip(TARGET.iter()).all(|(&process_char, &target_char)| {
                        (process_char as u8).to_ascii_lowercase() == target_char as u8
                    })
                });

                if contains_target && let Ok(handle) = OpenProcess(PROCESS_ALL_ACCESS, false, pid) {
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
