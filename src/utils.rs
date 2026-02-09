#![allow(non_snake_case)]
#![allow(dead_code)]
use std::{convert, fs, io, mem, path::*};
use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2;
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM},
        System::{Diagnostics::ToolHelp::*, Threading::*},
        UI::WindowsAndMessaging::*,
    },
    core::*,
};

pub struct UnsafeSend<T> {
    val: T,
}

impl<T> UnsafeSend<T> {
    pub fn new(val: T) -> Self {
        Self { val }
    }

    pub fn take(self) -> T {
        self.val
    }
}
use webview2_com::Microsoft::Web::WebView2::Win32::*;

unsafe impl<T> Send for UnsafeSend<T> {}

pub trait EnvironmentRef {
    fn env_ref(&self) -> &ICoreWebView2Environment;
}

impl EnvironmentRef for ICoreWebView2Environment {
    fn env_ref(&self) -> &ICoreWebView2Environment {
        self
    }
}

impl EnvironmentRef for UnsafeSend<ICoreWebView2Environment> {
    fn env_ref(&self) -> &ICoreWebView2Environment {
        &self.val
    }
}

pub fn create_utf_string(string: impl AsRef<str>) -> Vec<u16> {
    let mut string_utf: Vec<u16> = string.as_ref().encode_utf16().collect();
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

pub fn kill(wanted_process_name: &str) {
    unsafe {
        let current_pid = GetCurrentProcessId();
        let mut entry = PROCESSENTRY32W {
            dwSize: mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };

        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPALL, 0).unwrap();

        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let process_name = String::from_utf16_lossy(&entry.szExeFile);
                if process_name.contains(wanted_process_name)
                    && entry.th32ProcessID != current_pid
                    && let Ok(process) = OpenProcess(PROCESS_TERMINATE, false, entry.th32ProcessID)
                {
                    TerminateProcess(process, 0).ok();
                }
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
    }
}

pub fn set_cpu_throttling(webview: &ICoreWebView2, value: f32) {
    unsafe {
        webview
            .CallDevToolsProtocolMethod(
                w!("Emulation.setCPUThrottlingRate"),
                PCWSTR(create_utf_string(format!("{{\"rate\":{}}}", value)).as_ptr()),
                None,
            )
            .ok();
    }
}

pub fn atomic_write(path: &impl AsRef<Path>, data: &impl convert::AsRef<[u8]>) -> io::Result<()> {
    let path = path.as_ref();
    let tmp_path = path.with_extension("tmp");
    fs::write(&tmp_path, data)?;

    fs::rename(tmp_path, path)?;
    Ok(())
}
