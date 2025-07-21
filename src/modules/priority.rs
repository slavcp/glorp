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
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };

        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let pid = entry.th32ProcessID;
                if String::from_utf16_lossy(&entry.szExeFile)
                    .trim_matches('\0')
                    .to_string()
                    .to_lowercase()
                    .contains("webview2")
                {
                    if let Ok(handle) = OpenProcess(PROCESS_ALL_ACCESS, false, pid) {
                        SetPriorityClass(handle, priority_class).ok();
                        CloseHandle(handle).ok();
                    }
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
