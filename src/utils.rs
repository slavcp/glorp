#![allow(non_snake_case)]
use windows::{
    Win32::{
        Foundation::{BOOL, HWND, LPARAM},
        System::{
            Diagnostics::{Debug::OutputDebugStringA, ToolHelp::*},
            Threading::*,
        },
        UI::{Shell::ShellExecuteW, WindowsAndMessaging::{MB_ICONERROR, *}},
    },
    core::*,
};
pub fn create_utf_string(string: &str) -> Vec<u16> {
    let mut string_utf: Vec<u16> = string.encode_utf16().collect();
    string_utf.push(0);
    string_utf
}

pub fn LOWORD(l: usize) -> usize {
    l & 0xffff
}

pub fn HIWORD(l: usize) -> usize {
    (l >> 16) & 0xffff
}

pub fn find_child_window_by_class(parent: HWND, class_name: &str) -> HWND {
    let mut data = (HWND::default(), class_name);

    extern "system" fn enum_child_proc(handle: HWND, lparam: LPARAM) -> BOOL {
        unsafe {
            let data = lparam.0 as *mut (HWND, &str);
            let target_class = (*data).1;
            let mut class_name: [u16; 256] = [0; 256];

            GetClassNameW(handle, &mut class_name);

            let window_class = String::from_utf16_lossy(&class_name);

            if window_class.contains(target_class) {
                (*data).0 = handle;
                return BOOL(0);
            }

            BOOL(1)
        }
    }
    unsafe {
        if let BOOL(1) = EnumChildWindows(
            Some(parent),
            Some(enum_child_proc),
            LPARAM(&mut data as *mut (HWND, &str) as _),
        ) {
            eprint!("Could not find child window")
        }

        data.0
    }
}

pub fn set_panic_hook() {
    std::panic::set_hook(Box::new(|panic_info| {
        unsafe {
            OutputDebugStringA(s!("Panic occurred - logging details"));
        }

        let exe_path =
            std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("unknown_path"));
        let log_dir = exe_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("./"));
        let log_file_path = log_dir.join("crash_log.txt");

        let crash_message = format!(
            "Location: {}\n\
            Message: {}\n\
            \nStack Trace:\n{}\n",
            {
                let loc_string = panic_info
                    .location()
                    .map(|loc| loc.to_string())
                    .unwrap_or_else(|| "Unknown".to_string());
                loc_string.to_string()
            },
            panic_info
                .payload()
                .downcast_ref::<String>()
                .map(|s| s.as_str())
                .or_else(|| panic_info.payload().downcast_ref::<&str>().copied())
                .unwrap_or("<unknown>"),
            std::backtrace::Backtrace::force_capture()
        );

        if let Err(e) = std::fs::write(&log_file_path, &crash_message) {
            unsafe {
                OutputDebugStringA(s!("Failed to write crash log file"));
            }
            eprintln!(
                "Failed to write crash log to {}: {}",
                log_file_path.display(),
                e
            );
        } else {
            unsafe {
                OutputDebugStringA(s!("Crash log written successfully"));
            }
        }

        unsafe {
            let result = MessageBoxW(
                None,
                PCWSTR(
                    create_utf_string(&format!(
                        "An error has occurred.\n\
                     A crash report has been saved to:\n\
                     {}\n\n\
                     Click Yes to open the log.",
                        log_file_path.display()
                    ))
                    .as_ptr(),
                ),
                PCWSTR(create_utf_string("Application Error").as_ptr()),
                MB_YESNO | MB_ICONERROR,
            );

            if result.0 == 6 { // IDYES = 6
                ShellExecuteW(
                    None,
                    PCWSTR(create_utf_string("open").as_ptr()),
                    PCWSTR(create_utf_string(&log_file_path.to_string_lossy()).as_ptr()),
                    PCWSTR::null(),
                    PCWSTR::null(),
                    SW_SHOW,
                );
            }
        }
    }));
}

pub fn kill(wanted_process_name: &str) {
    unsafe {
        let current_pid = GetCurrentProcessId();
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };

        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPALL, 0).unwrap();

        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let process_name = String::from_utf16_lossy(&entry.szExeFile);
                if process_name.contains(wanted_process_name) && entry.th32ProcessID != current_pid
                {
                    if let Ok(process) = OpenProcess(PROCESS_TERMINATE, false, entry.th32ProcessID)
                    {
                        TerminateProcess(process, 0).ok();
                    }
                }
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
    }
}
