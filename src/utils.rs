#![allow(non_snake_case)]
use crate::CONFIG;
use std::{
    convert, env, fs, io, mem,
    path::{self, *},
};
use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2;
use windows::{
    Win32::{
        Foundation::{CloseHandle, HWND, LPARAM},
        System::{Diagnostics::ToolHelp::*, Threading::*},
        UI::WindowsAndMessaging::*,
    },
    core::*,
};

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UnsafeSend<T> {
    val: T,
}

unsafe impl<T> Send for UnsafeSend<T> {}
unsafe impl<T> Sync for UnsafeSend<T> {}

impl<T> UnsafeSend<T> {
    #[inline]
    pub const fn new(val: T) -> Self {
        Self { val }
    }

    #[inline]
    pub fn take(self) -> T {
        self.val
    }
}

impl<T> std::ops::Deref for UnsafeSend<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.val
    }
}

impl<T> std::ops::DerefMut for UnsafeSend<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.val
    }
}

use webview2_com::Microsoft::Web::WebView2::Win32::*;

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
    let s = string.as_ref();
    let mut v = Vec::with_capacity(s.len() + 1);
    v.extend(s.encode_utf16());
    v.push(0);
    v
}

pub fn LOWORD(l: usize) -> usize {
    l & 0xffff
}

pub fn HIWORD(l: usize) -> usize {
    (l >> 16) & 0xffff
}

pub fn settings_dir() -> path::PathBuf {
    path::PathBuf::from(env::var("USERPROFILE").unwrap())
        .join("Documents")
        .join("glorp")
}

pub fn config_bool(setting: &str, default: bool) -> bool {
    CONFIG.lock().unwrap().get(setting).unwrap_or(default)
}

pub fn config_string(setting: &str, default: impl Into<String>) -> String {
    CONFIG
        .lock()
        .unwrap()
        .get::<String>(setting)
        .unwrap_or_else(|| default.into())
}

pub fn find_child_window_by_class(parent: HWND, class_name: &str) -> HWND {
    let mut data = (HWND::default(), class_name);

    extern "system" fn enum_child_proc(handle: HWND, lparam: LPARAM) -> BOOL {
        unsafe {
            let data = lparam.0 as *mut (HWND, &str);
            let target_class = (*data).1;
            let mut class_name: [u16; 256] = [0; 256];

            GetClassNameW(handle, &mut class_name);
            let len = class_name.iter().position(|&c| c == 0).unwrap_or(256);
            let class_slice = &class_name[..len];
            let mut target_wide = [0u16; 64];
            let mut target_len = 0;
            for c in target_class.encode_utf16() {
                target_wide[target_len] = c;
                target_len += 1;
            }
            let target_slice = &target_wide[..target_len];
            if class_slice.windows(target_len).any(|w| w == target_slice) {
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

        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).unwrap();

        let mut target_wide = [0u16; 64];
        let mut target_len = 0;
        for c in wanted_process_name.encode_utf16() {
            target_wide[target_len] = c;
            target_len += 1;
        }
        let target_slice = &target_wide[..target_len];

        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
                if entry.szExeFile[..len].windows(target_len).any(|w| w == target_slice)
                    && entry.th32ProcessID != current_pid
                    && let Ok(process) = OpenProcess(PROCESS_TERMINATE, false, entry.th32ProcessID)
                {
                    TerminateProcess(process, 0).ok();
                    CloseHandle(process).ok();
                }
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        CloseHandle(snapshot).ok();
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
